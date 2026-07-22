use std::collections::{BTreeMap, BTreeSet, VecDeque};

use star_contracts::{
    GoalId, ProjectId, Sha256Hash,
    development::{
        BundleDependency, BundleParticipantState, CHANGE_BUNDLE_HANDOFF_SCHEMA_ID,
        CHANGE_BUNDLE_SCHEMA_ID, ChangeBundle, ChangeBundleHandoff, ChangeBundleParticipant,
        RemoteOutcome,
    },
};

use crate::{DevelopmentError, fingerprint, placeholder, safe_relative_path, token};

#[derive(Clone, Debug)]
pub struct ParticipantDraft {
    pub participant_id: String,
    pub project_id: ProjectId,
    pub checkout_revision: String,
    pub patch_fingerprint: Sha256Hash,
    pub gate_fingerprint: Sha256Hash,
}

#[derive(Clone, Debug)]
pub enum PublishObservation {
    Verified(Sha256Hash),
    Failed,
    Timeout,
}

pub trait CoordinationPort {
    fn prepare_owned_worktree(
        &mut self,
        participant: &ChangeBundleParticipant,
    ) -> Result<(), DevelopmentError>;

    fn merge_local(
        &mut self,
        participant: &ChangeBundleParticipant,
    ) -> Result<String, DevelopmentError>;

    fn publish_remote(&mut self, bundle: &ChangeBundle) -> PublishObservation;

    fn reconcile_remote(&mut self, bundle: &ChangeBundle) -> PublishObservation;
}

pub fn create_change_bundle(
    bundle_id: &str,
    goal_id: GoalId,
    mut drafts: Vec<ParticipantDraft>,
    mut dependencies: Vec<BundleDependency>,
) -> Result<ChangeBundle, DevelopmentError> {
    if !token(bundle_id, 128) || drafts.is_empty() {
        return Err(DevelopmentError::Invalid);
    }
    drafts.sort_by(|left, right| left.participant_id.cmp(&right.participant_id));
    if drafts
        .windows(2)
        .any(|pair| pair[0].participant_id == pair[1].participant_id)
        || drafts.iter().any(|draft| {
            !token(&draft.participant_id, 128) || !valid_revision(&draft.checkout_revision)
        })
    {
        return Err(DevelopmentError::Conflict);
    }
    dependencies.sort_by(|left, right| {
        (&left.from_participant_id, &left.to_participant_id)
            .cmp(&(&right.from_participant_id, &right.to_participant_id))
    });
    dependencies.dedup_by(|left, right| left == right);
    let ids = drafts
        .iter()
        .map(|draft| draft.participant_id.as_str())
        .collect::<BTreeSet<_>>();
    if dependencies.iter().any(|edge| {
        edge.from_participant_id == edge.to_participant_id
            || !ids.contains(edge.from_participant_id.as_str())
            || !ids.contains(edge.to_participant_id.as_str())
    }) {
        return Err(DevelopmentError::Conflict);
    }
    let merge_order = topological_order(&ids, &dependencies)?;
    let participants = drafts
        .into_iter()
        .map(|draft| ChangeBundleParticipant {
            owned_worktree: format!("worktrees/{bundle_id}/{}", draft.participant_id),
            participant_id: draft.participant_id,
            project_id: draft.project_id,
            checkout_revision: draft.checkout_revision,
            patch_fingerprint: draft.patch_fingerprint,
            gate_fingerprint: draft.gate_fingerprint,
            state: BundleParticipantState::Planned,
            resulting_commit: None,
        })
        .collect();
    seal_bundle(ChangeBundle {
        schema_id: CHANGE_BUNDLE_SCHEMA_ID.to_owned(),
        schema_version: 1,
        bundle_id: bundle_id.to_owned(),
        goal_id,
        revision: 1,
        participants,
        dependencies,
        merge_order,
        remote_outcome: RemoteOutcome::NotRequested,
        remote_snapshot_fingerprint: None,
        limitations: vec![],
        bundle_fingerprint: placeholder(),
    })
}

