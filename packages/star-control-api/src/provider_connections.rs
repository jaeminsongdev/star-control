use crate::constants::SCHEMA_VERSION;
use crate::{ApiControlService, ApiError, ApiReadOnlyService};
use serde_json::{json, Value};
use star_control_provider::{
    is_cloud_api_manifest, is_cloud_cli_manifest, is_cloud_provider_manifest,
    is_local_openai_compatible_manifest, ProviderManifest, ProviderRegistry,
    ProviderRegistryLoader,
};
use star_control_schema::{load_schema, validate_json};
use std::fs;
use std::path::{Path, PathBuf};

const BUILTIN_PROVIDER_REGISTRY: &str = "configs/registries/builtin-provider-registry.yaml";
const PROVIDER_INSTANCE_SCHEMA: &str = "provider-instance.schema.json";
const PROVIDER_INSTANCES_DIR: &str = "provider-instances";
const PROVIDER_CONNECTIONS_DIR: &str = "provider-connections";
const SELECTION_FILE: &str = "selection.json";
const FAKE_DEFAULT: &str = "fake-default";
const LOCAL_PROCESS_KIND: &str = "local_process_model";
const PROCESS_TRANSPORT: &str = "process";
const RAW_CREDENTIAL_FIELDS: &[&str] = &[
    "api_key",
    "apikey",
    "token",
    "access_token",
    "refresh_token",
    "secret",
    "password",
    "credential",
    "credentials",
    "bearer_token",
];

impl ApiReadOnlyService {
    pub(crate) fn provider_connections_response(&self) -> Result<Value, ApiError> {
        let registry = match self.load_builtin_provider_registry() {
            Ok(registry) => registry,
            Err(message) => {
                return self.error_envelope(
                    "provider_registry_unavailable",
                    &message,
                    json!({ "registry_path": BUILTIN_PROVIDER_REGISTRY }),
                );
            }
        };
        let providers = registry
            .providers()
            .into_iter()
            .map(|manifest| manifest_summary(manifest, &registry, self.repo_root()))
            .collect::<Vec<_>>();
        let instances = self.discover_saved_provider_instances()?;
        let selection = self.read_provider_selection();

        self.success_envelope(json!({
            "command": "provider_connections",
            "registry_path": BUILTIN_PROVIDER_REGISTRY,
            "storage": self.provider_connection_storage_summary(),
            "providers": providers,
            "instances": instances,
            "selection": selection,
            "policy": {
                "credential_raw_value_storage_allowed": false,
                "mock_success_allowed": false,
                "local_openai_compatible_live_surface": "cli_explicit_provider_instance_only",
                "daemon_local_ai_live_connector": "disabled",
                "cloud_live_execution_without_approval": "blocked",
                "live_calls_performed": false
            }
        }))
    }

    pub(crate) fn provider_connection_validate_response(
        &self,
        body: &Value,
    ) -> Result<Value, ApiError> {
        let instance = request_instance_value(body);
        let validation = self.validate_provider_instance_value(instance)?;
        let status = if validation.ok { "success" } else { "failed" };
        let error = if validation.ok {
            Value::Null
        } else {
            json!({
                "code": "provider_instance_invalid",
                "message": "provider instance did not pass schema or policy validation"
            })
        };
        self.envelope(
            status,
            json!({
                "command": "provider_connection_validate",
                "validation": validation.value
            }),
            error,
            validation.warnings,
        )
    }
}

