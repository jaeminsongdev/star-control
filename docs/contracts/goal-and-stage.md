# 단계 분해와 실행 계약

## 핵심 원칙

Star-Control은 목표를 아주 작은 할 일 목록으로 분해하지 않는다. 다른 모델이나 다른 실행 방식이 필요한 지점에서 단계를 나눈다.

단계 안의 세부 코딩 순서는 해당 단계에 배정된 Codex가 프로젝트 상황에 맞게 정한다.

## 새 단계가 필요한 조건

다음 중 하나가 달라지면 별도 단계로 나눈다.

- 필요한 결과의 성격
- 필요한 모델 등급
- 필요한 생각 깊이
- 조사, 구현, 검사, 검토처럼 실행 방식이 다름
- 읽기 전용과 수정 작업처럼 권한이 다름
- 비용이 발생함
- 다른 단계가 먼저 끝나야 함
- 별도 작업 복사본에서 동시에 실행할 수 있음
- 실패 시 독립적으로 다시 시도해야 함
- 완료 증거를 따로 판단해야 함

## 한 단계로 묶을 수 있는 조건

다음 조건을 함께 만족하면 작업량이 커도 한 단계로 묶을 수 있다.

- 같은 목적을 가진 변경
- 같은 모델과 생각 깊이로 처리 가능
- 같은 권한 범위
- 같은 검사 방법
- 중간에 별도 승인이나 결과 인계가 필요하지 않음
- 한 번에 실패 원인을 이해하고 다시 시도할 수 있음

## 기본 흐름

    목표
      ↓
    질문과 완료 기준 확정
      ↓
    단계 목록
      ↓
    단계별 배정
      ↓
    계획 승인
      ↓
    실행과 검사
      ↓
    증거와 이어하기 기록

## 핵심 기록

### GoalSpec

사용자가 원하는 최종 결과를 저장한다.

- 목표
- 대상 프로젝트
- 완료 모습
- 반드시 지킬 조건
- 하지 말아야 할 일
- 비용 한도
- 사용자 설정

### StageSpec

한 실행 단계의 계약이다.

- 단계 목적
- 포함하는 일
- 포함하지 않는 일
- 먼저 끝나야 하는 단계
- 동시에 실행 가능한 단계
- 대상 프로젝트와 작업 폴더
- 예상 변경 범위
- 완료 조건
- 실패 시 처리

### RouteDecision

해당 단계에 무엇을 배정했는지와 이유를 저장한다.

- 모델 역할: `sol | terra | luna`
- 실제로 선택된 Codex 모델 ID
- 원시 생각 깊이: `minimal | low | medium | high | xhigh`
- 단계 성격: `plan | execute | review`
- Star-Control 실행 방식: `single | max | ultra`
- 실행 방식 실현 경로: Codex가 직접 제공한 방식인지, Star-Control 관리형 병렬 실행인지
- 실행 시점 CapabilitySnapshot 참조
- 독립 검토 여부
- 권한 설정
- 비용 한도
- 자동 재시도와 승급 규칙

`model_reasoning_effort`에 전달할 수 있는 값과 `max`·`ultra`는 같은 필드가 아니다. 생각 깊이는 Codex의 원시 설정이고, 실행 방식은 Star-Control이 한 단계의 작업 수와 통합 방식을 정하는 내부 계약이다. Plan은 실행 방식이 아니라 단계가 설계·계획을 다루는지 나타내는 성격이다.

### ContextPack

Codex가 현재 단계에 필요한 자료만 모은 묶음이다.

- 프로젝트 규칙
- 관련 문서
- 관련 파일과 찾은 이유
- 현재 변경 상태
- 사용한 ProjectCatalogSnapshot·CodeIndexSnapshot과 partition freshness
- definition·reference·graph 결과의 실제 text·syntax·semantic tier와 limitation
- 앞 단계 결과
- 건드리면 안 되는 범위
- 실행할 검사

### PermissionPlan

어떤 동작을 자동으로 허용하고 무엇을 질문할지 저장한다.

### ValidationPlan

현재 단계에서 실제로 필요한 검사를 저장한다.

### EvidenceBundle

변경과 검사 결과를 저장한다.

