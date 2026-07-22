use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use star_contracts::{
    ProjectId, Sha256Hash,
    development::{
        ConsumerMigrationPlan, ConsumerMigrationState, ConsumerRewrite, EvidenceCompleteness,
        MANAGED_REGISTRY_SNAPSHOT_SCHEMA_ID, ManagedConsumer, ManagedConsumerState,
        ManagedDeclaration, ManagedDeclarationKind, ManagedLifecycle, ManagedRegistrySnapshot,
    },
};

use crate::{DevelopmentError, fingerprint, placeholder, safe_relative_path, token};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    registry_id: String,
    declarations: Vec<ManifestDeclaration>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestDeclaration {
    declaration_id: String,
    namespace: String,
    kind: ManagedDeclarationKind,
    value: String,
    owner_project_id: ProjectId,
    source_path: String,
    source_sha256: Sha256Hash,
    lifecycle: ManagedLifecycle,
    #[serde(default)]
    aliases: Vec<String>,
}

pub fn load_git_registry(
    manifest_bytes: &[u8],
    git_revision: &str,
    mut consumers: Vec<ManagedConsumer>,
) -> Result<ManagedRegistrySnapshot, DevelopmentError> {
    let text = std::str::from_utf8(manifest_bytes).map_err(|_| DevelopmentError::Invalid)?;
    let manifest: Manifest = toml::from_str(text).map_err(|_| DevelopmentError::Invalid)?;
    if manifest.schema_version != 1
        || !token(&manifest.registry_id, 128)
        || !valid_git_revision(git_revision)
        || manifest.declarations.is_empty()
    {
        return Err(DevelopmentError::Invalid);
    }
    let mut declarations = manifest
        .declarations
        .into_iter()
        .map(|entry| {
            let mut aliases = entry.aliases;
            aliases.sort();
            aliases.dedup();
            ManagedDeclaration {
                declaration_id: entry.declaration_id,
                namespace: entry.namespace,
                kind: entry.kind,
                value: entry.value,
                owner_project_id: entry.owner_project_id,
                source_path: entry.source_path,
                source_sha256: entry.source_sha256,
                lifecycle: entry.lifecycle,
                aliases,
            }
        })
        .collect::<Vec<_>>();
    declarations.sort_by(|left, right| left.declaration_id.cmp(&right.declaration_id));
    validate_declarations(&declarations)?;

    consumers.sort_by(|left, right| {
        (&left.declaration_id, &left.project_id, &left.path).cmp(&(
            &right.declaration_id,
            &right.project_id,
            &right.path,
        ))
    });
    if consumers.windows(2).any(|pair| {
        pair[0].declaration_id == pair[1].declaration_id
            && pair[0].project_id == pair[1].project_id
            && pair[0].path == pair[1].path
    }) {
        return Err(DevelopmentError::Conflict);
    }
    let by_id = declarations
        .iter()
        .map(|declaration| (declaration.declaration_id.as_str(), declaration))
        .collect::<BTreeMap<_, _>>();
    let mut limitations = Vec::new();
    for consumer in &consumers {
        if !safe_relative_path(&consumer.path) {
            return Err(DevelopmentError::Invalid);
        }
        let Some(declaration) = by_id.get(consumer.declaration_id.as_str()) else {
            return Err(DevelopmentError::Conflict);
        };
        let expected_state = if consumer.observed_value == declaration.value {
            ManagedConsumerState::Bound
        } else if declaration.aliases.contains(&consumer.observed_value) {
            ManagedConsumerState::Alias
        } else {
            consumer.state
        };
        if expected_state != consumer.state
            || matches!(
                consumer.state,
                ManagedConsumerState::Unresolved | ManagedConsumerState::Stale
            )
        {
            limitations.push(format!(
                "consumer_unverified:{}:{}",
                consumer.declaration_id, consumer.path
            ));
        }
        if declaration.lifecycle == ManagedLifecycle::Removed
            && matches!(
                consumer.state,
                ManagedConsumerState::Bound | ManagedConsumerState::Alias
            )
        {
            limitations.push(format!(
                "removed_declaration_has_consumer:{}",
                declaration.declaration_id
            ));
        }
    }
    limitations.sort();
    limitations.dedup();
    let completeness = if limitations.is_empty() {
        EvidenceCompleteness::Complete
    } else {
        EvidenceCompleteness::Partial
    };
    let manifest_sha256 = Sha256Hash::digest(manifest_bytes);
    let mut snapshot = ManagedRegistrySnapshot {
        schema_id: MANAGED_REGISTRY_SNAPSHOT_SCHEMA_ID.to_owned(),
        schema_version: 1,
        registry_id: manifest.registry_id,
        git_revision: git_revision.to_owned(),
        manifest_sha256,
        declarations,
        consumers,
        completeness,
        limitations,
        content_fingerprint: placeholder(),
    };
    snapshot.content_fingerprint = snapshot_fingerprint(&snapshot)?;
    Ok(snapshot)
}