impl ApiControlService {
    pub(crate) fn provider_connection_save_response(
        &self,
        body: &Value,
    ) -> Result<Value, ApiError> {
        let instance = request_instance_value(body);
        let validation = self.read_only.validate_provider_instance_value(instance)?;
        if !validation.ok {
            return self.read_only.envelope(
                "failed",
                json!({
                    "command": "provider_connection_save",
                    "validation": validation.value
                }),
                json!({
                    "code": "provider_instance_invalid",
                    "message": "provider instance was not saved"
                }),
                validation.warnings,
            );
        }
        let Some(store) = self.read_only.provider_connection_store() else {
            return self.read_only.error_envelope(
                "provider_connection_store_unconfigured",
                "provider connection storage requires daemon config root",
                json!({}),
            );
        };
        let instance_id = required_string(instance, "id").unwrap_or_default();
        let path = store.instance_path(&instance_id);
        if let Err(source) = fs::create_dir_all(store.instances_dir()) {
            return self.read_only.error_envelope(
                "provider_connection_store_failed",
                &format!(
                    "provider instance directory could not be created: {}",
                    source
                ),
                json!({ "directory": store.instances_dir().display().to_string() }),
            );
        }
        let mut bytes =
            serde_json::to_vec_pretty(instance).map_err(|source| ApiError::SchemaLoadFailed {
                path: path.clone(),
                message: source.to_string(),
            })?;
        bytes.push(b'\n');
        if let Err(source) = fs::write(&path, bytes) {
            return self.read_only.error_envelope(
                "provider_connection_store_failed",
                &format!("provider instance could not be saved: {}", source),
                json!({ "path": path.display().to_string() }),
            );
        }

        self.read_only.success_envelope(json!({
            "command": "provider_connection_save",
            "saved": true,
            "instance": instance_summary(instance, Some(&path), "valid"),
            "validation": validation.value,
            "cli_reuse": cli_reuse_value(&instance_id, Some(&path))
        }))
    }

    pub(crate) fn provider_connection_select_response(
        &self,
        body: &Value,
    ) -> Result<Value, ApiError> {
        let Some(store) = self.read_only.provider_connection_store() else {
            return self.read_only.error_envelope(
                "provider_connection_store_unconfigured",
                "provider connection selection requires daemon config root",
                json!({}),
            );
        };
        let instance_id = request_string(body, "provider_instance_id")
            .or_else(|| request_string(body, "instance_id"))
            .unwrap_or_default();
        if instance_id.is_empty() {
            return self.read_only.error_envelope(
                "provider_instance_id_required",
                "provider_instance_id is required",
                json!({}),
            );
        }
        let instance_path = store.instance_path(&instance_id);
        if !instance_path.is_file() && instance_id != FAKE_DEFAULT {
            return self.read_only.error_envelope(
                "provider_instance_not_found",
                "selected provider instance is not saved in daemon config root",
                json!({
                    "provider_instance_id": instance_id,
                    "path": instance_path.display().to_string()
                }),
            );
        }
        if let Err(source) = fs::create_dir_all(store.connections_dir()) {
            return self.read_only.error_envelope(
                "provider_connection_store_failed",
                &format!(
                    "provider connection directory could not be created: {}",
                    source
                ),
                json!({ "directory": store.connections_dir().display().to_string() }),
            );
        }
        let selection = json!({
            "schema_version": SCHEMA_VERSION,
            "selected_provider_instance_id": instance_id,
            "provider_instance_path": if instance_path.is_file() {
                Value::String(instance_path.display().to_string())
            } else {
                Value::Null
            },
            "live_calls_performed": false
        });
        let bytes =
            serde_json::to_vec_pretty(&selection).map_err(|source| ApiError::SchemaLoadFailed {
                path: store.selection_path(),
                message: source.to_string(),
            })?;
        if let Err(source) = fs::write(store.selection_path(), bytes) {
            return self.read_only.error_envelope(
                "provider_connection_store_failed",
                &format!("provider selection could not be saved: {}", source),
                json!({ "path": store.selection_path().display().to_string() }),
            );
        }
        self.read_only.success_envelope(json!({
            "command": "provider_connection_select",
            "selection": selection,
            "cli_reuse": cli_reuse_value(&instance_id, instance_path.is_file().then_some(instance_path.as_path()))
        }))
    }