### Checkpoint

다음 단계나 재개 작업이 다시 조사하지 않도록 필요한 정보만 저장한다.

### MergePlan

병렬 작업을 어떤 순서와 조건으로 합칠지 저장한다.

### CapabilitySnapshot

실행 시점에 Codex가 실제로 지원한 모델, 생각 깊이, 작업 방식, 권한 기능을 저장한다.

## GoalSpec 상세 계약

| 필드 | 필수 | 의미 |
|---|---|---|
| `goal_id` | 예 | GoalId |
| `title` | 예 | 짧은 사용자 표시 이름 |
| `objective` | 예 | 최종 얻으려는 결과 |
| `success_criteria` | 예 | 기계·사람이 확인할 완료 조건 목록 |
| `projects` | 예 | 하나 이상의 ProjectRef |
| `project_relations` | 예 | 여러 프로젝트의 provider·consumer·선행 관계. 없으면 빈 목록 |
| `included_scope` | 예 | 포함 기능·경로·산출물 |
| `excluded_scope` | 예 | 명시적으로 하지 않을 일 |
| `constraints` | 예 | 기술·안전·운영 제약 |
| `assumptions` | 예 | 현재 사실로 두되 검증 가능한 가정 |
| `questions` | 예 | 질문, 답변, 상태와 결정 영향 |
| `requested_work_profile` | 아니요 | 사용자 지정 작업 Profile ID |
| `budget_limit` | 아니요 | 비용·시간 상한 참조 |
| `status` | 예 | Goal 상태 |
| `created_by` | 예 | 사용자·Codex·system ActorRef |

`success_criteria`의 각 항목은 ID, 설명, 확인 방법과 `required` 여부를 가진다. 단순 문장 목록만 저장하면 최종 Gate가 어느 조건을 확인했는지 연결할 수 없으므로 EvidenceRef를 붙일 수 있어야 한다.

### ProjectRef

ProjectRef는 [공통 개발 관리 계약](development-management.md)의 Project를 Goal 안에서 가리키는 작은 view다. shared identity와 source metadata를 다시 소유하지 않는다.

아래 checkout·Catalog·Index field는 1단계 v2 **목표 계약**이며 현재 P0 schema·제품 구현 완료를 뜻하지 않는다. v2 migration gate 전 persisted v1은 단일 `root_binding_id` 표현만 지원한다.

| 필드 | 필수 | 의미 |
|---|---|---|
| `project_id` | 예 | 경로와 분리된 stable ID |
| `display_name` | 예 | 사용자 표시 이름 |
| `checkout_id` | attached source를 사용할 때 | [Project Catalog·Code Index](project-catalog-and-code-index.md)의 local ProjectCheckout |
| `repository_kind` | 예 | `git \| none` |
| `remote_identity` | 아니요 | secret 없는 host·owner·repo identity |
| `project_revision_id` | source를 고정할 때 | clean base revision |
| `workspace_snapshot_id` | 실제 workspace를 사용할 때 | dirty·untracked를 포함한 current source 관찰 |
| `project_catalog_snapshot_ref` | discovery를 사용했을 때 | Project·Checkout·workspace 관계 근거 |
| `code_index_snapshot_ref` | index를 사용했을 때 | entity·graph·Finding 근거 |
| `freshness` | snapshot ref가 있을 때 | partition별 current·stale·partial·unverified와 limitation |
| `role` | 예 | `primary \| provider \| consumer \| auxiliary` |
| `source_of_truth` | 예 | 이 목표에서 소유하는 계약·자료 |

여러 프로젝트 목표에서는 `project_id`로만 연결하며 한 프로젝트의 절대 경로를 다른 프로젝트 evidence에 복제하지 않는다.

raw root path는 persisted ProjectRef, event, DB와 evidence에 넣지 않는다. Windows adapter가 ProjectCheckout의 `root_binding_id`를 Controller process memory에서 해석한 뒤 ProjectPathRef를 실제 I/O에만 사용한다. P0 ProjectRef v1의 `root_binding_id`·`base_revision`은 1단계 구현 시 v2 `checkout_id`·typed snapshot ref로 migration하며 두 표현의 불일치는 오류다.