pub fn plan_consumer_migration(
    before: &ManagedRegistrySnapshot,
    after: &ManagedRegistrySnapshot,
    declaration_id: &str,
) -> Result<ConsumerMigrationPlan, DevelopmentError> {
    if before.registry_id != after.registry_id
        || before.completeness != EvidenceCompleteness::Complete
        || after.completeness != EvidenceCompleteness::Complete
    {
        return Err(DevelopmentError::Unverified);
    }
    let old = before
        .declarations
        .iter()
        .find(|entry| entry.declaration_id == declaration_id)
        .ok_or(DevelopmentError::Invalid)?;
    let new = after
        .declarations
        .iter()
        .find(|entry| entry.declaration_id == declaration_id)
        .ok_or(DevelopmentError::Invalid)?;
    let mut blockers = Vec::new();
    if old.kind != new.kind
        || old.namespace != new.namespace
        || old.owner_project_id != new.owner_project_id
    {
        blockers.push("declaration_identity_changed".to_owned());
    }
    if new.lifecycle != ManagedLifecycle::Active && new.lifecycle != ManagedLifecycle::Deprecated {
        blockers.push("target_lifecycle_not_consumable".to_owned());
    }
    if old.value != new.value && !new.aliases.contains(&old.value) {
        blockers.push("old_value_not_retained_as_alias".to_owned());
    }
    let mut rewrites = Vec::new();
    for consumer in before
        .consumers
        .iter()
        .filter(|consumer| consumer.declaration_id == declaration_id)
    {
        if matches!(
            consumer.state,
            ManagedConsumerState::Stale | ManagedConsumerState::Unresolved
        ) {
            blockers.push(format!("consumer_not_current:{}", consumer.path));
            continue;
        }
        if old.value != new.value && consumer.observed_value != new.value {
            rewrites.push(ConsumerRewrite {
                project_id: consumer.project_id.clone(),
                path: consumer.path.clone(),
                expected_source_sha256: consumer.source_sha256.clone(),
                before_value: consumer.observed_value.clone(),
                after_value: new.value.clone(),
            });
        }
    }
    rewrites.sort_by(|left, right| {
        (&left.project_id, &left.path).cmp(&(&right.project_id, &right.path))
    });
    blockers.sort();
    blockers.dedup();
    let state = if !blockers.is_empty() {
        ConsumerMigrationState::Blocked
    } else if rewrites.is_empty() {
        ConsumerMigrationState::NoChange
    } else {
        ConsumerMigrationState::Ready
    };
    let mut plan = ConsumerMigrationPlan {
        declaration_id: declaration_id.to_owned(),
        from_snapshot: before.content_fingerprint.clone(),
        to_snapshot: after.content_fingerprint.clone(),
        state,
        rewrites,
        blockers,
        plan_fingerprint: placeholder(),
    };
    plan.plan_fingerprint = fingerprint(
        "star.consumer-migration-plan",
        &serde_json::json!({
            "declaration_id":plan.declaration_id,
            "from_snapshot":plan.from_snapshot,
            "to_snapshot":plan.to_snapshot,
            "state":plan.state,
            "rewrites":plan.rewrites,
            "blockers":plan.blockers,
        }),
    )?;
    Ok(plan)
}