pub fn run_merge_queue(
    mut bundle: ChangeBundle,
    publish: bool,
    port: &mut dyn CoordinationPort,
) -> Result<ChangeBundle, DevelopmentError> {
    validate_bundle(&bundle)?;
    if bundle
        .participants
        .iter()
        .any(|participant| participant.state != BundleParticipantState::Planned)
    {
        return Err(DevelopmentError::Conflict);
    }
    for participant in &mut bundle.participants {
        if port.prepare_owned_worktree(participant).is_err() {
            participant.state = BundleParticipantState::Blocked;
            bundle.limitations.push(format!(
                "worktree_prepare_failed:{}",
                participant.participant_id
            ));
            bundle.revision += 1;
            return seal_bundle(bundle);
        }
        participant.state = BundleParticipantState::WorktreeReady;
    }
    for participant_id in bundle.merge_order.clone() {
        let predecessor_ids = bundle
            .dependencies
            .iter()
            .filter(|edge| edge.to_participant_id == participant_id)
            .map(|edge| edge.from_participant_id.as_str())
            .collect::<Vec<_>>();
        let prerequisites_met = predecessor_ids.iter().all(|predecessor| {
            bundle.participants.iter().any(|candidate| {
                candidate.participant_id == *predecessor
                    && candidate.state == BundleParticipantState::MergedLocal
            })
        });
        let index = bundle
            .participants
            .iter()
            .position(|participant| participant.participant_id == participant_id)
            .ok_or(DevelopmentError::Conflict)?;
        if !prerequisites_met {
            bundle.participants[index].state = BundleParticipantState::Blocked;
            bundle
                .limitations
                .push(format!("merge_predecessor_unsatisfied:{participant_id}"));
            continue;
        }
        bundle.participants[index].state = BundleParticipantState::MergeQueued;
        match port.merge_local(&bundle.participants[index]) {
            Ok(commit) if valid_revision(&commit) => {
                bundle.participants[index].state = BundleParticipantState::MergedLocal;
                bundle.participants[index].resulting_commit = Some(commit);
            }
            _ => {
                bundle.participants[index].state = BundleParticipantState::Blocked;
                bundle
                    .limitations
                    .push(format!("local_merge_failed:{participant_id}"));
            }
        }
    }
    if bundle
        .participants
        .iter()
        .any(|participant| participant.state != BundleParticipantState::MergedLocal)
    {
        bundle.remote_outcome = RemoteOutcome::NotRequested;
    } else if publish {
        let observation = port.publish_remote(&bundle);
        let final_observation = match observation {
            PublishObservation::Timeout => port.reconcile_remote(&bundle),
            other => other,
        };
        match final_observation {
            PublishObservation::Verified(snapshot) => {
                bundle.remote_outcome = RemoteOutcome::Verified;
                bundle.remote_snapshot_fingerprint = Some(snapshot);
                for participant in &mut bundle.participants {
                    participant.state = BundleParticipantState::RemoteVerified;
                }
            }
            PublishObservation::Failed => {
                bundle.remote_outcome = RemoteOutcome::Failed;
                bundle.limitations.push("remote_publish_failed".to_owned());
            }
            PublishObservation::Timeout => {
                bundle.remote_outcome = RemoteOutcome::OutcomeUnknown;
                bundle
                    .limitations
                    .push("publish_outcome_unknown".to_owned());
                for participant in &mut bundle.participants {
                    participant.state = BundleParticipantState::OutcomeUnknown;
                }
            }
        }
    }
    bundle.revision += 1;
    bundle.limitations.sort();
    bundle.limitations.dedup();
    seal_bundle(bundle)
}

