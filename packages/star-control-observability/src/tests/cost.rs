use super::helpers::{create_job, open_store, schema_root, temp_project};
use crate::{CostBudgetThresholds, CostMetricWriter};
use serde_json::json;
use std::fs;

#[test]
fn writes_schema_valid_cost_metric_inside_provider_output() {
    let project = temp_project("cost-write");
    let store = open_store(&project);
    create_job(&store);
    let writer = CostMetricWriter::new(schema_root());
    let metric = writer.metric("J-0001", "implement", "fake-default", 0.0, "USD", 1);

    let artifact_ref = writer
        .write_provider_metric(&store, &metric)
        .expect("write cost metric");
    assert_eq!(
        artifact_ref["path"],
        "provider-output/fake-default/cost-metric.json"
    );
    assert_eq!(artifact_ref["kind"], "provider_output");

    let read = writer
        .read_provider_metric(&store, "J-0001", "fake-default")
        .expect("read cost metric")
        .expect("cost metric exists");
    assert_eq!(read["estimated_cost"], 0.0);
    assert_eq!(read["wall_time_ms"], 1);

    fs::remove_dir_all(project).ok();
}

#[test]
fn cost_metric_writer_redacts_unexpected_secret_fields() {
    let project = temp_project("cost-redact");
    let store = open_store(&project);
    create_job(&store);
    let writer = CostMetricWriter::new(schema_root());
    let api_key = format!("{}{}", "sk-test", "-secret");
    let mut metric = writer.metric("J-0001", "implement", "cloud-default", 1.0, "USD", 20);
    metric["debug"] = json!(format!("Authorization: Bearer {}", api_key));

    writer
        .write_provider_metric(&store, &metric)
        .expect("write redacted cost metric");
    let text = fs::read_to_string(
        project.join(".ai-runs/J-0001/provider-output/cloud-default/cost-metric.json"),
    )
    .expect("read cost metric");
    assert!(!text.contains(&api_key));
    assert!(!text.contains("Bearer"));
    assert!(text.contains("[REDACTED]"));

    fs::remove_dir_all(project).ok();
}

#[test]
fn budget_guard_warns_without_requiring_metric_to_exist() {
    let project = temp_project("budget");
    let store = open_store(&project);
    create_job(&store);
    let writer = CostMetricWriter::new(schema_root());
    assert!(writer
        .read_provider_metric(&store, "J-0001", "fake-default")
        .expect("missing cost metric is not fatal")
        .is_none());

    let mut metric = writer.metric("J-0001", "implement", "cloud-default", 1.25, "USD", 50);
    metric["input_tokens"] = json!(25);
    metric["output_tokens"] = json!(30);
    let evaluation = writer
        .evaluate_budget(
            &metric,
            &CostBudgetThresholds::new()
                .with_max_estimated_cost(1.0)
                .with_max_wall_time_ms(25)
                .with_max_total_tokens(50),
        )
        .expect("evaluate budget");

    assert_eq!(evaluation["status"], "warning");
    assert_eq!(evaluation["enforcement"], "warn_only");
    assert_eq!(evaluation["reasons"].as_array().expect("reasons").len(), 3);
    assert_eq!(
        evaluation["metric_path"],
        "provider-output/cloud-default/cost-metric.json"
    );

    fs::remove_dir_all(project).ok();
}

#[test]
fn cost_metric_writer_rejects_unsafe_provider_path() {
    let project = temp_project("cost-traversal");
    let store = open_store(&project);
    create_job(&store);
    let writer = CostMetricWriter::new(schema_root());
    let metric = writer.metric("J-0001", "implement", "../cloud-default", 0.0, "USD", 1);

    let result = writer.write_provider_metric(&store, &metric);
    assert!(result.is_err());
    assert!(!project
        .join(".ai-runs/J-0001/provider-output/cost-metric.json")
        .exists());

    fs::remove_dir_all(project).ok();
}