### SourceRecord

자료조사와 외부 사실을 Context에 넣을 때는 URL만 남기지 않고 다음을 기록한다.

| 필드 | 의미 |
|---|---|
| `source_id` | SourceRecord ID |
| `source_kind` | `official_doc`, `repository`, `paper`, `issue`, `web_page`, `local_document` |
| `uri` | secret query를 제거한 원본 위치 |
| `title`, `publisher` | 표시 정보 |
| `published_at`, `source_updated_at` | source가 제공한 경우의 시각 |
| `verified_at` | Codex가 실제 확인한 UTC 시각 |
| `authority` | `primary`, `official`, `secondary`, `unknown` |
| `freshness` | 유효 기간과 다시 확인할 조건 |
| `content_hash` | 확인한 내용의 hash |
| `artifact_ref` | 허용된 범위의 redaction된 snapshot |
| `claims` | claim ID, 요약, source 위치와 limitation |

최신성이나 원문 접근을 확인하지 못한 주장은 `verified_at`을 만들지 않고 `unverified` limitation을 둔다. SourceRecord는 외부 내용을 현재 설계 정본으로 자동 승격하지 않는다.

## StageSpec 상세 계약

| 필드 | 필수 | 의미 |
|---|---|---|
| `stage_id` | 예 | StageId |
| `goal_id` | 예 | 상위 GoalId |
| `title` | 예 | 단계 표시 이름 |
| `objective` | 예 | 이 단계가 끝내야 하는 한 가지 결과 |
| `stage_mode` | 예 | `plan \| execute \| review` |
| `work_profile_id` | 예 | 적용할 작업 Profile |
| `project_ids` | 예 | 대상 프로젝트 |
| `included_work` | 예 | 단계 안에서 처리할 책임 |
| `excluded_work` | 예 | 다음 단계 또는 범위 밖 책임 |
| `expected_change_scope` | 예 | 예상 경로·계약·산출물 |
| `dependencies` | 예 | 먼저 완료돼야 하는 StageId |
| `parallel_group` | 아니요 | 함께 실행 가능한 group ID |
| `completion_criteria` | 예 | 단계 전용 완료 조건 |
| `failure_policy` | 예 | retry·replan·block·rollback |
| `route_decision_ref` | 아니요 | 계획 뒤 생성되는 RouteDecision |
| `permission_plan_ref` | 아니요 | 실행 전 확정되는 PermissionPlan |
| `validation_plan_ref` | 아니요 | 실행 전 확정되는 ValidationPlan |
| `state` | 예 | Stage 상태 |

StageSpec은 실행 중 조용히 덮어쓰지 않는다. 범위나 완료 조건이 달라지면 revision을 올리고 `stage.replanned` event에 이전 revision과 이유를 남긴다.

### StageGraph

| 필드 | 의미 |
|---|---|
| `goal_id` | 상위 Goal |
| `plan_revision` | 계획 version |
| `stages` | StageSpec reference 목록 |
| `edges` | `from`, `to`, `relation` |
| `parallel_groups` | 동시 실행 가능 묶음과 한도 |
| `critical_path` | 전체 완료를 막는 경로 |
| `integration_stage_id` | 병렬 결과를 합치는 단계 |

`relation`은 `requires | provides_contract | validates | merges` 중 하나다. cycle은 거부하고, 읽기 전용 조사 외에는 같은 예상 변경 범위가 겹치는 Stage를 같은 parallel group에 둘 수 없다.

## ContextPack 상세 계약

ContextPack은 전체 파일 내용 묶음이 아니라 왜 선택했는지 설명할 수 있는 참조 목록이다.

ProjectCatalogSnapshot·CodeIndexSnapshot·quality/freshness field는 1단계 v2 목표 계약이다. 구현 전에는 존재하지 않는 snapshot을 합성하거나 P0 ScanRun을 semantic index 근거로 승격하지 않는다.

