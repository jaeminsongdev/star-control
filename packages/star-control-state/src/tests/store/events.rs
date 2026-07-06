use super::super::{create_job, event, open_store, read_example, temp_project};
use std::fs;

#[test]
fn appends_events_in_order() {
    let project = temp_project();
    let store = open_store(&project);
    let job = create_job(&store);
    let job_id = job["job_id"].as_str().unwrap();

    store
        .append_event(job_id, &event(job_id, "EV-0002", "second"))
        .expect("append second event");
    store
        .append_event(job_id, &event(job_id, "EV-0003", "third"))
        .expect("append third event");
    let events = store.read_events(job_id).expect("read events");

    assert_eq!(events.len(), 3);
    assert_eq!(events[0]["type"], "JOB_CREATED");
    assert_eq!(events[1]["event_id"], "EV-0002");
    assert_eq!(events[2]["event_id"], "EV-0003");

    fs::remove_dir_all(project).ok();
}

#[test]
fn route_workspec_and_report_roundtrip() {
    let project = temp_project();
    let store = open_store(&project);
    create_job(&store);

    let route = read_example("examples/runs/J-0001/route.json");
    let workspec = read_example("examples/runs/J-0001/workspecs/implement.json");
    let report = read_example("examples/fake/impl-report-done.json");

    store.save_route("J-0001", &route).expect("save route");
    store
        .save_workspec("J-0001", "implement", &workspec)
        .expect("save workspec");
    store
        .save_report("J-0001", "implement-report", &report)
        .expect("save report");

    assert_eq!(store.load_route("J-0001").expect("load route"), route);
    assert_eq!(
        store
            .load_workspec("J-0001", "implement")
            .expect("load workspec"),
        workspec
    );
    assert_eq!(
        store
            .load_report("J-0001", "implement-report")
            .expect("load report"),
        report
    );

    fs::remove_dir_all(project).ok();
}
