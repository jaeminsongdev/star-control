# 데이터 계약 지도

## 목적

이 폴더는 Star-Control의 Package, 실행 파일, 상태 파일, MCP와 local IPC가 공유하는 데이터 의미를 정의한다. 구현 언어의 내부 구조가 아니라 저장·전달·검증되는 안정 계약이 기준이다.

0단계 공통 개발 관리 의미는 [공통 개발 관리와 로컬 관리 DB 계약](development-management.md), [ADR-0006](../decisions/ADR-0006-공통-개발-관리와-로컬-관리-DB-경계.md)과 [ADR-0007](../decisions/ADR-0007-P0-하이브리드-저장소와-운영-정책.md)에 확정했다. P0 inventory의 관리 계약 19개와 공통 GateDecision·ArtifactRef를 합친 21개 persisted type은 `star-contracts` type, generated JSON Schema, minimal/full/invalid/future fixture와 fingerprint golden으로 구현했다. 전체 P0 완료 여부는 [최종 구현 로드맵](../roadmap/final-implementation.md)과 `PLANS.md`의 검증 근거로만 판정한다. MCP 부분의 exact 구현 값은 [MCP 구현 동결 계약](mcp-implementation-contract.md), [ToolPackageManifest Reference](tool-package-manifest-reference.md), [Windows Tool Runtime](../architecture/windows-tool-runtime.md)과 [MCP 검증 행렬](../testing/mcp-verification-matrix.md)에 동결됐다. MCP Gateway·IPC·Registry·외부 EXE Runtime 범위의 type, generated Schema, fixture와 제품 코드는 구현됐으며 현재 구현·외부 gate 판정은 [MCP 완료 감사](../testing/mcp-completion-audit.md)에 분리해 기록한다. 이 상태 문구는 나머지 Star-Control 계약까지 구현됐다는 뜻이 아니다.

## 계약 원칙

1. 같은 직렬화 type은 `star-contracts`에서 한 번만 정의한다.
2. 모든 저장·전달 문서는 `schema_id`와 `schema_version`을 가진다.
3. 식별자, 시간, 경로, 상태와 오류 표현을 계약마다 새로 만들지 않는다.
4. 상태 snapshot과 관리 DB projection은 빠른 조회용이고 EventEnvelope가 기록된 변경 이력의 근거다. source-derived 사실은 Git 정본과 동일 입력의 재scan으로 다시 계산할 수 있어야 한다.
5. 원문 log와 큰 결과는 계약 안에 넣지 않고 hash가 있는 ArtifactRef로 연결한다.
6. secret, token, 사용자 이름, 개인 절대 경로와 민감 literal 원문은 어떤 persisted 계약에도 허용하지 않는다.
7. 확인된 사실, 추정, 미확인과 사용자 결정을 서로 다른 상태로 표현한다.
8. unknown field를 조용히 삭제하지 않고 version 정책에 따라 보존 또는 거부한다.
9. 외부 protocol의 변동 필드는 adapter에서 정규화하고 core 계약에 그대로 노출하지 않는다.
10. 계약 변경에는 valid, invalid, old-version fixture와 migration 판단이 따라야 한다.

## 공통 Envelope

기계가 생성해 장기 저장하는 domain 최상위 문서는 다음 공통 필드를 가진다.

| 필드 | 형식 | 규칙 |
|---|---|---|
| `schema_id` | stable string | 예: `star.goal-spec` |
| `schema_version` | positive integer | 계약별 독립 증가 |
| `document_id` | typed ID | 같은 문서의 stable identity |
| `revision` | non-negative integer | 내용 갱신마다 증가 |
| `created_at` | RFC 3339 UTC | 최초 생성 시각 |
| `updated_at` | RFC 3339 UTC | 현재 revision 생성 시각 |
| `producer` | ProducerRef | 제품 version과 component |
| `extensions` | map | 명시적으로 허용된 namespace만 사용 |

