# 이벤트와 상태 계약

## 목적

Star-Control은 현재 상태만 저장하지 않는다. 무엇이 어떤 순서와 이유로 바뀌었는지 EventEnvelope로 남기고, 빠른 조회를 위한 RunSnapshot과 개발 관리 projection을 그 event와 source 관찰에서 만든다. Controller만 상태를 쓰며 CLI, MCP와 향후 Codex 진입점은 application command를 보낼 뿐 DB·상태 파일을 직접 수정하지 않는다.

Project·ScanRun·Finding·ChangePlan의 상세 의미와 repository transaction은 [공통 개발 관리와 로컬 관리 DB 계약](development-management.md), 저장 위치와 보관 원칙은 [상태 기록과 이어하기](../architecture/state-and-artifacts.md), 공통 ID·시간·경로 형식은 [데이터 계약 지도](README.md)가 소유한다.

## 상태 단위

- Goal: 사용자가 요청한 장기 목표
- Run: Goal을 계획부터 완료 판단까지 실행한 한 세대
- Stage: 같은 성격과 완료 조건을 가진 실행 단계
- Attempt: Stage를 실제로 수행한 한 번의 시도
- Effect: 파일·process·Git·외부 서비스에 가한 한 side effect
- ScanRun: 한 WorkspaceSnapshot과 Rule set을 관찰한 실행
- ChangePlan: Finding을 대상으로 한 변경 계획 revision
- PatchSet: source에 적용하기 전후 hash가 고정된 변경 제안

Goal을 retry하거나 완료 후 추가 작업을 시작하면 기존 이력을 바꾸지 않고 새 RunId를 만든다. Stage의 재시도는 새 AttemptId를 만든다.

## Goal 상태

| 상태 | 의미 | 정상 다음 상태 |
|---|---|---|
| `draft` | 목표가 생성됐지만 아직 정리되지 않음 | `clarifying`, `planned`, `cancelled` |
| `clarifying` | 사용자 답이 필요한 질문을 모음 | `planned`, `blocked`, `cancelled` |
| `planned` | StageGraph와 배정 초안이 있음 | `approved`, `running`, `blocked`, `cancelled` |
| `approved` | 현재 계획과 permission 범위가 실행 가능 | `running`, `cancelled` |
| `running` | 하나 이상의 Stage가 실행 중이거나 준비됨 | `paused`, `validating`, `blocked`, `failed`, `cancelled` |
| `paused` | 새 실행을 시작하지 않는 사용자 일시정지 | `running`, `blocked`, `cancelled` |
| `validating` | 목표 또는 통합 검사 중 | `running`, `reviewing`, `blocked`, `failed` |
| `reviewing` | 독립 검토 또는 사용자 판단 중 | `running`, `merging`, `completed`, `blocked` |
| `merging` | worktree·여러 프로젝트 결과 통합 중 | `validating`, `blocked`, `failed` |
| `blocked` | 사용자 결정이나 외부 상태가 필요함 | `planned`, `running`, `cancelled`, `failed` |
| `failed` | 현재 Run의 자동 복구 범위를 넘김 | terminal |
| `cancelled` | 사용자가 현재 Run을 중단함 | terminal |
| `completed` | 완료 조건과 최종 evidence가 충족됨 | terminal |

`planned -> running` 직접 전이는 정책 Profile상 계획 승인이 필요 없고 모든 PermissionPlan이 자동 허용될 때만 가능하다. 그 경우에도 자동 승인 판단 event를 남긴다. `failed`, `cancelled`, `completed` Run은 다시 열지 않으며 새 Run으로 이어간다.

## Stage 상태

