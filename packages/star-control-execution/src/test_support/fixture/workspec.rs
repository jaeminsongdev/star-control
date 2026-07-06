use super::Fixture;
use serde_json::json;

impl Fixture {
    pub(crate) fn assign_implement_stage_to_local_process(&self) {
        let mut workspec = self
            .store
            .load_workspec("J-0001", "implement")
            .expect("load workspec");
        workspec["provider"] = json!("local-default");
        workspec["provider_instance"] = json!("local-default");
        workspec["required_outputs"] = json!(["provider-output/local-default/response.json"]);
        self.store
            .save_workspec("J-0001", "implement", &workspec)
            .expect("save local process workspec");
    }

    pub(crate) fn assign_implement_stage_to_cloud_provider(&self) {
        let mut workspec = self
            .store
            .load_workspec("J-0001", "implement")
            .expect("load workspec");
        workspec["provider"] = json!("cloud-default");
        workspec["provider_instance"] = json!("cloud-default");
        workspec["required_outputs"] = json!(["provider-output/cloud-default/response.json"]);
        self.store
            .save_workspec("J-0001", "implement", &workspec)
            .expect("save cloud workspec");
    }
}
