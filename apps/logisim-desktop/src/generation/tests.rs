use super::*;
use crate::background::support::{block, wait_until};
use crate::tests::support::workspace_shows;
use logisim_core::generation::Analysis;
fn workspace() -> Workspace {
    let mut w = Workspace::default();
    let mut a = Analysis::default();
    a.add_output("Y".into()).unwrap();
    let mut project = Project::single("main", a.generate().unwrap());
    project.add("other".into()).unwrap();
    w.replace_project(project);
    w.request_analysis();
    let (serial, _) = w.take_analysis_request().unwrap();
    w.receive_analysis(serial, Ok(a));
    w
}
fn wait(service: &mut Service, w: &mut Workspace) {
    wait_until(|| {
        service.update(w, || {});
        service.jobs.is_idle()
    });
}
#[test]
fn worker_delivers_readonly_and_replaced_document_cancels_without_joining() {
    let mut w = workspace();
    let before = w.project().clone();
    let mut service = Service::default();
    service.update(&mut w, || {});
    assert!(!service.jobs.is_idle());
    wait(&mut service, &mut w);
    assert_eq!(w.project(), &before);
    let mut w = workspace();
    let task = w.take_generation_request().unwrap();
    let request = Request {
        task,
        root: w.active_circuit(),
        project: Arc::new(w.project().clone()),
    };
    let blocked = block(&mut service.jobs, request, |request| {
        request.task.execute(|| false)
    });
    w.replace_project(Project::default());
    service.update(&mut w, || {});
    assert!(blocked.cancellation.is_cancelled());
    drop(blocked);
    wait(&mut service, &mut w);
    assert_eq!(w.project(), &Project::default());
    let mut w = workspace();
    service.update(&mut w, || {});
    drop(service); // Cancels and joins an outstanding task.
}

#[test]
fn generation_validates_both_document_snapshot_and_active_circuit() {
    for stale in ["neither", "project", "root"] {
        let mut w = workspace();
        let before = w.project().clone();
        let task = w.take_generation_request().unwrap();
        let serial = task.serial;
        let mut snapshot = before.clone();
        if stale == "project" {
            snapshot.main_circuit_mut().name = "old name".into();
        }
        let root = if stale == "root" {
            snapshot
                .circuits
                .iter()
                .find(|c| c.id != snapshot.main)
                .unwrap()
                .id
        } else {
            w.active_circuit()
        };
        let mut service = Service::default();
        let blocked = block(
            &mut service.jobs,
            Request {
                task,
                root,
                project: Arc::new(snapshot),
            },
            |request| request.task.execute(|| false),
        );
        assert!(w.generation_pending(serial));
        drop(blocked);
        wait(&mut service, &mut w);
        assert!(!w.generation_pending(serial));
        assert_eq!(w.project(), &before);
        assert_eq!(
            workspace_shows(
                &mut w,
                "The circuit changed during analysis. Please analyze it again."
            ),
            stale != "neither",
            "{stale}"
        );
    }
}

#[test]
fn worker_panic_is_reported_without_replacing_the_document() {
    let mut w = workspace();
    let before = w.project().clone();
    let mut service = Service::default();
    let blocked = block(
        &mut service.jobs,
        Request {
            task: w.take_generation_request().unwrap(),
            root: w.active_circuit(),
            project: Arc::new(before.clone()),
        },
        |_| panic!("test computation worker failure"),
    );
    drop(blocked);
    wait(&mut service, &mut w);
    assert_eq!(w.project(), &before);
    assert!(workspace_shows(&mut w, "Computation worker stopped."));
}