| 상태 | 의미 |
|---|---|
| `pending` | 선행 단계가 끝나지 않음 |
| `ready` | 선행 조건과 입력이 충족됨 |
| `running` | Attempt가 실행 중 |
| `paused` | Goal pause 또는 안전 중단으로 멈춤 |
| `validating` | Stage 완료 검사를 수행 중 |
| `reviewing` | 독립 검토나 사용자 판단을 기다림 |
| `merge_ready` | 변경이 완료되어 통합을 기다림 |
| `merging` | 해당 변경을 통합 중 |
| `blocked` | 승인·질문·외부 조건이 필요함 |
| `failed` | Stage의 attempt·escalation 한도를 넘김 |
| `cancelled` | Goal 취소 또는 명시적 Stage 취소 |
| `skipped` | 조건 분기상 실행하지 않기로 결정하고 이유를 기록함 |
| `completed` | 완료 조건·검사·필요한 통합을 충족함 |

`skipped`는 성공을 뜻하지 않는다. StageSpec이 조건부임을 선언하고 GateDecision이 생략을 허용할 때만 terminal 상태로 사용할 수 있다.

## 개발 관리 상태 경계

개발 관리 DB는 event만 저장하는 journal도, 현재 row만 저장하는 cache도 아니다. 다음 세 부류를 구분한다.

| 부류 | 예 | 변경 근거 | 재구축 |
|---|---|---|---|
| source-derived projection | ProjectRevision, WorkspaceSnapshot, Symbol, Finding | Git·Catalog·source와 ScanRun | 동일 입력으로 재scan |
| local operational state | local Suppression·Disposition, ChangePlan, idempotency | application command event | backup·export가 없으면 재구축 불가 |
| evidence index | ValidationResult·GateDecision·ArtifactRef relation | committed event와 `.ai-runs` manifest | artifact가 남아 있으면 제한적으로 reindex |

이 세 부류의 project detail은 ProjectId별 project store에 둔다. global store는 Project directory, cross-project relation과 `CoordinatedOperation`만 소유한다. store마다 독립적인 event sequence·hash chain·revision이 있으며 전체 하이브리드 저장소에 하나의 global revision이 있다고 가정하지 않는다.

source 관찰 batch 자체를 event payload에 inline으로 넣지 않는다. scan generation을 staging한 뒤 finalization transaction에서 다음을 함께 commit한다.

1. ScanRun terminal state와 count·completeness
2. visible generation pointer
3. Finding·Occurrence·Symbol·SymbolReference projection
4. `scan.finished` 또는 `scan.incomplete` event
5. store revision과 idempotency result

중간 batch는 일반 query에서 보이지 않는다. finalization 실패 시 이전 visible generation과 current Finding projection을 유지한다.

## EventEnvelope 계약

EventEnvelope는 append-only다. 수정·삭제하지 않고 잘못된 event는 이를 정정하는 새 event로 보완한다.

| 필드 | 형식 | 의미 |
|---|---|---|
| `schema_id` | string | `star.event` |
| `schema_version` | integer | Envelope version |
| `event_id` | EventId | 재사용하지 않는 ID |
| `stream_kind` | enum | `run`, `management` |
| `stream_id` | typed ID | RunId 또는 ManagementStoreId |
| `sequence` | positive integer | 같은 stream 안에서 빈틈 없이 증가 |
| `occurred_at` | RFC 3339 UTC | Controller가 기록한 시각 |
| `event_type` | stable string | `goal.created` 같은 사건 종류 |
| `event_class` | enum | `domain`, `audit`, `operation` |
| `payload_schema_id` | string | event payload 계약 ID |
| `payload_schema_version` | integer | payload version |
| `actor` | ActorRef | user, controller, MCP, Codex, tool |
| `goal_id`, `run_id` | conditional typed ID | `stream_kind=run`이면 필수 |
| `project_id` | optional ProjectId | management event가 한 project에 속할 때 |
| `store_scope` | optional enum/object | management이면 `global` 또는 `{ project_id }` |
| `store_revision` | optional integer | management transaction 뒤 revision |
| `coordinated_operation_id` | optional typed ID | cross-store command의 global coordination reference |
| `stage_id`, `attempt_id` | optional typed ID | 더 좁은 범위 |
| `entity_revision` | integer | event 적용 뒤 대상 revision |
| `correlation_id` | string | 한 사용자 command와 연결 |
| `causation_id` | optional ID | 이 사건을 유발한 command·event |
| `idempotency_key` | optional string | side effect·command 중복 방지 |
| `previous_event_hash` | optional SHA-256 | 같은 stream의 앞 event hash. 첫 event는 생략 |
| `payload` | typed object | event별 최소 변경 자료 |
| `artifact_refs` | ArtifactRef array | 큰 입력·출력 |
| `event_hash` | SHA-256 | canonical event 내용의 hash |

