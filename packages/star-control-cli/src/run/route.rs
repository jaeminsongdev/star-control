use crate::constants::DEFAULT_PROVIDER;
use serde_json::{json, Value};

pub(super) fn route_value_for_provider(route: &Value, provider_instance_id: &str) -> Value {
    let mut route = route.clone();
    if provider_instance_id == DEFAULT_PROVIDER {
        return route;
    }
    if let Some(assignments) = route.get_mut("assignments").and_then(Value::as_object_mut) {
        for assignment in assignments.values_mut() {
            if let Some(assignment) = assignment.as_object_mut() {
                assignment.insert(
                    "provider".to_string(),
                    Value::String(provider_instance_id.to_string()),
                );
            }
        }
    }
    if let Some(reasons) = route
        .get_mut("routing_reasons")
        .and_then(Value::as_array_mut)
    {
        reasons.push(Value::String(format!(
            "cli provider override: {}",
            provider_instance_id
        )));
    }
    route
}

pub(super) fn workspec_value_for_provider(workspec: &Value, provider_instance_id: &str) -> Value {
    let mut workspec = workspec.clone();
    if provider_instance_id == DEFAULT_PROVIDER {
        return workspec;
    }
    if let Some(workspec) = workspec.as_object_mut() {
        workspec.insert(
            "provider".to_string(),
            Value::String(provider_instance_id.to_string()),
        );
        workspec.insert(
            "provider_instance".to_string(),
            Value::String(provider_instance_id.to_string()),
        );
        workspec.insert(
            "required_outputs".to_string(),
            json!([format!(
                "provider-output/{}/response.json",
                provider_instance_id
            )]),
        );
    }
    workspec
}