    pub(crate) fn provider_connection_healthcheck_response(
        &self,
        body: &Value,
    ) -> Result<Value, ApiError> {
        let resolved = match self.read_only.resolve_request_instance(body) {
            Ok(value) => value,
            Err(message) => {
                return self.read_only.error_envelope(
                    "provider_instance_unavailable",
                    &message,
                    json!({}),
                );
            }
        };
        let validation = self
            .read_only
            .validate_provider_instance_value(&resolved.instance)?;
        let health = self
            .read_only
            .provider_healthcheck_value(&resolved.instance, &validation);
        let status = if validation.ok { "success" } else { "failed" };
        let error = if validation.ok {
            Value::Null
        } else {
            json!({
                "code": "provider_instance_invalid",
                "message": "provider healthcheck stopped at validation"
            })
        };
        self.read_only.envelope(
            status,
            json!({
                "command": "provider_connection_healthcheck",
                "provider_instance_path": resolved.path.map(|path| path.display().to_string()),
                "validation": validation.value,
                "healthcheck": health
            }),
            error,
            validation.warnings,
        )
    }

    pub(crate) fn provider_connection_run_request_response(
        &self,
        body: &Value,
    ) -> Result<Value, ApiError> {
        let mode = request_string(body, "mode").unwrap_or_else(|| "dry_run".to_string());
        if mode != "dry_run" && mode != "live" {
            return self.read_only.error_envelope(
                "provider_run_mode_invalid",
                "mode must be dry_run or live",
                json!({ "mode": mode }),
            );
        }
        let resolved = match self.read_only.resolve_request_instance(body) {
            Ok(value) => value,
            Err(message) => {
                return self.read_only.error_envelope(
                    "provider_instance_unavailable",
                    &message,
                    json!({}),
                );
            }
        };
        let validation = self
            .read_only
            .validate_provider_instance_value(&resolved.instance)?;
        if !validation.ok {
            return self.read_only.envelope(
                "failed",
                json!({
                    "command": "provider_connection_run_request",
                    "mode": mode,
                    "validation": validation.value
                }),
                json!({
                    "code": "provider_instance_invalid",
                    "message": "provider run request stopped at validation"
                }),
                validation.warnings,
            );
        }

        let instance_id = required_string(&resolved.instance, "id").unwrap_or_default();
        let provider_id = required_string(&resolved.instance, "provider").unwrap_or_default();
        let registry = match self.read_only.load_builtin_provider_registry() {
            Ok(registry) => registry,
            Err(message) => {
                return self.read_only.error_envelope(
                    "provider_registry_unavailable",
                    &message,
                    json!({ "registry_path": BUILTIN_PROVIDER_REGISTRY }),
                );
            }
        };
        let Some(manifest) = registry.manifest(&provider_id) else {
            return self.read_only.error_envelope(
                "provider_manifest_not_found",
                "provider manifest is not registered",
                json!({ "provider": provider_id }),
            );
        };
        let plan = provider_run_plan_value(manifest, &resolved.instance, resolved.path.as_deref());
        if mode == "dry_run" {
            return self.read_only.success_envelope(json!({
                "command": "provider_connection_run_request",
                "mode": mode,
                "request_status": "planned",
                "provider_execution_performed": false,
                "live_calls_performed": false,
                "plan": plan,
                "cli_reuse": cli_reuse_value(&instance_id, resolved.path.as_deref())
            }));
        }

        if manifest.id() == "provider.fake" || is_local_process_manifest(manifest) {
            return self.enqueue_provider_run_request(body, &instance_id, resolved.path.as_deref());
        }

        let (code, message, status) = if is_local_openai_compatible_manifest(manifest) {
            (
                "daemon_local_ai_live_connector_disabled",
                "local OpenAI-compatible live execution is available only through the explicit CLI provider-instance path in this slice",
                "blocked",
            )
        } else if is_cloud_provider_manifest(manifest) {
            (
                "cloud_live_execution_requires_approval",
                "cloud live execution and paid external calls are blocked without explicit approval",
                "blocked",
            )
        } else {
            (
                "provider_live_execution_unsupported",
                "daemon live execution is not enabled for this provider kind",
                "blocked",
            )
        };
        self.read_only.envelope(
            status,
            json!({
                "command": "provider_connection_run_request",
                "mode": mode,
                "request_status": "blocked",
                "provider_execution_performed": false,
                "live_calls_performed": false,
                "plan": plan,
                "cli_reuse": cli_reuse_value(&instance_id, resolved.path.as_deref())
            }),
            json!({ "code": code, "message": message }),
            Vec::new(),
        )
    }

