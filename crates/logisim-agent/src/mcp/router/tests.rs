use super::*;
use std::{path::Path, time::Duration};

fn reply(frame: &[u8]) -> Value {
    serde_json::from_slice(frame).unwrap()
}

#[test]
fn stalled_session_cannot_block_its_neighbor_and_a_crash_accounts_for_queued_ids() {
    let (tx, rx) = mpsc::channel();
    let notify = || {
        let tx = tx.clone();
        move |event| {
            tx.send(event).unwrap();
        }
    };
    let next = || rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/agent/counter.circ");
    let mut router = Router::new(Service::default());
    let (release, gate) = mpsc::channel();
    router
        .open(
            &1.into(),
            path.clone(),
            move |_, _| {
                gate.recv_timeout(Duration::from_secs(10)).unwrap();
                panic!("injected worker panic")
            },
            notify(),
        )
        .unwrap();
    let slow = next();
    let a = slow.session.clone();
    router.finish(slow);
    router
        .open(&2.into(), path, Service::call, notify())
        .unwrap();
    let fast = next();
    let b = fast.session.clone();
    router.finish(fast);
    let submit = |router: &mut Router, id, key: &String| {
        router.submit(
            &Value::from(id),
            Ok(Request::SimulationState {
                session: key.clone(),
            }),
            None,
            notify(),
        )
    };
    assert!(submit(&mut router, 3, &a).is_none());
    assert!(submit(&mut router, 4, &a).is_none());
    assert!(submit(&mut router, 5, &b).is_none());
    let fast = next();
    assert_eq!(fast.session, b);
    assert_eq!(reply(&router.finish(fast)[0])["id"], 5);
    release.send(()).unwrap();
    let crash = next();
    // This call arrives after failure but before the router observes it.
    assert!(submit(&mut router, 6, &a).is_none());
    let failed = router.finish(crash);
    assert_eq!(failed.len(), 3);
    for (frame, id) in failed.iter().zip([3, 4, 6]) {
        let value = reply(frame);
        assert_eq!(value["id"], id);
        assert_eq!(
            value["result"]["structuredContent"]["error"]["code"],
            "worker_stopped"
        );
    }
    assert_eq!(
        reply(&submit(&mut router, 7, &a).unwrap())["result"]["structuredContent"]["error"]["code"],
        "unknown_session"
    );
    assert!(submit(&mut router, 8, &b).is_none());
    assert_eq!(
        reply(&router.finish(next())[0])["result"]["structuredContent"]["error"]["code"],
        "simulation_not_open"
    );
}