EventEnvelope와 진단처럼 append-only인 자료는 `revision` 대신 `sequence`와 instance ID를 사용한다. IPC·MCP message, ErrorEnvelope와 내부 command는 각 transport envelope를 따르며 전체 document metadata 대신 schema version, 요청·상관 ID와 시간을 가진다. 이 자료를 장기 저장하면 ArtifactRef 또는 versioned domain document로 감싼다.

사람이 직접 편집하는 StarConfig와 Catalog descriptor는 예외다. 파일 역할에서 `schema_id`가 정해지고 `schema_version` 또는 `format_version`을 명시하며, Controller가 읽을 때 origin·hash가 있는 versioned snapshot으로 변환한다. 사람이 timestamp와 revision을 손으로 관리하게 하지 않는다.

## 공통 형식

### 식별자

| 종류 | 예시 | 규칙 |
|---|---|---|
| ProjectId | `prj_01J...` | 경로나 저장소 이름과 분리된 stable ID |
| ProjectRevisionId | `prv_<base32-sha256>` | Project의 immutable source revision identity |
| WorkspaceSnapshotId | `wsp_<base32-sha256>` | 실제 관찰한 workspace byte·scope identity |
| ScanRunId | `scn_01J...` | 한 scan 실행 instance |
| FindingId | `fnd_<base32-sha256>` | Rule의 stable finding identity |
| OccurrenceId | `occ_<base32-sha256>` | snapshot·location·source hash에 고정된 관찰 identity |
| CanonicalSourceId | `src_<base32-sha256>` | Project 안의 source identity |
| SymbolId | `sym_<base32-sha256>` | source-derived symbol identity |
| SymbolReferenceId | `srf_<base32-sha256>` | source-derived reference edge identity |
| SuppressionId | `sup_01J...` | shared 또는 local suppression decision |
| BaselineId | `bas_01J...` | Finding set 기준 |
| DispositionId | `dsp_01J...` | Finding triage decision |
| ChangePlanId | `cpl_01J...` | local change plan |
| PatchSetId | `pat_01J...` | immutable patch proposal |
| ValidationResultId | `vrs_01J...` | normalized validation result |
| ManagementStoreId | `mst_01J...` | local store generation identity |
| GoalId | `gol_01J...` | 사용자 목표 하나 |
| RunId | `run_01J...` | 목표의 한 실행 세대 |
| StageId | `stg_01J...` | 단계 계획 node |
| AttemptId | `att_01J...` | 단계 실행 시도 |
| ArtifactId | `art_01J...` | 저장 artifact |
| EventId | `evt_01J...` | append-only event |
| ApprovalId | `apr_01J...` | 한 승인 요청 |
| DiagnosticId | `dia_01J...` | 한 실행에서 발생한 진단 instance |
| OperationId | `opn_01J...` | 비동기 application command |
| RequestId | `req_01J...` | MCP·IPC·외부 process 요청 |
| ToolTrustId | `trt_01J...` | 외부 tool code trust record |
| ToolCacheId | `trc_01J...` | durable last-known-good entry |
| TaskInvocationId | `inv_01J...` | shell 재해석 없는 한 실행 요청 |
| ValidationRunId | `val_01J...` | 한 검사 실행 |
| GateId | `gat_01J...` | 한 완료·검토·차단 판단 |
| EvidenceBundleId | `evb_01J...` | 한 실행의 기계 판독 증거 묶음 |
| WaiverId | `wav_01J...` | 사용자가 승인한 한 예외 |
| ChangeSetId | `chg_01J...` | 한 시점의 변경 집합 |
| SourceId | `src_01J...` | 확인한 외부·로컬 근거 |
| EvaluationRunId | `eva_01J...` | 한 비교 평가 실행 |
| ReleaseId | `rel_01J...` | 한 release 후보 |

ID는 대소문자를 구분하는 ASCII string이며 재사용하지 않는다. `<base32-sha256>`은 전체 32-byte digest의 lowercase RFC 4648 base32 52자이고, `01J...` 표기는 uppercase Crockford ULID 26자다. 사람이 붙이는 제목과 branch 이름은 ID가 아니다.