    fn enqueue_provider_run_request(
        &self,
        body: &Value,
        instance_id: &str,
        instance_path: Option<&Path>,
    ) -> Result<Value, ApiError> {
        let project_id = request_string(body, "project_id").unwrap_or_else(|| "local".to_string());
        let job_id = request_string(body, "job_id").unwrap_or_default();
        if job_id.is_empty() {
            return self.read_only.error_envelope(
                "job_id_required",
                "job_id is required for live daemon queue requests",
                json!({ "project_id": project_id }),
            );
        }
        let Some(queue) = self.read_only.daemon_queue.as_ref() else {
            return self.read_only.error_envelope(
                "daemon_queue_unavailable",
                "daemon queue is not registered for provider run requests",
                json!({}),
            );
        };
        let Some(store) = self.read_only.projects.get(&project_id) else {
            return self.read_only.error_envelope(
                "project_not_registered",
                "project is not registered with the daemon API",
                json!({ "project_id": project_id }),
            );
        };
        let instance_paths = if instance_id == FAKE_DEFAULT {
            Vec::new()
        } else {
            let Some(path) = instance_path else {
                return self.read_only.error_envelope(
                    "provider_instance_path_required",
                    "provider_instance_path is required for non-default daemon queue execution",
                    json!({ "provider_instance_id": instance_id }),
                );
            };
            vec![path.to_path_buf()]
        };
        match queue.enqueue_project_job_with_provider_instances(store, &job_id, instance_paths) {
            Ok(entry) => self.read_only.success_envelope(json!({
                "command": "provider_connection_run_request",
                "mode": "live",
                "request_status": "queued",
                "provider_execution_performed": false,
                "live_calls_performed": false,
                "queue_entry": entry,
                "next_action": "run star-daemon serve --max-ticks to process queued local provider work"
            })),
            Err(source) => self.read_only.error_envelope(
                "daemon_queue_enqueue_failed",
                &source.to_string(),
                json!({
                    "project_id": project_id,
                    "job_id": job_id,
                    "provider_instance_id": instance_id
                }),
            ),
        }
    }
}

#[derive(Debug, Clone)]
struct ProviderConnectionStore {
    config_root: PathBuf,
}

impl ProviderConnectionStore {
    fn instances_dir(&self) -> PathBuf {
        self.config_root.join(PROVIDER_INSTANCES_DIR)
    }

    fn connections_dir(&self) -> PathBuf {
        self.config_root.join(PROVIDER_CONNECTIONS_DIR)
    }

    fn instance_path(&self, instance_id: &str) -> PathBuf {
        self.instances_dir().join(format!("{}.json", instance_id))
    }

    fn selection_path(&self) -> PathBuf {
        self.connections_dir().join(SELECTION_FILE)
    }
}

#[derive(Debug, Clone)]
struct ResolvedProviderInstance {
    instance: Value,
    path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct ProviderInstanceValidation {
    ok: bool,
    value: Value,
    warnings: Vec<String>,
}

impl ApiReadOnlyService {
    fn provider_connection_store(&self) -> Option<ProviderConnectionStore> {
        self.config_root
            .clone()
            .map(|config_root| ProviderConnectionStore { config_root })
    }

    fn repo_root(&self) -> PathBuf {
        repo_root_from_schema_root(&self.schema_root)
    }

    fn load_builtin_provider_registry(&self) -> Result<ProviderRegistry, String> {
        ProviderRegistryLoader::new(self.repo_root())
            .load_registry(BUILTIN_PROVIDER_REGISTRY, &[])
            .map_err(|source| source.to_string())
    }

    fn validate_provider_instance_value(
        &self,
        instance: &Value,
    ) -> Result<ProviderInstanceValidation, ApiError> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let schema_path = self.schema_root.join(PROVIDER_INSTANCE_SCHEMA);
        let schema = load_schema(&schema_path).map_err(|source| ApiError::SchemaLoadFailed {
            path: schema_path,
            message: source.to_string(),
        })?;
        let schema_result = validate_json(instance, &schema);
        if !schema_result.is_ok() {
            errors.extend(schema_result.errors.iter().map(|error| {
                json!({
                    "check": "schema",
                    "location": error.location,
                    "message": error.message,
                    "schema_path": error.schema_path
                })
            }));
        }

