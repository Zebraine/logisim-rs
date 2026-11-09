use super::*;
use logisim_core::{Component, Kind, Point, generation::Cell};
use std::{
    sync::Barrier,
    time::{Duration, Instant},
};
fn workspace() -> Workspace {
    let mut a = Analysis::default();
    a.add_output("Y".into()).unwrap();
    a.set_cell(0, 0, Cell::One).unwrap();
    let mut w = Workspace::default();
    w.replace_circuit(a.generate().unwrap());
    w
}
fn wait(service: &mut Service, w: &mut Workspace) {
    let start = Instant::now();
    while service.job.is_some() || service.waiting.is_some() {
        assert!(start.elapsed() < Duration::from_secs(10));
        service.update(w, || {});
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn isolated_worker_delivers_without_changing_document_and_shutdown_joins() {
    let mut w = workspace();
    let before = w.project().clone();
    let mut service = Service::default();
    w.request_analysis();
    service.update(&mut w, || {});
    assert!(service.job.is_some());
    wait(&mut service, &mut w);
    assert_eq!(w.project(), &before);
    w.request_analysis();
    service.update(&mut w, || {});
    drop(service);
}

#[test]
fn worker_analyzes_the_explicit_target_while_a_stateful_parent_is_open() {
    let mut w = workspace();
    let mut project = w.project().clone();
    let child = project.main;
    let parent = project.add("parent".into()).unwrap();
    project.main = parent;
    let instance = project.instance(child, Point::new(200, 200)).unwrap();
    let circuit = &mut project.circuit_mut(parent).unwrap().circuit;
    circuit.add_component(instance);
    circuit.add_component(Component::new(Kind::Register, Point::new(400, 200)));
    w.replace_project(project.clone());
    let mut service = Service::default();
    w.request_circuit_analysis(child);
    service.update(&mut w, || {});
    assert_eq!(service.job.as_ref().unwrap().request.root, child);
    wait(&mut service, &mut w);
    // A successful result opens the generator and requests its initial expression.
    assert!(w.take_generation_request().is_some());
    assert_eq!(w.active_circuit(), parent);
    assert_eq!(w.project(), &project);
}
#[test]
fn stale_and_replaced_requests_are_discarded_without_blocking_the_gui() {
    let mut w = workspace();
    let mut service = Service::default();
    w.request_analysis();
    let (serial, _) = w.take_analysis_request().unwrap();
    let request = Request {
        serial,
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
            Ok(Analysis::default())
        }),
    });
    // Replacing a document, even with identical IDs, invalidates the UI request.
    let mut project = w.project().clone();
    project
        .main_circuit_mut()
        .circuit
        .add_component(Component::new(Kind::Register, Point::ZERO));
    w.replace_project(project);
    w.request_analysis();
    service.update(&mut w, || {});
    assert!(service.waiting.is_some());
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
    assert!(!w.analysis_pending(serial));
}
