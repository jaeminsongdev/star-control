//! Deterministic, path-redacted project catalog snapshots.

use std::{collections::BTreeMap, fs, path::Path};

use chrono::Utc;
use serde::Serialize;
use star_contracts::{
    ids::ProjectCatalogSnapshotId,
    index::{
        IndexLimitation, ProjectCatalogCheckoutRef, ProjectCatalogCounts, ProjectCatalogEdge,
        ProjectCatalogEdgeKind, ProjectCatalogProjectRef, ProjectCatalogSnapshot, WorkspaceKind,
        WorkspaceNode,
    },
    management::{Completeness, Project, ProjectCheckout, ProjectPathRef, RepositoryKind},
};
use star_domain::versioned_fingerprint;

use crate::ProjectError;

#[derive(Clone, Debug, Serialize)]
pub struct DiscoveryConfig {
    pub max_roots: usize,
    pub max_workspaces: usize,
    pub recognize_cargo: bool,
    pub nested_ownership: String,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            max_roots: 256,
            max_workspaces: 4_096,
            recognize_cargo: true,
            nested_ownership: "deepest_registered_checkout".to_owned(),
        }
    }
}

pub struct CatalogSnapshotInput<'a> {
    pub project: &'a Project,
    pub checkout: &'a ProjectCheckout,
    pub root: &'a Path,
}