fn validate_declarations(declarations: &[ManagedDeclaration]) -> Result<(), DevelopmentError> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut owned_values = BTreeMap::<String, String>::new();
    for declaration in declarations {
        if !token(&declaration.declaration_id, 160)
            || !token(&declaration.namespace, 128)
            || declaration.value.trim().is_empty()
            || declaration.value.len() > 512
            || !safe_relative_path(&declaration.source_path)
            || !ids.insert(declaration.declaration_id.clone())
            || !names.insert((
                declaration.namespace.clone(),
                declaration.kind,
                declaration.value.clone(),
            ))
            || declaration
                .aliases
                .iter()
                .any(|alias| alias.trim().is_empty())
        {
            return Err(DevelopmentError::Conflict);
        }
        for value in std::iter::once(&declaration.value).chain(declaration.aliases.iter()) {
            if let Some(owner) = owned_values.insert(
                format!("{}:{:?}:{value}", declaration.namespace, declaration.kind),
                declaration.declaration_id.clone(),
            ) && owner != declaration.declaration_id
            {
                return Err(DevelopmentError::Conflict);
            }
        }
    }
    Ok(())
}

fn valid_git_revision(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn snapshot_fingerprint(
    snapshot: &ManagedRegistrySnapshot,
) -> Result<Sha256Hash, DevelopmentError> {
    fingerprint(
        MANAGED_REGISTRY_SNAPSHOT_SCHEMA_ID,
        &serde_json::json!({
            "registry_id":snapshot.registry_id,
            "git_revision":snapshot.git_revision,
            "manifest_sha256":snapshot.manifest_sha256,
            "declarations":snapshot.declarations,
            "consumers":snapshot.consumers,
            "completeness":snapshot.completeness,
            "limitations":snapshot.limitations,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(value: &str, aliases: &str) -> Vec<u8> {
        format!(
            r#"schema_version = 1
registry_id = "shared-contracts"

[[declarations]]
declaration_id = "errors.invalid_input"
namespace = "star.error"
kind = "error_code"
value = "{value}"
owner_project_id = "prj_00000000000000000000000000"
source_path = "src/errors.rs"
source_sha256 = "sha256:{hash}"
lifecycle = "active"
aliases = [{aliases}]
"#,
            hash = "1".repeat(64)
        )
        .into_bytes()
    }

    #[test]
    fn git_manifest_is_canonical_and_consumer_migration_is_exact() {
        let project = ProjectId::parse("prj_00000000000000000000000000").unwrap();
        let consumer = ManagedConsumer {
            declaration_id: "errors.invalid_input".to_owned(),
            project_id: project,
            path: "src/main.rs".to_owned(),
            observed_value: "OLD_ERROR".to_owned(),
            state: ManagedConsumerState::Bound,
            source_sha256: Sha256Hash::digest(b"consumer"),
        };
        let before =
            load_git_registry(&manifest("OLD_ERROR", ""), &"a".repeat(40), vec![consumer]).unwrap();
        let after = load_git_registry(
            &manifest("NEW_ERROR", "\"OLD_ERROR\""),
            &"b".repeat(40),
            vec![],
        )
        .unwrap();
        let plan = plan_consumer_migration(&before, &after, "errors.invalid_input").unwrap();
        assert_eq!(plan.state, ConsumerMigrationState::Ready);
        assert_eq!(plan.rewrites.len(), 1);
        assert_eq!(plan.rewrites[0].after_value, "NEW_ERROR");
    }

    #[test]
    fn stale_consumer_and_alias_omission_block_migration() {
        let project = ProjectId::parse("prj_00000000000000000000000000").unwrap();
        let before = load_git_registry(
            &manifest("OLD_ERROR", ""),
            &"a".repeat(40),
            vec![ManagedConsumer {
                declaration_id: "errors.invalid_input".to_owned(),
                project_id: project,
                path: "src/main.rs".to_owned(),
                observed_value: "OLD_ERROR".to_owned(),
                state: ManagedConsumerState::Stale,
                source_sha256: Sha256Hash::digest(b"stale"),
            }],
        )
        .unwrap();
        assert_eq!(before.completeness, EvidenceCompleteness::Partial);
        let after = load_git_registry(&manifest("NEW_ERROR", ""), &"b".repeat(40), vec![]).unwrap();
        assert!(matches!(
            plan_consumer_migration(&before, &after, "errors.invalid_input"),
            Err(DevelopmentError::Unverified)
        ));
    }
}