### 시간과 기간

- 절대 시각은 UTC RFC 3339 string으로 기록한다.
- duration과 timeout은 suffix 없는 숫자를 피하고 `duration_ms`처럼 단위를 필드 이름에 넣는다.
- 사용자 표시에서만 local timezone으로 변환한다.
- 아직 끝나지 않은 종료 시각은 필드를 생략하고 빈 문자열을 사용하지 않는다.

### 경로와 위치

- 프로젝트 내부 파일은 ProjectId와 slash 기반 상대 경로를 사용한다.
- 외부 경로가 반드시 필요하면 LocalPathRef로 분리하고 기본 report에서 가린다.
- `..`로 root를 벗어나는 상대 경로, NUL과 정규화되지 않은 device path를 거부한다.
- line과 column은 1부터 시작하고 끝 위치는 exclusive로 통일한다.

### 선택 값과 삭제

- 값이 없으면 필드를 생략한다. `null`, 빈 문자열과 빈 object를 같은 뜻으로 섞지 않는다.
- 설정에서 상속 값을 지울 때는 필드별 `clear` 또는 `remove` 계약을 사용하고 TOML의 임의 sentinel을 만들지 않는다.
- enum은 lowercase snake_case를 사용하며 unknown 값을 임의로 기본값에 대응하지 않는다.

### 공통 reference와 값 type

| type | 필수 내용 | 규칙 |
|---|---|---|
| `DocumentRef` | `schema_id`, `document_id`, `revision`, `sha256` | revision과 hash를 함께 고정 |
| `CatalogRef` | `catalog_id`, `format_version`, `item_version`, `sha256` | 같은 ID의 내용 바뀜 방지 |
| `ProducerRef` | `component`, `product_version`, `build_id`, `platform` | 생성 주체 추적 |
| `ActorRef` | `actor_type`, pseudonymous `actor_id`, optional ephemeral `display_name`, `auth_source` | persisted form에는 OS 사용자 이름·email을 넣지 않고 user 의도를 전달한 actor와 user 자체를 구분 |
| `ProjectPathRef` | `project_id`, slash 기반 상대 `path`, `path_kind` | project root 이탈 금지 |
| `LocalPathRef` | opaque local anchor와 redacted label | raw 절대 경로는 Controller process memory에서만 허용하고 persisted form은 root binding·relative identity만 사용 |
| `LocationRef` | ProjectPathRef, 1-based start, exclusive end, optional symbol | 진단 위치 공통 형식 |
| `Money` | decimal string `amount`, ISO `currency`, `price_source_ref` | binary float 금지, 검증된 금액만 기록 |
| `Completeness` | `complete`, `partial`, `unverified` | 미실행·미확인을 성공과 분리 |
| `Sensitivity` | `public`, `internal`, `confidential`, `secret` | `secret` 원문 직렬화 금지 |

DocumentRef가 가리키는 내용이 바뀌면 같은 revision을 재사용하지 않는다. 사람이 읽는 이름, 파일 경로와 외부 system ID는 Star-Control typed ID를 대신하지 않는다.

## 계약 Inventory

현재 Inventory는 68개 계약 항목이다. `IpcRequest·Response`와 고정 MCP surface처럼 한 행이 여러 generated Schema를 소유할 수 있으므로 이 수는 실제 `.schema.json` 파일 수와 같다는 뜻이 아니다. 0단계의 관리 계약 19개와 공통 `GateDecision`·`ArtifactRef`를 합친 21개 persisted type은 Rust type, generated Schema와 fixture까지 구현했으며 전체 P0 완료 여부는 별도 검증 근거로 판정한다.