ActorRef는 `actor_type`, pseudonymous stable ID, 선택적인 실행 중 표시 이름과 인증 출처를 가진다. persisted event에는 OS 사용자 이름·email·개인 절대 경로를 넣지 않는다. Codex가 대화에서 사용자의 말을 전달한 경우 actor는 user로 위조하지 않고 `actor_type=codex`, `asserted_user_intent=true`와 근거 correlation을 남긴다.

Goal·Stage·Validation lifecycle event는 `stream_kind=run`, `stream_id=run_id`다. Project directory·coordination과 project별 scan projection·local decision·migration·backup·retention은 `stream_kind=management`, `stream_id=해당 store_id`다. management event는 Goal 없이 기록할 수 있으며 project store event는 일치하는 `project_id`와 `store_scope`를 요구한다. global event가 project detail payload를 복제하면 안 된다. 서로 다른 stream의 sequence와 hash chain을 섞지 않는다.

### 핵심 event 종류

| 영역 | event type |
|---|---|
| Goal·plan | `goal.created`, `goal.updated`, `clarification.requested`, `clarification.answered`, `plan.created`, `plan.revised`, `goal.state_changed` |
| Stage·route | `stage.ready`, `stage.replanned`, `route.decided`, `stage.started`, `stage.paused`, `stage.resumed`, `stage.completed`, `stage.failed`, `stage.skipped` |
| 승인·정책 | `approval.requested`, `approval.resolved`, `approval.expired`, `policy.blocked` |
| side effect | `effect.requested`, `effect.started`, `effect.completed`, `effect.failed`, `effect.reconciled` |
| 검사·증거 | `validation.planned`, `validation.started`, `validation.finished`, `gate.decided`, `artifact.recorded`, `checkpoint.created` |
| 개발 관리 | `project.registered`, `project.detached`, `workspace.captured`, `scan.started`, `scan.batch_staged`, `scan.finished`, `scan.incomplete`, `scan.failed`, `finding.observed`, `finding.not_observed`, `shared_decisions.projected`, `suppression.changed`, `baseline.changed`, `disposition.changed` |
| 변경 관리 | `change_plan.created`, `change_plan.revised`, `patch.prepared`, `patch.apply_requested`, `patch.applied`, `patch.partially_applied`, `patch.rollback_completed`, `patch.rollback_blocked`, `validation_result.recorded` |
| 병합·완료 | `merge.planned`, `merge.started`, `merge.conflicted`, `merge.completed`, `handoff.created`, `goal.completed` |
| 운영 | `controller.recovered`, `management.store_opened`, `management.integrity_failed`, `management.backup_created`, `management.migration_started`, `management.migration_finished`, `management.rebuilt`, `management.coordination_prepared`, `management.participant_committed`, `management.coordination_completed`, `management.coordination_blocked`, `management.outcome_unknown`, `config.changed`, `capability.refreshed`, `tool.package_candidate_detected`, `tool.package_candidate_rejected`, `tool.registry_published`, `tool.executable_changed`, `tool.trust_changed`, `tool.invocation_queued`, `tool.process_created`, `tool.cancel_requested`, `tool.invocation_finished`, `tool.outcome_unknown`, `ipc.auth_failed`, `ipc.key_rotated`, `cost.recorded`, `retention.applied` |

Registry event는 source 경로 원문 대신 redacted source ID, 이전·새 revision, package ID, descriptor hash, executable identity hash와 진단 reference를 남긴다. `tool.package_candidate_rejected`는 활성 last-known-good package를 삭제했다는 뜻이 아니며 candidate 거부와 현재 active revision을 함께 기록한다. `tool.registry_published`만 active `registry_revision`을 증가시킨다.