| 필드 | 의미 |
|---|---|
| `context_pack_id` | stable ID |
| `stage_id` | 대상 Stage |
| `project_inputs` | ProjectRef 목록과 각 checkout·revision·workspace snapshot |
| `project_catalog_snapshot_ref` | multi-project·workspace 관계를 선택한 snapshot |
| `code_index_snapshot_refs` | project별 index snapshot과 partition fingerprint |
| `items` | ContextItem 목록 |
| `token_budget` | 전달 가능한 대략적 한도 |
| `omissions` | 제외한 자료와 이유 |
| `quality_summary` | current·stale·partial·unsupported와 tier별 coverage |
| `freshness_policy` | `require_current \| allow_stale_with_warning \| pinned_snapshot` |
| `generated_at` | 생성 시각 |

ContextItem은 `kind`, ProjectId, CheckoutId, 상대 경로 또는 URI, ProjectRevisionId·WorkspaceSnapshotId·content hash, index entity key·tier, 포함 이유, source authority, freshness, sensitivity, 전달 방식과 limitation을 가진다. `sensitivity=secret`인 항목은 원문 전달을 금지하고 존재와 필요한 권한만 표시한다.

default `freshness_policy=require_current`다. current probe가 실패했거나 index가 stale이면 ContextPack이 이전 자료를 current로 복사하지 않는다. explicit pinned snapshot은 과거 분석·재현에만 사용하고 `omissions`와 `quality_summary`에 현재 source와 다른 이유를 남긴다. semantic query가 syntax·text로 fallback하면 실제 `used_tier`를 각 item과 quality summary에 기록한다.

## PermissionPlan과 ApprovalRequest

PermissionPlan은 단계 전체 정책이고 ApprovalRequest는 실제 행동 한 건 또는 원자적으로 묶인 행동 집합이다.

### PermissionPlan 필드

| 필드 | 의미 |
|---|---|
| `permission_plan_id` | 문서 ID |
| `goal_id`, `run_id`, `stage_id` | 적용 범위 |
| `stage_revision` | 판단한 StageSpec revision |
| `policy_profile_ref` | `star.policy-profile.safe-default` 같은 PolicyProfileDescriptor reference |
| `action_policies` | [설정 계약](config-and-catalog.md)의 ActionId별 `auto`, `prompt`, `deny` |
| `path_rules` | 허용·읽기 금지·수정 금지·외부 전달 금지 ProjectPathRef |
| `process_rules` | executable·argument·working directory 제한 |
| `network_rules` | host·operation·download·external write 제한 |
| `environment_rules` | 허용 환경 변수 이름과 SecretRef 종류 |
| `paid_action_rules` | 유료 판정 근거와 불확실할 때의 처리 |
| `external_constraints` | Codex approval·sandbox, 운영체제와 관리자 제한 |
| `effective_config_ref` | field별 provenance가 있는 설정 근거 |
| `scope_hash` | 계획·대상·비용·제약의 canonical hash |
| `expires_at` | capability·계획 변화 전 최대 유효 시각 |

action ID가 없거나 분류할 수 없는 동작은 `default_action`을 사용한다. 외부 제한이 더 강하면 `action_policies`에는 실제 effective 결과를 기록하고 원래 Profile 값은 provenance로 남긴다.

### ApprovalRequest 필드

| 필드 | 의미 |
|---|---|
| `approval_id` | ApprovalId |
| `goal_id`, `run_id`, `stage_id` | 요청 범위 |
| `permission_plan_ref` | 요청을 만든 PermissionPlan revision |
| `action_id` | 설정 계약의 행동 분류 |
| `targets` | 대상 경로·remote·계정·resource |
| `reason` | 필요한 이유 |
| `impact` | 예상 변경, 측정된 사용량과 비용 범위 |
| `reversibility` | 되돌리기 방법과 한계 |
| `evidence_refs` | 판단 근거 |
| `scope_hash` | 승인 당시 계획·대상·비용·제약 hash |
| `requested_by` | 요청을 만든 ActorRef |
| `resolved_by` | 결정을 전달한 ActorRef |
| `decision_reason` | 승인 조건 또는 거부 이유 |
| `expires_at` | 승인 만료 |
| `decision` | `pending \| approved \| denied \| expired \| superseded` |

대상, 비용 범위, scope hash 또는 action ID가 달라지면 기존 승인을 재사용하지 않는다. MCP를 통해 전달된 사용자 결정은 Codex가 전달했다는 actor provenance를 보존한다.

