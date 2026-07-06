use crate::analysis::RequestAnalysis;
use crate::constants::{FAKE_PROVIDER_ID, ROUTER_DECISION_SCHEMA, ROUTE_SCHEMA, SCHEMA_VERSION};
use crate::contract::validate_contract;
use crate::types::{JobSpec, RouteSpec, RouterDecision, RouterOutput};
use crate::workspec::{
    assignments_for_stages, build_workspec_for_stage, decision_id, summary,
    workspec_paths_for_stages,
};
use crate::RouterError;
use serde_json::json;
use star_control_provider::{CapabilityValue, ProviderRegistry};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RouterEngine<'a> {
    registry: &'a ProviderRegistry,
    schema_root: PathBuf,
}

impl<'a> RouterEngine<'a> {
    pub fn new(registry: &'a ProviderRegistry, schema_root: impl Into<PathBuf>) -> Self {
        Self {
            registry,
            schema_root: schema_root.into(),
        }
    }

    pub fn route(&self, job: &JobSpec) -> Result<RouterOutput, RouterError> {
        let analysis = RequestAnalysis::analyze(job);
        let provider_instance_id =
            self.select_fake_provider_instance("worker-impl", "return_json")?;
        let stages = analysis.stages();
        let assignments = assignments_for_stages(&stages, &provider_instance_id, analysis.profile);
        let workspec_paths = workspec_paths_for_stages(&stages);

        let decision_value = json!({
            "schema_version": SCHEMA_VERSION,
            "decision_id": decision_id(job.job_id()),
            "size": analysis.size.as_str(),
            "risk": analysis.risk.as_str(),
            "policy_profile": analysis.profile.as_str(),
            "decision": analysis.decision.as_str(),
            "requires_user_approval": analysis.requires_user_approval,
            "approval_reasons": analysis.approval_reasons,
            "change_types": analysis.change_type_strings(),
            "routing_reasons": analysis.routing_reasons,
            "recommended_stages": stages,
        });
        validate_contract(
            &decision_value,
            Path::new("router-decision.json"),
            &self.schema_root,
            ROUTER_DECISION_SCHEMA,
        )?;

        let route_value = json!({
            "schema_version": SCHEMA_VERSION,
            "job_id": job.job_id(),
            "summary": summary(job.request_text()),
            "size": analysis.size.as_str(),
            "risk": analysis.risk.as_str(),
            "policy_profile": analysis.profile.as_str(),
            "decision": analysis.decision.as_str(),
            "change_types": analysis.change_type_strings(),
            "routing_reasons": analysis.routing_reasons,
            "stages": stages,
            "assignments": assignments,
            "requires_user_approval": analysis.requires_user_approval,
            "approval_reasons": analysis.approval_reasons,
            "workspecs": workspec_paths,
        });
        validate_contract(
            &route_value,
            Path::new("route.json"),
            &self.schema_root,
            ROUTE_SCHEMA,
        )?;

        let mut workspecs = BTreeMap::new();
        for stage in stages.iter().filter(|stage| **stage != "route") {
            let workspec = build_workspec_for_stage(
                job,
                stage,
                &provider_instance_id,
                &analysis,
                &self.schema_root,
            )?;
            workspecs.insert(stage.to_string(), workspec);
        }

        Ok(RouterOutput {
            decision: RouterDecision {
                value: decision_value,
            },
            route: RouteSpec { value: route_value },
            workspecs,
        })
    }

    fn select_fake_provider_instance(
        &self,
        role: &str,
        capability: &str,
    ) -> Result<String, RouterError> {
        for instance in self.registry.enabled_instances() {
            let manifest = self.registry.manifest_for_instance(instance.id())?;
            if manifest.id() != FAKE_PROVIDER_ID {
                continue;
            }
            let profile = self.registry.capability_for_instance(instance.id())?;
            let has_capability = profile
                .capability(capability)
                .map(CapabilityValue::is_enabled)
                .unwrap_or(false);
            let offline = profile
                .capability("work_offline")
                .map(CapabilityValue::is_enabled)
                .unwrap_or(false);
            if has_capability && offline {
                return Ok(instance.id().to_string());
            }
        }

        Err(RouterError::NoProviderAvailable {
            role: role.to_string(),
            capability: capability.to_string(),
        })
    }
}