pub fn build_handoff(bundle: &ChangeBundle) -> Result<ChangeBundleHandoff, DevelopmentError> {
    validate_bundle(bundle)?;
    let mut blockers = bundle.limitations.clone();
    let mut commits = Vec::new();
    for participant in &bundle.participants {
        match participant.resulting_commit.as_ref() {
            Some(commit) => commits.push(commit.clone()),
            None => blockers.push(format!("missing_commit:{}", participant.participant_id)),
        }
        if participant.state != BundleParticipantState::RemoteVerified {
            blockers.push(format!(
                "participant_not_remote_verified:{}",
                participant.participant_id
            ));
        }
    }
    commits.sort();
    blockers.sort();
    blockers.dedup();
    let ready = blockers.is_empty() && bundle.remote_outcome == RemoteOutcome::Verified;
    let mut artifacts = bundle
        .participants
        .iter()
        .flat_map(|participant| {
            [
                participant.patch_fingerprint.clone(),
                participant.gate_fingerprint.clone(),
            ]
        })
        .collect::<Vec<_>>();
    artifacts.sort();
    artifacts.dedup();
    let mut handoff = ChangeBundleHandoff {
        schema_id: CHANGE_BUNDLE_HANDOFF_SCHEMA_ID.to_owned(),
        schema_version: 1,
        bundle_id: bundle.bundle_id.clone(),
        bundle_revision: bundle.revision,
        bundle_fingerprint: bundle.bundle_fingerprint.clone(),
        participant_commits: commits,
        artifact_fingerprints: artifacts,
        remote_outcome: bundle.remote_outcome,
        ready,
        blockers,
        handoff_fingerprint: placeholder(),
    };
    handoff.handoff_fingerprint = fingerprint(
        CHANGE_BUNDLE_HANDOFF_SCHEMA_ID,
        &serde_json::json!({
            "bundle_id":handoff.bundle_id,
            "bundle_revision":handoff.bundle_revision,
            "bundle_fingerprint":handoff.bundle_fingerprint,
            "participant_commits":handoff.participant_commits,
            "artifact_fingerprints":handoff.artifact_fingerprints,
            "remote_outcome":handoff.remote_outcome,
            "ready":handoff.ready,
            "blockers":handoff.blockers,
        }),
    )?;
    Ok(handoff)
}

pub fn validate_bundle(bundle: &ChangeBundle) -> Result<(), DevelopmentError> {
    if bundle.schema_id != CHANGE_BUNDLE_SCHEMA_ID
        || bundle.schema_version != 1
        || bundle.revision == 0
        || !token(&bundle.bundle_id, 128)
        || bundle.participants.is_empty()
        || bundle
            .participants
            .iter()
            .any(|participant| !safe_relative_path(&participant.owned_worktree))
    {
        return Err(DevelopmentError::Invalid);
    }
    let ids = bundle
        .participants
        .iter()
        .map(|participant| participant.participant_id.as_str())
        .collect::<BTreeSet<_>>();
    if ids.len() != bundle.participants.len()
        || topological_order(&ids, &bundle.dependencies)? != bundle.merge_order
    {
        return Err(DevelopmentError::Conflict);
    }
    Ok(())
}