## StageResult 계약

StageResult는 한 Stage revision의 실제 실행과 수용 결과를 묶는다. 실패한 attempt를 지우지 않고 최종 수용 attempt와 함께 참조한다.

| 필드 | 의미 |
|---|---|
| `stage_result_id` | 문서 ID |
| `goal_id`, `run_id`, `stage_id`, `stage_revision` | 대상 Stage |
| `outcome` | `succeeded`, `failed`, `blocked`, `cancelled` |
| `attempts` | AttemptId, RouteDecision, 시작·종료, 결과·오류 reference |
| `accepted_attempt_id` | 결과로 채택한 attempt. 성공일 때 필수 |
| `context_pack_ref` | 실제 입력 자료 |
| `permission_plan_ref` | 실제 권한 범위 |
| `codex_thread_refs` | adapter가 정규화한 opaque thread·turn reference |
| `result_summary` | 완료 조건 기준의 짧은 결과 |
| `output_artifact_refs` | 생성 문서·파일·report |
| `change_set_ref` | 실제 변경 목록 |
| `claim_evidence` | Stage 완료 주장과 evidence reference 대응 |
| `diagnostic_refs` | 실행 중 발견한 문제 |
| `validation_plan_ref`, `gate_decision_ref` | 검사와 Stage gate |
| `cost_record_refs` | 실제 측정 usage |
| `scope_deviations` | 계획과 달라진 범위·이유·승인 |
| `checkpoint_ref`, `handoff_ref` | 이어하기와 결과 전달 |

`outcome=succeeded`는 process 종료만으로 만들 수 없다. required 완료 조건과 Stage Gate를 충족해야 한다. 실패·취소 결과에도 이미 생긴 변경, side effect와 복구 상태를 빠짐없이 둔다.

## MergePlan 상세 계약

| 필드 | 의미 |
|---|---|
| `merge_plan_id` | MergePlan ID |
| `goal_id` | 상위 Goal |
| `base_revision` | 병합 기준 revision |
| `inputs` | stage·worktree·commit·evidence 참조 |
| `order` | 병합 순서와 이유 |
| `overlap_analysis` | 파일·symbol·contract 겹침 |
| `conflict_policy` | 자동·Codex 판단·사용자 판단 경계 |
| `integration_validation_plan_ref` | 병합 후 검사 |
| `rollback_ref` | 병합 실패 복구 기준 |
| `status` | `draft \| ready \| merging \| conflicted \| validated \| failed` |

## 불변식

- Goal에는 하나 이상의 ProjectRef와 하나 이상의 required 성공 조건이 있어야 한다.
- StageGraph의 모든 Stage는 같은 Goal에 속하고 dependency cycle이 없어야 한다.
- 실행 가능한 Stage는 확정 RouteDecision, PermissionPlan과 ValidationPlan을 가져야 한다.
- Stage가 완료되면 result, EvidenceBundle과 Checkpoint reference가 있어야 한다.
- Goal 완료 시 모든 required Stage와 성공 조건이 evidence로 연결돼야 한다.
- 취소·실패·차단 상태를 완료로 변환하려면 새 event와 근거가 필요하다.

## 계획 수정

사용자는 실행 전 단계 내용과 순서를 바꿀 수 있다. 실행 중 새로운 사실이 발견되면 Star-Control은 다음처럼 처리한다.

- 결과에 영향이 없는 세부 변경은 단계 안에서 처리하고 기록한다.
- 같은 성격의 필수 작업은 단계 범위를 넓히고 이유를 기록한다.
- 모델, 권한, 비용, 완료 조건이 달라지면 새 단계로 나눈다.
- 유료 동작이나 설정상 승인 대상이 생기면 중단하고 묻는다.

## 단계 완료

다음 조건을 모두 만족하면 단계가 완료된다.

- 단계 목적에 맞는 변경 또는 조사 결과가 있음
- 필요한 검사가 끝남
- 실패한 검사가 숨겨지지 않음
- 범위 변화가 기록됨
- 다음 단계에 필요한 이어하기 기록이 생성됨
- 비용과 실행 결과가 저장됨
