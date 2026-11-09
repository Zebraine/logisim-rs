use super::{support::*, *};
use std::sync::mpsc;

fn finished<R, T>(jobs: &LatestJob<R, T>) {
    wait_until(|| jobs.running.as_ref().unwrap().thread.is_finished());
}

#[test]
fn completion_keeps_request_identity_wakes_and_is_delivered_once() {
    let mut jobs = LatestJob::default();
    let (wake, woke) = mpsc::channel();
    jobs.submit(String::from("request"));
    assert!(
        jobs.start(
            "test-background-result",
            |_| true,
            |request, cancellation| {
                assert!(!cancellation.is_cancelled());
                request.len()
            },
            move || wake.send(()).unwrap(),
        )
        .is_none()
    );
    woke.recv_timeout(TIMEOUT).unwrap();
    finished(&jobs);
    let (request, result) = jobs.poll(|_| true).unwrap();
    assert_eq!(request, "request");
    assert_eq!(result.unwrap(), 7);
    assert!(jobs.poll(|_| true).is_none());
    assert!(jobs.is_idle());
}

#[test]
fn replacement_waits_for_predecessor_and_only_latest_request_runs() {
    let mut jobs = LatestJob::default();
    let blocked = block(&mut jobs, 1, |request| request);
    jobs.submit(2);
    jobs.submit(3);
    assert!(blocked.cancellation.is_cancelled());
    assert!(jobs.poll(|_| true).is_none());
    assert!(
        jobs.start(
            "test-background-overlap",
            |_| panic!("must not start before predecessor exits"),
            |_, _| panic!("must not run concurrently"),
            || {},
        )
        .is_none()
    );
    drop(blocked);
    finished(&jobs);
    assert!(jobs.poll(|_| true).is_none());
    assert!(
        jobs.start(
            "test-background-latest",
            |_| true,
            |request, _| request * 10,
            || {},
        )
        .is_none()
    );
    finished(&jobs);
    let (request, result) = jobs.poll(|_| true).unwrap();
    assert_eq!((request, result.unwrap()), (3, 30));
    assert!(jobs.is_idle());
}

#[test]
fn cancellation_does_not_wait_and_cannot_be_revoked_by_a_later_poll() {
    let mut jobs = LatestJob::default();
    let blocked = block(&mut jobs, 1, |request| request);
    // The worker cannot finish until this thread releases it below.
    assert!(jobs.poll(|_| false).is_none());
    assert!(blocked.cancellation.is_cancelled());
    drop(blocked);
    finished(&jobs);
    assert!(jobs.poll(|_| true).is_none());
    assert!(jobs.is_idle());
}

#[test]
fn finished_result_is_discarded_if_replaced_before_delivery() {
    let mut jobs = LatestJob::default();
    let blocked = block(&mut jobs, 1, |request| request);
    drop(blocked);
    finished(&jobs);
    jobs.submit(2);
    assert!(jobs.poll(|_| true).is_none());
    // The pending replacement may also expire before its thread starts.
    assert!(
        jobs.start(
            "test-background-expired",
            |request| {
                assert_eq!(*request, 2);
                false
            },
            |_, _| panic!("expired request must not execute"),
            || panic!("expired request must not wake"),
        )
        .is_none()
    );
    assert!(jobs.is_idle());
}

#[test]
fn panic_is_returned_to_the_caller_and_does_not_block_the_next_job() {
    let mut jobs = LatestJob::default();
    let blocked = block(&mut jobs, 1, |_| -> i32 { panic!("test worker failure") });
    drop(blocked);
    finished(&jobs);
    let (request, result) = jobs.poll(|_| true).unwrap();
    assert_eq!(request, 1);
    assert!(result.is_err());
    let blocked = block(&mut jobs, 2, |request| request);
    drop(blocked);
    finished(&jobs);
    assert_eq!(jobs.poll(|_| true).unwrap().1.unwrap(), 2);
}

#[test]
fn shutdown_cancels_and_joins_before_returning() {
    let mut jobs = LatestJob::default();
    let (done, completed) = mpsc::channel();
    jobs.submit(1);
    assert!(
        jobs.start(
            "test-background-shutdown",
            |_| true,
            move |request, cancellation| {
                wait_until(|| cancellation.is_cancelled());
                done.send(request).unwrap();
            },
            || {},
        )
        .is_none()
    );
    drop(jobs);
    assert_eq!(completed.try_recv().unwrap(), 1);
}