fn topological_order(
    ids: &BTreeSet<&str>,
    dependencies: &[BundleDependency],
) -> Result<Vec<String>, DevelopmentError> {
    let mut indegree = ids
        .iter()
        .map(|id| (*id, 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<&str, Vec<&str>>::new();
    for edge in dependencies {
        if edge.from_participant_id == edge.to_participant_id
            || !ids.contains(edge.from_participant_id.as_str())
            || !ids.contains(edge.to_participant_id.as_str())
        {
            return Err(DevelopmentError::Conflict);
        }
        *indegree
            .get_mut(edge.to_participant_id.as_str())
            .ok_or(DevelopmentError::Conflict)? += 1;
        outgoing
            .entry(edge.from_participant_id.as_str())
            .or_default()
            .push(edge.to_participant_id.as_str());
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| *id)
        .collect::<VecDeque<_>>();
    let mut order = Vec::new();
    while let Some(id) = ready.pop_front() {
        order.push(id.to_owned());
        let mut children = outgoing.get(id).cloned().unwrap_or_default();
        children.sort();
        for child in children {
            let degree = indegree.get_mut(child).ok_or(DevelopmentError::Conflict)?;
            *degree -= 1;
            if *degree == 0 {
                let position = ready
                    .iter()
                    .position(|queued| *queued > child)
                    .unwrap_or(ready.len());
                ready.insert(position, child);
            }
        }
    }
    if order.len() != ids.len() {
        return Err(DevelopmentError::Conflict);
    }
    Ok(order)
}

fn seal_bundle(mut bundle: ChangeBundle) -> Result<ChangeBundle, DevelopmentError> {
    validate_bundle(&bundle)?;
    bundle.bundle_fingerprint = fingerprint(
        CHANGE_BUNDLE_SCHEMA_ID,
        &serde_json::json!({
            "bundle_id":bundle.bundle_id,
            "goal_id":bundle.goal_id,
            "revision":bundle.revision,
            "participants":bundle.participants,
            "dependencies":bundle.dependencies,
            "merge_order":bundle.merge_order,
            "remote_outcome":bundle.remote_outcome,
            "remote_snapshot_fingerprint":bundle.remote_snapshot_fingerprint,
            "limitations":bundle.limitations,
        }),
    )?;
    Ok(bundle)
}

fn valid_revision(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakePort {
        publishes: usize,
        reconciles: usize,
        timeout: bool,
        reconcile_verified: bool,
    }

    impl CoordinationPort for FakePort {
        fn prepare_owned_worktree(
            &mut self,
            _participant: &ChangeBundleParticipant,
        ) -> Result<(), DevelopmentError> {
            Ok(())
        }

        fn merge_local(
            &mut self,
            participant: &ChangeBundleParticipant,
        ) -> Result<String, DevelopmentError> {
            Ok(if participant.participant_id == "core" {
                "c".repeat(40)
            } else {
                "d".repeat(40)
            })
        }

        fn publish_remote(&mut self, _bundle: &ChangeBundle) -> PublishObservation {
            self.publishes += 1;
            if self.timeout {
                PublishObservation::Timeout
            } else {
                PublishObservation::Verified(Sha256Hash::digest(b"remote"))
            }
        }

        fn reconcile_remote(&mut self, _bundle: &ChangeBundle) -> PublishObservation {
            self.reconciles += 1;
            if self.reconcile_verified {
                PublishObservation::Verified(Sha256Hash::digest(b"remote"))
            } else {
                PublishObservation::Timeout
            }
        }
    }

    fn draft(id: &str) -> ParticipantDraft {
        ParticipantDraft {
            participant_id: id.to_owned(),
            project_id: ProjectId::new(),
            checkout_revision: "a".repeat(40),
            patch_fingerprint: Sha256Hash::digest(format!("patch-{id}").as_bytes()),
            gate_fingerprint: Sha256Hash::digest(format!("gate-{id}").as_bytes()),
        }
    }

    #[test]
    fn owned_worktrees_merge_in_dag_order_and_verified_remote_makes_handoff_ready() {
        let bundle = create_change_bundle(
            "bundle-1",
            GoalId::new(),
            vec![draft("app"), draft("core")],
            vec![BundleDependency {
                from_participant_id: "core".to_owned(),
                to_participant_id: "app".to_owned(),
            }],
        )
        .unwrap();
        assert_eq!(bundle.merge_order, ["core", "app"]);
        let mut port = FakePort {
            publishes: 0,
            reconciles: 0,
            timeout: false,
            reconcile_verified: false,
        };
        let merged = run_merge_queue(bundle, true, &mut port).unwrap();
        assert_eq!(merged.remote_outcome, RemoteOutcome::Verified);
        assert!(build_handoff(&merged).unwrap().ready);
    }

    #[test]
    fn publish_timeout_is_not_retried_and_unknown_blocks_handoff() {
        let bundle =
            create_change_bundle("bundle-2", GoalId::new(), vec![draft("core")], vec![]).unwrap();
        let mut port = FakePort {
            publishes: 0,
            reconciles: 0,
            timeout: true,
            reconcile_verified: false,
        };
        let merged = run_merge_queue(bundle, true, &mut port).unwrap();
        assert_eq!(port.publishes, 1);
        assert_eq!(port.reconciles, 1);
        assert_eq!(merged.remote_outcome, RemoteOutcome::OutcomeUnknown);
        let handoff = build_handoff(&merged).unwrap();
        assert!(!handoff.ready);
        assert!(
            handoff
                .blockers
                .iter()
                .any(|item| item == "publish_outcome_unknown")
        );
    }

    #[test]
    fn cyclic_bundle_is_rejected_before_port_side_effects() {
        let result = create_change_bundle(
            "cycle",
            GoalId::new(),
            vec![draft("a"), draft("b")],
            vec![
                BundleDependency {
                    from_participant_id: "a".to_owned(),
                    to_participant_id: "b".to_owned(),
                },
                BundleDependency {
                    from_participant_id: "b".to_owned(),
                    to_participant_id: "a".to_owned(),
                },
            ],
        );
        assert!(matches!(result, Err(DevelopmentError::Conflict)));
    }
}