        let raw_findings = raw_credential_field_paths(instance);
        if !raw_findings.is_empty() {
            errors.push(json!({
                "check": "credential_raw_value",
                "message": "provider instance contains raw credential field names",
                "paths": raw_findings
            }));
        }

        let provider_id = required_string(instance, "provider").unwrap_or_default();
        let instance_id = required_string(instance, "id").unwrap_or_default();
        let registry = match self.load_builtin_provider_registry() {
            Ok(registry) => Some(registry),
            Err(message) => {
                errors.push(json!({
                    "check": "provider_registry",
                    "message": message
                }));
                None
            }
        };
        let mut manifest_value = Value::Null;
        let mut policy = json!({
            "credential_raw_value_accessed": false,
            "live_calls_performed": false,
            "mock_success": false
        });
        if let Some(registry) = registry.as_ref() {
            match registry.manifest(&provider_id) {
                Some(manifest) => {
                    manifest_value = manifest_summary(manifest, registry, self.repo_root());
                    policy = provider_policy_value(manifest, instance);
                    validate_instance_policy(instance, manifest, &mut errors, &mut warnings);
                }
                None => errors.push(json!({
                    "check": "provider_manifest",
                    "message": "provider manifest is not registered",
                    "provider": provider_id
                })),
            }
        }

        let ok = errors.is_empty();
        Ok(ProviderInstanceValidation {
            ok,
            value: json!({
                "ok": ok,
                "provider_instance_id": instance_id,
                "provider": provider_id,
                "errors": errors,
                "warnings": warnings,
                "manifest": manifest_value,
                "policy": policy
            }),
            warnings,
        })
    }