| 계약 | Schema ID | 소유 문서 | 저장·전달 위치 |
|---|---|---|---|
| GoalSpec | `star.goal-spec` | [목표·단계](goal-and-stage.md) | Goal state, MCP·IPC |
| ProjectRef | `star.project-ref` | [목표·단계](goal-and-stage.md) | Goal·Context·multi-project |
| Project | `star.project` | [공통 개발 관리](development-management.md) | Git 선언·관리 DB projection |
| ProjectRevision | `star.project-revision` | [공통 개발 관리](development-management.md) | 관리 DB·scan input |
| WorkspaceSnapshot | `star.workspace-snapshot` | [공통 개발 관리](development-management.md) | 관리 DB·artifact manifest |
| ScanRun | `star.scan-run` | [공통 개발 관리](development-management.md) | 관리 DB·scan evidence |
| Rule | `star.rule` | [공통 개발 관리](development-management.md) | Git·Catalog 선언, resolved snapshot |
| Finding | `star.finding` | [공통 개발 관리](development-management.md) | 관리 DB projection |
| Occurrence | `star.occurrence` | [공통 개발 관리](development-management.md) | 관리 DB·evidence reference |
| Symbol | `star.symbol` | [공통 개발 관리](development-management.md) | 관리 DB derived index |
| SymbolReference | `star.symbol-reference` | [공통 개발 관리](development-management.md) | 관리 DB derived edge |
| CanonicalSource | `star.canonical-source` | [공통 개발 관리](development-management.md) | Project source identity |
| Suppression | `star.suppression` | [공통 개발 관리](development-management.md) | Git shared 선언 또는 local DB state |
| Baseline | `star.baseline` | [공통 개발 관리](development-management.md) | Git shared 선언 또는 local DB state |
| Disposition | `star.disposition` | [공통 개발 관리](development-management.md) | local triage state |
| ChangePlan | `star.change-plan` | [공통 개발 관리](development-management.md) | local application state |
| PatchSet | `star.patch-set` | [공통 개발 관리](development-management.md) | 관리 DB summary·`.ai-runs` diff |
| ChangeRecipe | `star.change-recipe` | [공통 개발 관리](development-management.md) | Git·Catalog 선언 |
| ValidationResult | `star.validation-result` | [공통 개발 관리](development-management.md) | 관리 DB·evidence |
| ManagementStoreStatus | `star.management-store-status` | [공통 개발 관리](development-management.md) | Controller lifecycle query |
| CoordinatedOperation | `star.coordinated-operation` | [공통 개발 관리](development-management.md) | global store·project participant receipt |
| SourceRecord | `star.source-record` | [목표·단계](goal-and-stage.md) | Context·자료조사 evidence |
| StageSpec | `star.stage-spec` | [목표·단계](goal-and-stage.md) | plan, stage state |
| StageGraph | `star.stage-graph` | [목표·단계](goal-and-stage.md) | plan |
| StageResult | `star.stage-result` | [목표·단계](goal-and-stage.md) | stage output·handoff |
| ContextPack | `star.context-pack` | [목표·단계](goal-and-stage.md) | stage artifact |
| RouteDecision | `star.route-decision` | [라우팅](routing.md) | stage route |
| CapabilitySnapshot | `star.capability-snapshot` | [라우팅](routing.md) | run·stage evidence |
| PermissionPlan | `star.permission-plan` | [목표·단계](goal-and-stage.md) | stage permission |
| ApprovalRequest | `star.approval-request` | [목표·단계](goal-and-stage.md) | state, MCP·IPC |
| TaskInvocation | `star.task-invocation` | [검증·증거](validation-and-evidence.md) | tool·check 실행 |
| ValidationPlan | `star.validation-plan` | [검증·증거](validation-and-evidence.md) | stage validation |
| ValidationRun | `star.validation-run` | [검증·증거](validation-and-evidence.md) | evidence |
| ChangeSet | `star.change-set` | [검증·증거](validation-and-evidence.md) | 영향 분석·diff·merge |
| Diagnostic | `star.diagnostic` | [오류·진단](errors-and-diagnostics.md) | evidence, ReviewPack |
| GateDecision | `star.gate-decision` | [검증·증거](validation-and-evidence.md) | stage·goal gate |
| EvidenceBundle | `star.evidence-bundle` | [검증·증거](validation-and-evidence.md) | project `.ai-runs` |
| ReviewPack | `star.review-pack` | [검증·증거](validation-and-evidence.md) | review artifact |
| ArtifactRef | `star.artifact-ref` | [검증·증거](validation-and-evidence.md) | 모든 계약의 큰 자료 참조 |
| EventEnvelope | `star.event` | [이벤트·상태](events-and-state.md) | management repository, `.ai-runs` JSONL export |
| RunSnapshot | `star.run-snapshot` | [이벤트·상태](events-and-state.md) | management repository projection |
| OperationSnapshot | `star.operation-snapshot` | [이벤트·상태](events-and-state.md) | 비동기 command 조회 |
| Checkpoint | `star.checkpoint` | [이벤트·상태](events-and-state.md) | stage artifact |
| Handoff | `star.handoff` | [이벤트·상태](events-and-state.md) | stage·final report |
| MergePlan | `star.merge-plan` | [목표·단계](goal-and-stage.md) | merge state |
| ReproductionPack | `star.reproduction-pack` | [검증·증거](validation-and-evidence.md) | failure evidence |
| CostRecord | `star.cost-record` | [검증·증거](validation-and-evidence.md) | evidence·evaluation |
| BudgetSnapshot | `star.budget-snapshot` | [검증·증거](validation-and-evidence.md) | route·permission·gate |
| EvaluationRun | `star.evaluation-run` | [검증·증거](validation-and-evidence.md) | shadow 비교·규칙 개선 |
| ReleaseManifest | `star.release-manifest` | [검증·증거](validation-and-evidence.md) | release readiness |
| RemoteStateSnapshot | `star.remote-state-snapshot` | [검증·증거](validation-and-evidence.md) | Git·PR·check·release 조회 |
| ErrorEnvelope | `star.error` | [오류·진단](errors-and-diagnostics.md) | CLI·MCP·IPC |
| IpcRequest·Response | `star.ipc-*` | [Local IPC](local-ipc.md) | named pipe |
| McpToolResult·고정 tool input | 고정 surface별 ID | [MCP 구현 계약](mcp-implementation-contract.md) | STDIO MCP |
| ToolPackageManifest | `star.tool-package-manifest` | [Manifest Reference](tool-package-manifest-reference.md) | `tools.d/*.toml` |
| ToolRegistrySnapshot | `star.tool-registry-snapshot` | [외부 Tool Registry](external-tool-registry.md) | Controller·evidence |
| ToolTrustRecord | `star.tool-trust-record` | [Windows Tool Runtime](../architecture/windows-tool-runtime.md) | user trust store |
| ToolRegistryCache | `star.tool-registry-cache` | [Windows Tool Runtime](../architecture/windows-tool-runtime.md) | durable last-known-good |
| ExternalToolRequest | `star.external-tool-request` | [Windows Tool Runtime](../architecture/windows-tool-runtime.md) | Controller→adapter EXE |
| ExternalToolResponse | `star.external-tool-response` | [Windows Tool Runtime](../architecture/windows-tool-runtime.md) | adapter EXE→Controller |
| StarConfig | `star.config` | [설정·Catalog](config-and-catalog.md) | user·project·goal |
| EffectiveConfig | `star.effective-config` | [설정·Catalog](config-and-catalog.md) | application 실행 입력 |
| CatalogSnapshot | `star.catalog-snapshot` | [설정·Catalog](config-and-catalog.md) | route·validation·evidence |
| TaskDescriptor | `star.task-descriptor` | [설정·Catalog](config-and-catalog.md) | Catalog·project config |
| ToolDescriptor | `star.tool-descriptor` | [MCP 구현 계약](mcp-implementation-contract.md) | Controller search·describe·invoke |
| CheckDescriptor | `star.check-descriptor` | [설정·Catalog](config-and-catalog.md) | Catalog |
| ProfileDescriptor | `star.profile-descriptor` | [설정·Catalog](config-and-catalog.md) | Catalog |
| PolicyProfileDescriptor | `star.policy-profile-descriptor` | [설정·Catalog](config-and-catalog.md) | Catalog·permission 설정 |