상태 변경 event는 `from`, `to`, `reason_code`, 사용자용 이유와 관련 gate·approval reference를 가진다. 문자열 이유만 보고 상태를 복구하지 않는다.

## RunSnapshot 계약

RunSnapshot은 마지막으로 commit된 Event sequence까지 접은 조회용 문서다.

| 필드 | 의미 |
|---|---|
| `goal_id`, `run_id` | snapshot 대상 |
| `goal_revision`, `goal_status` | 현재 Goal 상태 |
| `goal_spec_ref`, `stage_graph_ref` | 현재 계획 계약과 revision |
| `stage_states` | StageId별 상태, revision, 현재 Attempt |
| `active_attempts` | 실행 중인 Codex·tool과 heartbeat |
| `pending_approvals` | 아직 유효한 ApprovalRequest |
| `open_effects` | requested 뒤 완료·실패가 확정되지 않은 effect |
| `validation_state`, `merge_state` | 현재 gate와 merge 요약 |
| `latest_checkpoint_refs` | Stage별 최신 Checkpoint |
| `config_ref`, `catalog_snapshot_ref`, `capability_snapshot_ref` | 실행 판단의 기준 |
| `artifact_index_ref` | evidence manifest |
| `last_sequence`, `last_event_hash` | 반영한 event 경계 |
| `snapshot_completeness` | `complete`, `rebuilding`, `degraded` |

Snapshot은 직접 patch하지 않는다. command handler가 repository transaction 안에서 새 event와 projection revision을 commit한 뒤 reducer가 새 snapshot을 만든다. 불일치하면 기록된 event에서 Goal 상태를 재구축하고, source-derived 개발 관리 projection은 정본 source를 다시 scan한다. local-only decision을 source scan으로 추측해 만들지 않는다.

## OperationSnapshot 계약

오래 걸리는 IPC·MCP command는 Goal 상태와 별도로 operation 진행을 조회할 수 있다.

| 필드 | 의미 |
|---|---|
| `operation_id` | 비동기 operation ID |
| `command`, `correlation_id` | 시작 command와 추적 ID |
| `goal_id`, `run_id`, `stage_id` | 관련 범위 |
| `status` | `received`, `resolving`, `approval_wait`, `queued`, `starting`, `running`, `cancelling`, `succeeded`, `failed`, `cancelled`, `denied`, `expired`, `outcome_unknown` |
| `accepted_at`, `started_at`, `finished_at` | lifecycle 시각 |
| `progress` | 결정적인 total이 있을 때의 completed·total·unit |
| `cancellable` | 현재 안전한 취소가 가능한지 |
| `last_heartbeat_at` | worker 생존 확인 시각 |
| `result_ref` | 성공 결과 계약 reference |
| `error` | 실패 ErrorEnvelope |
| `latest_event_sequence` | 상태 근거 event 경계 |