    fn discover_saved_provider_instances(&self) -> Result<Vec<Value>, ApiError> {
        let Some(store) = self.provider_connection_store() else {
            return Ok(Vec::new());
        };
        let dir = store.instances_dir();
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut instances = Vec::new();
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(source) => {
                return Ok(vec![json!({
                    "status": "invalid",
                    "path": dir.display().to_string(),
                    "error": format!("provider instance directory could not be read: {}", source)
                })]);
            }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let value = match fs::read_to_string(&path)
                .ok()
                .and_then(|content| serde_json::from_str::<Value>(&content).ok())
            {
                Some(value) => value,
                None => {
                    instances.push(json!({
                        "status": "invalid",
                        "path": path.display().to_string(),
                        "error": "provider instance file is not readable JSON"
                    }));
                    continue;
                }
            };
            let validation = self.validate_provider_instance_value(&value)?;
            instances.push(instance_summary(
                &value,
                Some(&path),
                if validation.ok { "valid" } else { "invalid" },
            ));
        }
        instances.sort_by(|left, right| {
            left.get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .cmp(right.get("id").and_then(Value::as_str).unwrap_or(""))
        });
        Ok(instances)
    }

    fn provider_connection_storage_summary(&self) -> Value {
        match self.provider_connection_store() {
            Some(store) => json!({
                "configured": true,
                "config_root": store.config_root.display().to_string(),
                "provider_instances_dir": store.instances_dir().display().to_string(),
                "selection_path": store.selection_path().display().to_string()
            }),
            None => json!({
                "configured": false,
                "provider_instances_dir": null,
                "selection_path": null
            }),
        }
    }

    fn read_provider_selection(&self) -> Value {
        let Some(store) = self.provider_connection_store() else {
            return Value::Null;
        };
        let path = store.selection_path();
        let Some(value) = fs::read_to_string(&path)
            .ok()
            .and_then(|content| serde_json::from_str::<Value>(&content).ok())
        else {
            return Value::Null;
        };
        value
    }

    fn resolve_request_instance(&self, body: &Value) -> Result<ResolvedProviderInstance, String> {
        if body.get("instance").is_some() || body.get("id").is_some() {
            return Ok(ResolvedProviderInstance {
                instance: request_instance_value(body).clone(),
                path: None,
            });
        }
        if let Some(path) = request_string(body, "provider_instance_path") {
            let path = PathBuf::from(path);
            let instance = self.load_instance_value_from_path(&path)?;
            return Ok(ResolvedProviderInstance {
                instance,
                path: Some(path),
            });
        }
        if let Some(instance_id) = request_string(body, "provider_instance_id")
            .or_else(|| request_string(body, "instance_id"))
        {
            if instance_id == FAKE_DEFAULT {
                return Ok(ResolvedProviderInstance {
                    instance: json!({
                        "id": FAKE_DEFAULT,
                        "provider": "provider.fake",
                        "enabled": true,
                        "limits": {
                            "timeout_seconds": 300,
                            "max_parallel_jobs": 1
                        },
                        "routing_tags": ["test"]
                    }),
                    path: None,
                });
            }
            let Some(store) = self.provider_connection_store() else {
                return Err("provider connection storage is not configured".to_string());
            };
            let path = store.instance_path(&instance_id);
            let instance = self.load_instance_value_from_path(&path)?;
            return Ok(ResolvedProviderInstance {
                instance,
                path: Some(path),
            });
        }
        Err("instance, provider_instance_path, or provider_instance_id is required".to_string())
    }

    fn load_instance_value_from_path(&self, path: &Path) -> Result<Value, String> {
        ProviderRegistryLoader::new(self.repo_root())
            .load_instance(path)
            .map(|instance| instance.value().clone())
            .map_err(|source| source.to_string())
    }

    fn provider_healthcheck_value(
        &self,
        instance: &Value,
        validation: &ProviderInstanceValidation,
    ) -> Value {
        let provider_id = required_string(instance, "provider").unwrap_or_default();
        let registry = match self.load_builtin_provider_registry() {
            Ok(registry) => registry,
            Err(message) => {
                return json!({
                    "status": "failed",
                    "healthcheck_mode": "offline_policy",
                    "live_probe_performed": false,
                    "live_calls_performed": false,
                    "error": message
                });
            }
        };
        let Some(manifest) = registry.manifest(&provider_id) else {
            return json!({
                "status": "failed",
                "healthcheck_mode": "offline_policy",
                "live_probe_performed": false,
                "live_calls_performed": false,
                "error": "provider manifest is not registered"
            });
        };
        let status = if !validation.ok {
            "failed"
        } else if is_cloud_provider_manifest(manifest) && !privacy_handoff_approved(instance) {
            "approval_required"
        } else {
            "policy_ready"
        };
        json!({
            "status": status,
            "healthcheck_mode": "offline_policy",
            "connector_scope": connector_scope(manifest),
            "live_probe_performed": false,
            "network_or_process_probe_performed": false,
            "credential_raw_value_accessed": false,
            "live_calls_performed": false,
            "mock_success": false,
            "checks": {
                "schema_valid": validation.ok,
                "manifest_present": true,
                "loopback_policy_valid": !is_local_openai_compatible_manifest(manifest)
                    || local_loopback_policy_error(instance).is_none(),
                "cloud_privacy_handoff_approved": if is_cloud_provider_manifest(manifest) {
                    Value::Bool(privacy_handoff_approved(instance))
                } else {
                    Value::Null
                }
            }
        })
    }
}