`ValidationRun`, `GateDecision`, `EvidenceBundle`, `Diagnostic`과 지원 ref·enum의 공개 구현은 `crates/foundation/star-contracts`에 있다. 외부 adapter는 이 crate의 type과 checked-in `specs/schemas/v1`만 소비하며 동등한 완료·진단 DTO를 별도로 정의하지 않는다.

## 데이터 연결

```text
GoalSpec
  -> StageGraph -> StageSpec
       ├─ ContextPack
       ├─ RouteDecision -> CapabilitySnapshot
       ├─ PermissionPlan -> ApprovalRequest
       ├─ ValidationPlan -> TaskInvocation -> ValidationRun -> Diagnostic
       ├─ StageResult -> ChangeSet -> EvidenceBundle
       ├─ Checkpoint -> Handoff
       └─ EvidenceBundle -> ReviewPack -> GateDecision

StarConfig + PolicyProfileDescriptor + ProfileDescriptor + 외부 제한
  -> EffectiveConfig -> CatalogSnapshot

ToolPackageManifest + ToolDescriptor + executable identity
  -> ToolTrustRecord + ToolRegistryCache -> Controller live ToolRegistrySnapshot
       -> star-mcp fixed search·describe·risk call -> typed IPC tool.invoke
       -> Controller hash·lane 검증 -> ExternalToolRequest -> EXE -> ExternalToolResponse

Project + Git·Catalog·source
  -> ProjectRevision -> WorkspaceSnapshot -> CanonicalSource
       -> ScanRun -> Symbol + SymbolReference
       -> ScanRun -> Occurrence -> Finding
            ├─ Baseline + Suppression + Disposition
            └─ ChangeRecipe -> ChangePlan -> PatchSet
                 -> ValidationResult -> GateDecision
  -> 큰 diff·log·trace·report는 ArtifactRef -> `.ai-runs`

모든 변경
  -> EventEnvelope -> RunSnapshot -> OperationSnapshot

병렬 단계
  -> MergePlan -> 통합 ValidationPlan -> 최종 EvidenceBundle
```