pub fn build_project_catalog_snapshot(
    inputs: &[CatalogSnapshotInput<'_>],
    config: &DiscoveryConfig,
) -> Result<ProjectCatalogSnapshot, ProjectError> {
    if inputs.len() > config.max_roots {
        return Err(ProjectError::ResourceLimit);
    }
    let mut projects = BTreeMap::new();
    let mut checkouts = BTreeMap::new();
    let mut limitations = Vec::new();
    let mut errors = 0_u64;
    for input in inputs {
        if input.checkout.project_id != input.project.project_id
            || checkouts
                .insert(
                    input.checkout.checkout_id.clone(),
                    input.checkout.project_id.clone(),
                )
                .is_some()
        {
            return Err(ProjectError::InvalidManifest);
        }
        let project_content_fingerprint = versioned_fingerprint(
            "star.project-catalog-project-content",
            1,
            &serde_json::json!({
                "project_id":input.project.project_id,
                "identity_scope":input.project.identity_scope,
                "repository_kind":input.project.repository_kind,
                "source_of_truth":input.project.source_of_truth,
                "declaration_fingerprint":input.project.declaration_fingerprint,
                "registration_state":input.project.registration_state,
                "attached_checkout_ids":input.project.attached_checkout_ids,
            }),
        )
        .map_err(|_| ProjectError::Fingerprint)?;
        match projects.insert(
            input.project.project_id.clone(),
            project_content_fingerprint.clone(),
        ) {
            Some(previous) if previous != project_content_fingerprint => {
                return Err(ProjectError::InvalidManifest);
            }
            _ => {}
        }
        if !input.root.is_absolute() || !input.root.is_dir() {
            errors += 1;
            limitations.push(limitation(
                "PROJECT_ROOT_UNAVAILABLE",
                Some(input.checkout.checkout_id.as_str()),
            ));
        }
        if input.project.repository_kind != input.checkout.repository_kind {
            errors += 1;
            limitations.push(limitation(
                "PROJECT_IDENTITY_CONFLICT",
                Some(input.checkout.checkout_id.as_str()),
            ));
        }
    }

    let mut project_refs: Vec<_> = projects
        .into_iter()
        .map(
            |(project_id, content_fingerprint)| ProjectCatalogProjectRef {
                project_id,
                content_fingerprint,
            },
        )
        .collect();
    project_refs.sort_by(|left, right| left.project_id.cmp(&right.project_id));
    let mut checkout_refs: Vec<_> = inputs
        .iter()
        .map(|input| ProjectCatalogCheckoutRef {
            checkout_id: input.checkout.checkout_id.clone(),
            project_id: input.project.project_id.clone(),
            observation_fingerprint: input.checkout.content_fingerprint.clone(),
        })
        .collect();
    checkout_refs.sort_by(|left, right| left.checkout_id.cmp(&right.checkout_id));

    let mut workspace_nodes = Vec::new();
    if config.recognize_cargo {
        for input in inputs.iter().filter(|input| input.root.is_dir()) {
            match cargo_workspace(input) {
                Ok(Some(workspace)) => {
                    workspace_nodes.push(workspace);
                    if workspace_nodes.len() > config.max_workspaces {
                        return Err(ProjectError::ResourceLimit);
                    }
                }
                Ok(None) => {}
                Err(ProjectError::Io | ProjectError::InvalidManifest) => {
                    errors += 1;
                    limitations.push(limitation(
                        "PROJECT_WORKSPACE_MANIFEST_UNAVAILABLE",
                        Some(input.checkout.checkout_id.as_str()),
                    ));
                }
                Err(error) => return Err(error),
            }
        }
    }
    workspace_nodes.sort_by(|left, right| left.workspace_key.cmp(&right.workspace_key));

    let mut project_edges = Vec::new();
    for (index, left) in inputs.iter().enumerate() {
        for right in inputs.iter().skip(index + 1) {
            let relation = if is_strict_child(left.root, right.root) {
                Some((right, left, ProjectCatalogEdgeKind::Nested))
            } else if is_strict_child(right.root, left.root) {
                Some((left, right, ProjectCatalogEdgeKind::Nested))
            } else if left.checkout.repository_kind == RepositoryKind::Git
                && left.checkout.repository_binding_id.is_some()
                && left.checkout.repository_binding_id == right.checkout.repository_binding_id
            {
                Some((left, right, ProjectCatalogEdgeKind::SameRepository))
            } else {
                None
            };
            if let Some((from, to, relation)) = relation {
                let evidence_fingerprint = versioned_fingerprint(
                    "star.project-catalog-edge",
                    1,
                    &serde_json::json!({
                        "from_project_id":from.project.project_id,
                        "from_checkout_id":from.checkout.checkout_id,
                        "to_project_id":to.project.project_id,
                        "to_checkout_id":to.checkout.checkout_id,
                        "relation":relation,
                    }),
                )
                .map_err(|_| ProjectError::Fingerprint)?;
                project_edges.push(ProjectCatalogEdge {
                    from_project_id: from.project.project_id.clone(),
                    from_checkout_id: from.checkout.checkout_id.clone(),
                    to_project_id: to.project.project_id.clone(),
                    to_checkout_id: to.checkout.checkout_id.clone(),
                    relation,
                    evidence_fingerprint,
                });
            }
        }
    }
    project_edges.sort_by(|left, right| {
        (&left.from_checkout_id, &left.to_checkout_id, left.relation).cmp(&(
            &right.from_checkout_id,
            &right.to_checkout_id,
            right.relation,
        ))
    });
    limitations.sort_by(|left, right| (&left.code, &left.scope).cmp(&(&right.code, &right.scope)));
    limitations.dedup();

    let discovery_scope_fingerprint = versioned_fingerprint(
        "star.project-discovery-scope",
        1,
        &checkout_refs
            .iter()
            .map(|item| (&item.checkout_id, &item.observation_fingerprint))
            .collect::<Vec<_>>(),
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    let discovery_config_fingerprint =
        versioned_fingerprint("star.project-discovery-config", 1, config)
            .map_err(|_| ProjectError::Fingerprint)?;
    let completeness = if errors == 0 {
        Completeness::Complete
    } else {
        Completeness::Partial
    };
    let counts = ProjectCatalogCounts {
        roots: inputs.len() as u64,
        projects: project_refs.len() as u64,
        checkouts: checkout_refs.len() as u64,
        workspaces: workspace_nodes.len() as u64,
        excluded: 0,
        errors,
    };
    let content_fingerprint = versioned_fingerprint(
        "star.project-catalog-snapshot-content",
        1,
        &serde_json::json!({
            "discovery_scope_fingerprint":discovery_scope_fingerprint,
            "discovery_config_fingerprint":discovery_config_fingerprint,
            "project_refs":project_refs,
            "checkout_refs":checkout_refs,
            "workspace_nodes":workspace_nodes,
            "project_edges":project_edges,
            "counts":counts,
            "completeness":completeness,
            "limitations":limitations,
        }),
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    let identity_fingerprint = versioned_fingerprint(
        "star.project-catalog-snapshot-id",
        1,
        &serde_json::json!({
            "discovery_scope_fingerprint":discovery_scope_fingerprint,
            "discovery_config_fingerprint":discovery_config_fingerprint,
            "content_fingerprint":content_fingerprint,
        }),
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    Ok(ProjectCatalogSnapshot {
        schema_id: "star.project-catalog-snapshot".to_owned(),
        schema_version: 1,
        project_catalog_snapshot_id: ProjectCatalogSnapshotId::from_fingerprint(
            &identity_fingerprint,
        ),
        discovery_scope_fingerprint,
        discovery_config_fingerprint,
        project_refs,
        checkout_refs,
        workspace_nodes,
        project_edges,
        counts,
        completeness,
        limitations,
        captured_at: Utc::now(),
        content_fingerprint,
    })
}

fn limitation(code: &str, scope: Option<&str>) -> IndexLimitation {
    IndexLimitation {
        code: code.to_owned(),
        scope: scope.map(str::to_owned),
        parameters: BTreeMap::new(),
    }
}

fn is_strict_child(parent: &Path, child: &Path) -> bool {
    parent != child && child.starts_with(parent)
}

fn cargo_workspace(
    input: &CatalogSnapshotInput<'_>,
) -> Result<Option<WorkspaceNode>, ProjectError> {
    let marker = input.root.join("Cargo.toml");
    if !marker.is_file() {
        return Ok(None);
    }
    let source = fs::read_to_string(&marker).map_err(|_| ProjectError::Io)?;
    let document: toml::Value =
        toml::from_str(&source).map_err(|_| ProjectError::InvalidManifest)?;
    let workspace = document.get("workspace").and_then(toml::Value::as_table);
    let package = document.get("package").and_then(toml::Value::as_table);
    if workspace.is_none() && package.is_none() {
        return Ok(None);
    }
    let mut members = workspace
        .and_then(|table| table.get("members"))
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
        .filter_map(|member| ProjectPathRef::parse(member.replace('\\', "/")).ok())
        .collect::<Vec<_>>();
    members.sort();
    members.dedup();
    let evidence_fingerprint = versioned_fingerprint(
        "star.workspace-detection",
        1,
        &serde_json::json!({
            "project_id":input.project.project_id,
            "checkout_id":input.checkout.checkout_id,
            "marker_source":"Cargo.toml",
            "members":members,
        }),
    )
    .map_err(|_| ProjectError::Fingerprint)?;
    Ok(Some(WorkspaceNode {
        workspace_key: format!("cargo:{}", evidence_fingerprint.as_str()),
        kind: WorkspaceKind::Cargo,
        project_id: input.project.project_id.clone(),
        checkout_id: input.checkout.checkout_id.clone(),
        marker_source: ProjectPathRef::parse("Cargo.toml")
            .map_err(|_| ProjectError::InvalidManifest)?,
        member_refs: members,
        evidence_fingerprint,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_contracts::{
        Sha256Hash,
        ids::{CheckoutId, ProjectId, RootBindingId},
        management::{
            CheckoutAttachmentState, CheckoutHeadState, CheckoutKind, IdentityScope,
            RegistrationState,
        },
    };

    fn fixture(
        project_id: ProjectId,
        checkout_id: CheckoutId,
        kind: CheckoutKind,
    ) -> (Project, ProjectCheckout) {
        let fingerprint = Sha256Hash::digest(project_id.as_str().as_bytes());
        let project = Project {
            schema_id: "star.project".to_owned(),
            schema_version: 2,
            project_id: project_id.clone(),
            identity_scope: IdentityScope::Local,
            display_name: "fixture".to_owned(),
            repository_kind: if kind == CheckoutKind::FilesystemRoot {
                RepositoryKind::None
            } else {
                RepositoryKind::Git
            },
            source_of_truth: vec!["source".to_owned()],
            declaration_fingerprint: fingerprint.clone(),
            registration_state: RegistrationState::Attached,
            attached_checkout_ids: vec![checkout_id.clone()],
            latest_revision_id: None,
            latest_workspace_snapshot_id: None,
        };
        let checkout = ProjectCheckout {
            schema_id: "star.project-checkout".to_owned(),
            schema_version: 1,
            checkout_id,
            project_id,
            root_binding_id: Some(RootBindingId::new()),
            repository_kind: project.repository_kind,
            checkout_kind: kind,
            repository_binding_id: None,
            worktree_binding_id: None,
            object_format: None,
            head_state: CheckoutHeadState::Unavailable,
            head_ref: None,
            head_commit_id: None,
            head_tree_id: None,
            upstream_ref: None,
            default_branch_hint: None,
            remote_identity: None,
            attachment_state: CheckoutAttachmentState::Attached,
            last_observed_at: Utc::now(),
            limitations: vec![],
            content_fingerprint: fingerprint,
        };
        (project, checkout)
    }

    #[test]
    fn nested_and_non_git_checkouts_are_distinct_without_persisting_paths() {
        let root = std::env::temp_dir().join(format!("star-catalog-{}", CheckoutId::new()));
        let nested = root.join("nested");
        fs::create_dir_all(&nested).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers=[\"nested\"]\n",
        )
        .unwrap();
        let (parent_project, parent_checkout) = fixture(
            ProjectId::new(),
            CheckoutId::new(),
            CheckoutKind::FilesystemRoot,
        );
        let (child_project, child_checkout) =
            fixture(ProjectId::new(), CheckoutId::new(), CheckoutKind::Clone);
        let snapshot = build_project_catalog_snapshot(
            &[
                CatalogSnapshotInput {
                    project: &parent_project,
                    checkout: &parent_checkout,
                    root: &root,
                },
                CatalogSnapshotInput {
                    project: &child_project,
                    checkout: &child_checkout,
                    root: &nested,
                },
            ],
            &DiscoveryConfig::default(),
        )
        .unwrap();
        assert_eq!(snapshot.completeness, Completeness::Complete);
        assert_eq!(snapshot.workspace_nodes.len(), 1);
        assert!(
            snapshot
                .project_edges
                .iter()
                .any(|edge| edge.relation == ProjectCatalogEdgeKind::Nested)
        );
        let encoded = serde_json::to_string(&snapshot).unwrap();
        assert!(!encoded.contains(&root.to_string_lossy().to_string()));
    }

    #[test]
    fn empty_catalog_is_complete_and_invalid_workspace_is_partial() {
        let empty = build_project_catalog_snapshot(&[], &DiscoveryConfig::default()).unwrap();
        assert_eq!(empty.completeness, Completeness::Complete);
        assert_eq!(empty.counts.projects, 0);

        let root = std::env::temp_dir().join(format!("star-catalog-invalid-{}", CheckoutId::new()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace\n").unwrap();
        let (project, checkout) = fixture(
            ProjectId::new(),
            CheckoutId::new(),
            CheckoutKind::FilesystemRoot,
        );
        let snapshot = build_project_catalog_snapshot(
            &[CatalogSnapshotInput {
                project: &project,
                checkout: &checkout,
                root: &root,
            }],
            &DiscoveryConfig::default(),
        )
        .unwrap();
        assert_eq!(snapshot.completeness, Completeness::Partial);
        assert_eq!(snapshot.counts.errors, 1);
        assert!(
            snapshot
                .limitations
                .iter()
                .any(|item| item.code == "PROJECT_WORKSPACE_MANIFEST_UNAVAILABLE")
        );
    }
}