`outcome_unknown`은 실패 원인이 아니라 side effect 결과를 확정할 수 없다는 뜻이다. reconciliation 전에는 자동 재실행하지 않는다. 정확한 전이는 [MCP 구현 동결 계약](mcp-implementation-contract.md#invocationoperation-상태기계)을 따른다.

## Checkpoint 계약

Checkpoint는 긴 Stage를 안전하게 이어가기 위한 최소 자료다.

- 소속 Goal, Run, Stage, Attempt와 Stage revision
- checkpoint를 만든 이유와 시각
- 이미 완료된 작업과 결과 ArtifactRef
- 현재 workspace·worktree revision과 변경 fingerprint
- 아직 열린 effect와 취소·재실행 가능 여부
- 남은 작업, 다음 command와 완료 조건
- 다시 읽어야 할 ContextPack reference
- 사용한 RouteDecision, EffectiveConfig와 CapabilitySnapshot
- 재개 전 검사할 precondition과 만료 조건

Checkpoint는 전체 대화와 전체 로그를 복사하지 않는다. workspace fingerprint나 precondition이 달라지면 자동 재개하지 않고 새 계획 또는 사용자 판단으로 보낸다.

## Handoff 계약

Handoff는 Codex 교체, Stage 종료와 최종 인계에 사용한다.

| 필드 | 의미 |
|---|---|
| `handoff_kind` | `stage`, `review`, `recovery`, `final` |
| `from_actor`, `intended_role` | 작성 주체와 다음 담당 역할 |
| `scope` | Goal·Stage·revision |
| `objective` | 현재 목표와 완료 조건 |
| `completed` | 끝난 일과 evidence reference |
| `remaining` | 남은 일과 우선순위 |
| `decisions` | 바꾸지 말아야 할 결정과 근거 |
| `protected_scope` | 건드리면 안 되는 경로·외부 상태 |
| `open_questions`, `risks` | 미해결 사항 |
| `next_validation` | 다음에 실행할 필수 검사 |
| `recommended_route` | 다음 단계의 역할·생각 깊이·실행 방식 hint |
| `checkpoint_ref` | 이어하기 기준 |

Handoff의 요약은 evidence를 대체하지 않으며 모든 완료 주장은 ArtifactRef나 event 구간으로 역추적할 수 있어야 한다.

## 쓰기·동시성·복구 규칙

1. Controller 한 process만 management repository, event와 snapshot을 쓴다. current-user 단일 writer lease를 얻지 못하면 두 번째 Controller는 시작하지 않는다.
2. 모든 mutating command는 expected store revision·version vector·exact source hash·승인된 plan fingerprint 중 command 의미에 맞는 stale-write precondition을 받는다. 현재 값이 다르면 `STATE_REVISION_CONFLICT`로 거부하고 최신 상태를 돌려준다.
3. 재시도 가능한 command는 `idempotency_key`가 필수다. 같은 key와 같은 payload는 이전 결과를 돌려주고, payload가 다르면 충돌이다.
4. 큰 artifact는 먼저 임시 위치에 쓰고 redaction·size·hash를 검증한 뒤 `.ai-runs`에서 안전하게 교체한다. 이후 이를 참조하는 event와 projection을 같은 project store transaction으로 commit한다.
5. event, idempotency record, store revision과 현재 projection은 **한 logical store 안에서** 한 repository transaction이다. 여러 store를 바꾸면 global prepared operation과 project participant receipt를 사용하고 완료 전 성공으로 표시하지 않는다. `.ai-runs`의 JSONL·manifest view는 commit 뒤 생성하는 파생 export이며 DB와 충돌하면 재생성한다.
6. DB event chain, exported JSONL 마지막 행 또는 artifact hash가 맞지 않으면 조용히 버리지 않고 격리, Diagnostic과 recovery event를 만든다.
7. 외부 side effect는 실행 전에 `effect.requested`를 commit하고 안정 idempotency key를 전달한다. 종료 뒤 completed 또는 failed를 기록한다.
8. 시작 때 열린 effect를 조회·대조할 수 있으면 reconciliation하고, 확인할 수 없으면 중복 실행하지 않고 `blocked`로 둔다.
9. 시스템 시계 역행이 sequence 순서를 바꾸지 않는다. 정렬은 sequence를 사용하고 timestamp는 표시·진단용이다.
10. CLI·MCP handler와 향후 Codex entry adapter는 repository handle을 소유하지 않는다. 모든 query·mutation은 같은 application service를 통한다.

## 상태 전이 검증

- 허용되지 않은 `from -> to` 전이는 오류이며 event를 쓰지 않는다.
- 상태 전이와 함께 필요한 GateDecision, ApprovalRequest, Checkpoint가 있는지 검사한다.
- Stage 선행 조건은 StageGraph의 현재 revision에서 계산한다.
- Goal 완료 전 모든 필수 Stage가 `completed` 또는 정당한 `skipped`인지, 열린 effect와 pending approval이 없는지 확인한다.
- 취소는 이미 일어난 side effect를 숨기지 않는다. 취소 시점과 rollback 여부를 EvidenceBundle에 남긴다.