## 구현 기능과 계약 대응

| 기능 | 주 계약 |
|---|---|
| A01 목표·작업 계약 | GoalSpec, ProjectRef, PermissionPlan |
| A02 단계 계획·재계획 | StageGraph, StageSpec, StageResult |
| A03 프로젝트 이해·Context | ProjectRef, Project, ProjectRevision, WorkspaceSnapshot, CanonicalSource, ContextPack, SourceRecord |
| A04 변경 영향·위험 분석 | ScanRun, Rule, Finding, Occurrence, Symbol, SymbolReference, ChangeSet, Diagnostic, ValidationPlan |
| A05 Codex 능력·배정 | RouteDecision, CapabilitySnapshot, BudgetSnapshot |
| A06 Codex 실행·터미널 | TaskInvocation, ExternalToolRequest·Response, StageResult, OperationSnapshot |
| A07 상태·Checkpoint·복구 | EventEnvelope, RunSnapshot, Checkpoint, Handoff |
| A08 권한·승인·비밀정보 | PermissionPlan, ApprovalRequest, PolicyProfileDescriptor, EffectiveConfig |
| A09 Worktree·병렬·병합 | MergePlan, ChangeSet, GateDecision |
| A10 Registry | ToolPackageManifest, ToolRegistrySnapshot, Task·Tool·Check·Profile descriptor, CatalogSnapshot |
| B01 변경·범위·주장·증거 | Finding, Suppression, Baseline, Disposition, ChangePlan, PatchSet, ValidationResult, ChangeSet, EvidenceBundle, ReviewPack |
| B02 테스트 신뢰성 | CheckDescriptor, ValidationRun, Diagnostic |
| B03 검증기 보호·Corpus | Diagnostic, GateDecision, EvaluationRun |
| B04 계약·구조·설정·migration | ManagementStoreStatus, ValidationPlan, EffectiveConfig, version 계약 |
| B05 보안·의존성·공급망 | Diagnostic, ArtifactRef, ReleaseManifest |
| B06 실패 분석·재현 | ErrorEnvelope, ReproductionPack, Checkpoint |
| B07 문서·설정·환경 일치 | StarConfig, TaskInvocation, ValidationRun |
| B08 성능·자원·build | ValidationRun, CostRecord, EvaluationRun |
| B09 CI·Release·배포 준비 | ReleaseManifest, GateDecision, RemoteStateSnapshot |
| C01 작업 Profile | ProfileDescriptor, EffectiveConfig, CatalogSnapshot |
| D01 여러 프로젝트·원격·자료조사 | ProjectRef, SourceRecord, RemoteStateSnapshot, MergePlan |
| D02 비용·평가·규칙 개선 | CostRecord, BudgetSnapshot, EvaluationRun |
| D03 Windows 배포·수명주기 | ReleaseManifest, version·migration, ErrorEnvelope |