fn validate_instance_policy(
    instance: &Value,
    manifest: &ProviderManifest,
    errors: &mut Vec<Value>,
    warnings: &mut Vec<String>,
) {
    if is_local_openai_compatible_manifest(manifest) {
        if let Some(message) = local_loopback_policy_error(instance) {
            errors.push(json!({
                "check": "loopback_policy",
                "message": message
            }));
        }
    }

    if is_cloud_api_manifest(manifest) {
        let credential_ref = instance.get("credential_ref").and_then(Value::as_str);
        if credential_ref.map(allowed_credential_ref) != Some(true) {
            errors.push(json!({
                "check": "credential_ref",
                "message": "cloud API provider requires credential_ref with an allowed reference prefix"
            }));
        }
        if !privacy_handoff_approved(instance) {
            warnings.push(
                "cloud API privacy_handoff_approved is false; live execution remains blocked"
                    .to_string(),
            );
        }
    }

    if is_cloud_cli_manifest(manifest) {
        let credential_ref_ok = instance
            .get("credential_ref")
            .and_then(Value::as_str)
            .map(allowed_credential_ref)
            == Some(true);
        let login_session_ok = instance
            .pointer("/transport_config/auth_mode")
            .and_then(Value::as_str)
            == Some("login_session");
        if !credential_ref_ok && !login_session_ok {
            errors.push(json!({
                "check": "cloud_cli_auth",
                "message": "cloud CLI provider requires credential_ref or transport_config.auth_mode=login_session"
            }));
        }
        if !privacy_handoff_approved(instance) {
            warnings.push(
                "cloud CLI privacy_handoff_approved is false; live execution remains blocked"
                    .to_string(),
            );
        }
    }
}

fn local_loopback_policy_error(instance: &Value) -> Option<String> {
    let Some(base_url) = instance
        .pointer("/endpoint/base_url")
        .and_then(Value::as_str)
    else {
        return Some("local OpenAI-compatible provider requires endpoint.base_url".to_string());
    };
    let Some(rest) = base_url.strip_prefix("http://") else {
        return Some("local OpenAI-compatible endpoint must use http://".to_string());
    };
    let host_port = rest.split_once('/').map_or(rest, |(host, _)| host);
    let (host, port) = host_port
        .rsplit_once(':')
        .map_or((host_port, "80"), |(host, port)| (host, port));
    if host != "127.0.0.1" && host != "localhost" {
        return Some("local OpenAI-compatible endpoint must be loopback-only".to_string());
    }
    if port.parse::<u16>().is_err() {
        return Some("local OpenAI-compatible endpoint port must be a number".to_string());
    }
    if instance
        .pointer("/endpoint/model")
        .and_then(Value::as_str)
        .is_none()
    {
        return Some("local OpenAI-compatible provider requires endpoint.model".to_string());
    }
    None
}

fn provider_policy_value(manifest: &ProviderManifest, instance: &Value) -> Value {
    json!({
        "connector_scope": connector_scope(manifest),
        "live_execution": live_execution_policy(manifest),
        "credential_ref_configured": instance.get("credential_ref").is_some(),
        "credential_raw_value_accessed": false,
        "live_calls_performed": false,
        "mock_success": false
    })
}

fn provider_run_plan_value(
    manifest: &ProviderManifest,
    instance: &Value,
    path: Option<&Path>,
) -> Value {
    json!({
        "provider_instance_id": required_string(instance, "id").unwrap_or_default(),
        "provider": required_string(instance, "provider").unwrap_or_default(),
        "provider_instance_path": path.map(|path| path.display().to_string()),
        "provider_kind": manifest.kind(),
        "provider_transport": manifest.transport(),
        "connector_scope": connector_scope(manifest),
        "live_execution": live_execution_policy(manifest),
        "daemon_scheduler_accepts": manifest.id() == "provider.fake" || is_local_process_manifest(manifest),
        "provider_output_dir": format!(
            ".ai-runs/{{job_id}}/provider-output/{}/",
            required_string(instance, "id").unwrap_or_default()
        ),
        "credential_raw_value_accessed": false,
        "live_calls_performed": false,
        "mock_success": false
    })
}

fn manifest_summary(
    manifest: &ProviderManifest,
    registry: &ProviderRegistry,
    repo_root: PathBuf,
) -> Value {
    let profile = registry.capability_profile(manifest.id());
    json!({
        "id": manifest.id(),
        "name": manifest.value().get("name").and_then(Value::as_str).unwrap_or(manifest.id()),
        "kind": manifest.kind(),
        "transport": manifest.transport(),
        "adapter": manifest.adapter(),
        "manifest_path": repo_relative_path(&repo_root, manifest.path()),
        "capabilities_path": profile.map(|profile| repo_relative_path(&repo_root, profile.path())),
        "routing_tags": profile.map(|profile| profile.routing_tags().to_vec()).unwrap_or_default(),
        "connector_scope": connector_scope(manifest),
        "live_execution": live_execution_policy(manifest)
    })
}

