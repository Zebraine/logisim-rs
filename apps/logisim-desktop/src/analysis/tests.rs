use super::*;
use crate::background::support::{block, wait_until};
use crate::tests::support::workspace_shows;
use logisim_core::{Component, Kind, Point, generation::Cell};
fn workspace() -> Workspace {
    let mut a = Analysis::default();
    a.add_output("Y".into()).unwrap();
    a.set_cell(0, 0, Cell::One).unwrap();
    let mut w = Workspace::default();
    w.replace_circuit(a.generate().unwrap());
    w
}
fn wait(service: &mut Service, w: &mut Workspace) {
    wait_until(|| {
        service.update(w, || {});
        service.jobs.is_idle()
    });
}
#[test]
fn isolated_worker_delivers_without_changing_document_and_shutdown_joins() {
    let mut w = workspace();
    let before = w.project().clone();
    let mut service = Service::default();
    w.request_analysis();
    service.update(&mut w, || {});
    assert!(!service.jobs.is_idle());
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
    let blocked = block(&mut service.jobs, request, |_| Ok(Analysis::default()));
    // Replacing a document, even with identical IDs, invalidates the UI request.
    let mut project = w.project().clone();
    project
        .main_circuit_mut()
        .circuit
        .add_component(Component::new(Kind::Register, Point::ZERO));
    w.replace_project(project);
    w.request_analysis();
    service.update(&mut w, || {});
    assert!(!service.jobs.is_idle());
    assert!(blocked.cancellation.is_cancelled());
    drop(blocked);
    wait(&mut service, &mut w);
    assert!(!w.analysis_pending(serial));
}

#[test]
fn changed_snapshot_is_rejected_even_while_the_request_is_pending() {
    let mut w = workspace();
    w.request_analysis();
    let (serial, root) = w.take_analysis_request().unwrap();
    let analysis = logisim_sim::analysis::analyze(w.project(), root).unwrap();
    let mut snapshot = w.project().clone();
    snapshot.main_circuit_mut().name = "old name".into();
    let mut service = Service::default();
    let blocked = block(
        &mut service.jobs,
        Request {
            serial,
            root,
            project: Arc::new(snapshot),
        },
        move |_| Ok(analysis),
    );
    assert!(w.analysis_pending(serial));
    drop(blocked);
    wait(&mut service, &mut w);
    assert!(!w.analysis_pending(serial));
    assert!(w.take_generation_request().is_none());
    assert!(workspace_shows(
        &mut w,
        "The circuit changed during analysis. Please analyze it again."
    ));
}

#[test]
fn worker_panic_is_reported_without_replacing_the_document() {
    let mut w = workspace();
    let before = w.project().clone();
    w.request_analysis();
    let (serial, root) = w.take_analysis_request().unwrap();
    let mut service = Service::default();
    let blocked = block(
        &mut service.jobs,
        Request {
            serial,
            root,
            project: Arc::new(before.clone()),
        },
        |_| panic!("test analysis worker failure"),
    );
    drop(blocked);
    wait(&mut service, &mut w);
    assert_eq!(w.project(), &before);
    assert!(workspace_shows(&mut w, "Circuit analysis worker stopped."));
}