## 직렬화와 정본

- 사람이 편집하는 설정과 Catalog: TOML
- 기계 상태·계약·결과: UTF-8 JSON
- append-only event·diagnostic stream: 한 줄에 한 JSON object인 JSONL
- 사람이 읽는 보고서: canonical JSON에서 생성한 Markdown
- binary·큰 log·diff: 별도 artifact와 ArtifactRef
- management persistence: backend-neutral repository 뒤의 로컬 DB. SQL·backend 이름은 public contract가 아님

JSON key 순서는 의미가 없다. contract hash와 fingerprint를 만들 때는 다음 canonical byte 규칙을 사용한다.

- UTF-8, BOM 없음, 줄바꿈과 불필요한 공백 없음
- object key를 Unicode code point 오름차순으로 정렬
- JSON string은 동일한 최소 escape 규칙 사용
- integer는 10진수와 불필요한 선행 0 없음
- 소수·비율·금액은 JSON 부동소수점 대신 단위가 있는 decimal string 사용
- hash 자체를 담는 field는 hash 입력에서 제외
- ArtifactRef의 SHA-256은 canonical JSON이 아니라 저장된 원본 byte에 계산

각 fingerprint는 포함할 field 집합을 계약에 고정하고 fixture로 같은 결과를 검사한다. Markdown report는 근거 자료가 아니라 canonical contract에서 만든 view다.

## 호환성과 변경

[Version과 Migration](versioning-and-migrations.md)의 규칙을 적용한다.

- 직렬화 shape 또는 의미 변경: 계약별 schema_version 증가
- additive optional field: compatibility manifest에 이전 reader 동작 명시
- 필수 field 추가, 의미 변경, enum 제거: migration 또는 새 계약 필요
- 더 높은 미지원 version: 쓰지 않고 read-only inspection 또는 명확한 거부
- 설정·state·IPC·Catalog·Plugin version은 서로 독립 관리

## 구현 전 완료 조건

1. Inventory의 각 계약에 소유 문서와 `star-contracts` module이 하나씩 대응한다.
2. 모든 enum, ID, 시간, 경로와 absence 규칙이 일관된다.
3. Goal→Stage→실행→검증→병합→완료 흐름에 끊긴 참조가 없다.
4. MCP와 CLI가 같은 application command와 ErrorEnvelope를 사용한다.
5. 설정 병합 결과를 field별 provenance와 함께 재현할 수 있다.
6. 저장 계약마다 valid, invalid와 이전 version fixture 계획이 있다.
7. raw secret과 외부 protocol 원문이 core contract에 들어오지 않는다.
8. 새 외부 EXE·ToolPackageManifest 추가와 path·EXE 교체가 Gateway·Controller source 변경, MCP·Codex 재시작을 요구하지 않는다.
9. 0단계 source-derived 계약은 Git 정본과 같은 scan 입력에서 재구축되며 local-only state의 backup·loss 경계가 명확하다.
10. CLI-only command graph가 Codex, App Server, 다른 AI와 OpenAI API client를 구성하지 않는다.
