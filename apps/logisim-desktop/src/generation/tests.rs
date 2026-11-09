use super::*;
use logisim_core::generation::Analysis;
use std::{
    sync::Barrier,
    time::{Duration, Instant},
};
fn workspace() -> Workspace {
    let mut w = Workspace::default();
    let mut a = Analysis::default();
    a.add_output("Y".into()).unwrap();
    w.replace_circuit(a.generate().unwrap());
    w.request_analysis();
    let (serial, _) = w.take_analysis_request().unwrap();
    w.receive_analysis(serial, Ok(a));
    w
}
fn wait(service: &mut Service, w: &mut Workspace) {
    let start = Instant::now();
    while service.job.is_some() || service.waiting.is_some() {
        assert!(start.elapsed() < Duration::from_secs(10));
        service.update(w, || {});
        thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn worker_delivers_readonly_and_replaced_document_cancels_without_joining() {
    let mut w = workspace();
    let before = w.project().clone();
    let mut service = Service::default();
    service.update(&mut w, || {});
    assert!(service.job.is_some());
    wait(&mut service, &mut w);
    assert_eq!(w.project(), &before);
    let mut w = workspace();
    let task = w.take_generation_request().unwrap();
    let request = Request {
        task: task.clone(),
        root: w.active_circuit(),
        project: Arc::new(w.project().clone()),
    };
    let barrier = Arc::new(Barrier::new(2));
    let worker_barrier = barrier.clone();
    service.job = Some(Job {
        request,
        cancelled: Arc::new(AtomicBool::new(false)),
        thread: thread::spawn(move || {
            worker_barrier.wait();
            task.execute(|| false)
        }),
    });
    w.replace_project(Project::default());
    service.update(&mut w, || {});
    assert!(
        service
            .job
            .as_ref()
            .unwrap()
            .cancelled
            .load(Ordering::Relaxed)
    );
    barrier.wait();
    wait(&mut service, &mut w);
    assert_eq!(w.project(), &Project::default());
    let mut w = workspace();
    service.update(&mut w, || {});
    drop(service); // Cancels and joins an outstanding task.
}