fn instance_summary(instance: &Value, path: Option<&Path>, status: &str) -> Value {
    json!({
        "id": required_string(instance, "id").unwrap_or_default(),
        "provider": required_string(instance, "provider").unwrap_or_default(),
        "enabled": instance.get("enabled").and_then(Value::as_bool).unwrap_or(false),
        "routing_tags": instance
            .get("routing_tags")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        "profile": instance.get("profile").cloned().unwrap_or(Value::Null),
        "endpoint": instance.get("endpoint").cloned().unwrap_or(Value::Null),
        "command": instance.get("command").cloned().unwrap_or(Value::Null),
        "credential_ref": instance.get("credential_ref").cloned().unwrap_or(Value::Null),
        "path": path.map(|path| path.display().to_string()),
        "status": status,
        "credential_raw_value_accessed": false
    })
}

fn cli_reuse_value(instance_id: &str, path: Option<&Path>) -> Value {
    json!({
        "provider": instance_id,
        "provider_instance_path": path.map(|path| path.display().to_string()),
        "args": if let Some(path) = path {
            json!(["--provider", instance_id, "--provider-instance", path.display().to_string().as_str()])
        } else {
            json!(["--provider", instance_id])
        }
    })
}

fn connector_scope(manifest: &ProviderManifest) -> &'static str {
    match manifest.kind() {
        "fake_provider" => "offline_fixture",
        "human_handoff" => "human_handoff",
        "local_process_model" => "local_process",
        "local_openai_compatible_server"
        | "local_anthropic_compatible_server"
        | "remote_self_hosted_model" => "local_ai",
        "cloud_api_model" | "cloud_cli_agent" => "cloud_ai",
        _ => "unknown",
    }
}

fn live_execution_policy(manifest: &ProviderManifest) -> &'static str {
    if manifest.id() == "provider.fake" {
        "not_required"
    } else if is_local_process_manifest(manifest) {
        "daemon_queue_supported"
    } else if is_local_openai_compatible_manifest(manifest) {
        "cli_explicit_provider_instance_only"
    } else if is_cloud_provider_manifest(manifest) {
        "approval_required"
    } else {
        "unsupported"
    }
}

fn is_local_process_manifest(manifest: &ProviderManifest) -> bool {
    manifest.kind() == LOCAL_PROCESS_KIND && manifest.transport() == PROCESS_TRANSPORT
}

fn request_instance_value(body: &Value) -> &Value {
    body.get("instance").unwrap_or(body)
}

fn request_string(body: &Value, key: &str) -> Option<String> {
    body.get(key).and_then(Value::as_str).map(str::to_string)
}

fn required_string(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(str::to_string)
}

fn allowed_credential_ref(value: &str) -> bool {
    ["env:", "keychain:", "secret-manager:", "login-session:"]
        .iter()
        .any(|prefix| value.starts_with(prefix))
}

fn privacy_handoff_approved(instance: &Value) -> bool {
    instance
        .pointer("/transport_config/privacy_handoff_approved")
        .and_then(Value::as_bool)
        == Some(true)
}

fn raw_credential_field_paths(value: &Value) -> Vec<String> {
    let mut findings = Vec::new();
    collect_raw_credential_field_paths(value, "$", &mut findings);
    findings
}

fn collect_raw_credential_field_paths(value: &Value, path: &str, findings: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                let child_path = format!("{}.{}", path, key);
                if RAW_CREDENTIAL_FIELDS
                    .iter()
                    .any(|field| *field == key.to_ascii_lowercase())
                {
                    findings.push(child_path.clone());
                }
                collect_raw_credential_field_paths(child, &child_path, findings);
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                collect_raw_credential_field_paths(
                    child,
                    &format!("{}[{}]", path, index),
                    findings,
                );
            }
        }
        _ => {}
    }
}

fn repo_root_from_schema_root(schema_root: &Path) -> PathBuf {
    schema_root
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn repo_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
