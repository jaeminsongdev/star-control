# 데이터 계약 지도

## 목적

이 폴더는 Star-Control의 Package, 실행 파일, 상태 파일, MCP와 local IPC가 공유하는 데이터 의미를 정의한다. 구현 언어의 내부 구조가 아니라 저장·전달·검증되는 안정 계약이 기준이다.

0단계 공통 개발 관리 의미는 [공통 개발 관리와 로컬 관리 DB 계약](development-management.md), [ADR-0006](../decisions/ADR-0006-공통-개발-관리와-로컬-관리-DB-경계.md)과 [ADR-0007](../decisions/ADR-0007-P0-하이브리드-저장소와-운영-정책.md)에 확정했다. P0 inventory의 관리 계약 19개와 공통 GateDecision·ArtifactRef를 합친 21개 persisted type은 `star-contracts` type, generated JSON Schema, minimal/full/invalid/future fixture와 fingerprint golden으로 구현했다. 1단계의 `ProjectCheckout`, `ProjectCatalogSnapshot`, `CodeIndexSnapshot` 본체와 full 2단계 계획 계약은 아직 제품 구현 전이다. P-0030의 Git 추적 allowlist·exact-root status와 P-0031의 `star.validation-plan` `tracked_path_precursor`·cache pure policy는 각각 M1 persisted snapshot과 full M2 graph를 가장하지 않는 운영 precursor다. 3단계의 공통 Gate는 기존 공개 type 이름을 재사용하되 [검증·증거 계약](validation-and-evidence.md)의 M3 field와 nested type, [오류·진단 계약](errors-and-diagnostics.md)의 Diagnostic v2, [공통 개발 관리 계약](development-management.md)의 Rule·Baseline·Suppression·Disposition v2를 목표로 하며 runner·writer는 **문서 설계 상태**다. 4단계는 [안전한 Patch·Refactor·codemod 엔진 계약](safe-patch-and-codemod.md)의 ChangeRecipe v2·TargetSelector·RecipeExecution v1·PatchSet v2·PatchApplication v1, 5단계는 [관리형 Symbol·상수·에러 코드 Registry 계약](managed-symbol-registry.md)의 Git manifest·ManagedDeclaration·ManagedRegistrySnapshot·binding·consumer compatibility, 6단계는 [계약 호환성·환경](contract-compatibility-and-environment.md)의 8개 input/report 계약, 7단계는 [실패 재현·보안·의존성 유지보수](failure-security-and-dependency-maintenance.md)의 failure·dependency·supply-chain·Radar 계약, 8단계는 [Migration·성능·언어·플랫폼](migration-performance-and-platform.md)의 12개 migration/measurement/equivalence/handoff 계약, 9단계는 [CrossRepo ChangeBundle](cross-repo-change-bundle.md)의 9개 bundle/worktree/merge/remote/release-handoff 계약을 목표로 하며 모두 제품 구현 전이다. 10단계는 [CI·Release·평가·최종 제품 완성 계약](ci-release-evaluation-and-product-completion.md)에서 `ReleaseManifest` v2, `EvaluationRun` v2, validation/evidence v5와 Catalog lifecycle을 상세화하며 새 최상위 Inventory type을 만들지 않는다. 11단계 [Rust 코드 스타일 자동 교정](../features/rust-code-style-auto-fix.md)도 새 top-level run truth 없이 기존 `RecipeExecution`·`PatchSet`·`PatchApplication`·`ValidationRun`·`EvidenceBundle`에 `RustToolchainBinding`·`RustStylePolicySnapshot`·`RustStyleCoverageMatrix`·`RustStyleStepExecution` nested type을 연결하며 **제품 구현 전**이다. 전체 단계 완료 여부는 [최종 구현 로드맵](../roadmap/final-implementation.md)과 `PLANS.md`의 검증 근거로만 판정한다. MCP 부분의 exact 구현 값은 [MCP 구현 동결 계약](mcp-implementation-contract.md), [ToolPackageManifest Reference](tool-package-manifest-reference.md), [Windows Tool Runtime](../architecture/windows-tool-runtime.md)과 [MCP 검증 행렬](../testing/mcp-verification-matrix.md)에 동결됐다. MCP Gateway·IPC·Registry·외부 EXE Runtime 범위의 type, generated Schema, fixture와 제품 코드는 구현됐으며 현재 구현·외부 gate 판정은 [MCP 완료 감사](../testing/mcp-completion-audit.md)에 분리해 기록한다. 이 상태 문구는 나머지 Star-Control 계약까지 구현됐다는 뜻이 아니다.

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

## Codex Runtime과 업데이트

P-0038의 Runtime Generation selector, candidate review와 activation record는 [Runtime update와 activation 계약](runtime-update-and-activation.md)이 소유한다. P-0039의 `star-updater.exe`, CodexInstance·CodexTask·WorkSession·McpConnection projection, Controller 30초 idle lease, restart-required integration update와 durable receipt는 [Codex 생명주기와 Star Updater 계약](codex-lifecycle-and-updater.md)이 소유한다. 이 계약은 구현 진행 중이며 현재 설치본이 이미 4 EXE 또는 자동 restart를 제공한다는 뜻이 아니다.

## 공통 형식

### 식별자

| 종류 | 예시 | 규칙 |
|---|---|---|
| ProjectId | `prj_01J...` | 경로나 저장소 이름과 분리된 stable ID |
| CheckoutId | `cko_01J...` | 한 Project의 local working copy attachment ID |
| ProjectCatalogSnapshotId | `pcs_<base32-sha256>` | discovery scope의 Project·Checkout·workspace 관계 snapshot |
| CodeIndexSnapshotId | `cix_<base32-sha256>` | 한 WorkspaceSnapshot의 index content identity |
| ManagedDeclarationId | `mdc_<base32-sha256>` | 의미·namespace·owner에 고정된 관리 선언 ID. public value와 별개 |
| ManagedRegistrySnapshotId | `mrs_<base32-sha256>` | Git manifest와 binding 관찰 결과의 derived snapshot identity |
| ProjectRevisionId | `prv_<base32-sha256>` | Project의 immutable source revision identity |
| WorkspaceSnapshotId | `wsp_<base32-sha256>` | 실제 관찰한 workspace byte·scope identity |
| ScanRunId | `scn_01J...` | 한 scan 실행 instance |
| FindingId | `fnd_<base32-sha256>` | Rule의 stable finding identity |
| OccurrenceId | `occ_<base32-sha256>` | snapshot·location·source hash에 고정된 관찰 identity |
| CanonicalSourceId | `src_<base32-sha256>` | Project 안의 source identity |
| SymbolId | `sym_<base32-sha256>` | source-derived symbol identity |
| SymbolReferenceId | `srf_<base32-sha256>` | source-derived reference edge identity |
| SuppressionId | `sup_01J...` | shared 또는 local suppression decision |
| BaselineId | `bas_01J...` | Finding·Diagnostic issue set 기준. v1은 Finding 전용 |
| DispositionId | `dsp_01J...` | Finding triage decision |
| ChangePlanId | `cpl_01J...` | local change plan |
| PatchSetId | `pat_01J...` | immutable patch proposal |
| RecipeExecutionId | `rex_01J...` | 한 Recipe preview·idempotence attempt |
| PatchApplicationId | `pap_01J...` | 한 PatchSet apply·recovery lifecycle |
| ValidationResultId | `vrs_01J...` | normalized validation result |
| ManagementStoreId | `mst_01J...` | local store generation identity |
| GoalId | `gol_01J...` | 사용자 목표 하나 |
| TaskSpecId | `tsk_01J...` | 사용자가 직접 입력한 한 변경 계획 revision 계열 |
| ScopeRevisionId | `scp_01J...` | requested·analysis·change·validation scope revision |
| ImpactAnalysisId | `imp_01J...` | 한 input fingerprint의 영향 계산 |
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

현재 설계 Inventory는 122개 계약 항목이다. 기존 0~11단계의 119개에 P-0026 설치 transport 기술 계약 3개를 더한 수다. `IpcRequest·Response`와 고정 MCP surface처럼 한 행이 여러 generated Schema를 소유할 수 있으므로 이 수는 실제 `.schema.json` 파일 수와 같다는 뜻이 아니다. 0단계의 관리 계약 19개와 공통 `GateDecision`·`ArtifactRef`를 합친 21개 persisted type은 Rust type, generated Schema와 fixture까지 구현했다. 1단계의 새 3개와 2단계의 새 `TaskSpec`·`ScopeRevision`·`ImpactAnalysis`·`RiskPathDescriptor` 4개 계약은 설계 상태다. 3단계는 이 표의 validation·Rule·Baseline·Suppression·Disposition 계약을 새 의미 version으로 확장하고 아직 공개 type이 없는 ReviewPack과 nested type을 구현 대상으로 구체화하지만 새 최상위 Inventory 행을 추가하지 않는다. 4단계는 기존 ChangeRecipe·PatchSet의 새 version 외에 `RecipeExecution`·`PatchApplication` 2개 최상위 목표 계약을 추가했다. 5단계의 `ManagedRegistryManifest`·`ManagedRegistryFragment`·`ManagedRegistrySnapshot` 3개와 6단계의 compatibility·documentation·environment top-level 계약 8개도 설계 상태다. 7단계는 기존 `ReproductionPack`을 상세화하고 새 persisted document 8개와 Catalog descriptor 2개를 추가한다. 8단계는 migration 6개, performance 3개, language/platform·9단계 handoff 3개인 top-level 계약 12개를 추가한다. 9단계는 `MultiProjectGoal`, bundle/participant, worktree·merge·remote·release handoff 9개 top-level 계약을 추가하고 기존 MergePlan·RemoteStateSnapshot과 evidence 계약을 새 version으로 확장한다. 10단계는 기존 `ReleaseManifest`, `EvaluationRun`, validation/evidence, Catalog descriptor를 새 의미 version으로 확장한다. 11단계는 기존 M3/M4 top-level record에 `RustToolchainBinding`, `RustStylePolicySnapshot`, `RustStyleCoverageMatrix`, `RustStyleStepExecution`을 nested versioned evidence로만 추가하므로 0~11단계 목표 Inventory 수는 119를 유지한다. nested `ManagedDeclaration`, `ContractChangeRecord`, `ConfigKeyTrace`, `UpdateCandidate`, `RadarItem`, `MigrationStep`, `Measurement`, `EquivalenceDimension`, `ProjectRelation`, `BundleStep`, `CompatibilityWindow`, `MergeQueueEntry`, `ProjectReleaseInput`, release artifact entry, evaluation case result와 Rust style 4개 type은 별도 최상위 행으로 세지 않는다. P-0026의 세 기술 계약은 Rust type·generated Schema와 installer-owned local file로 구현됐지만 M10 release/evaluation 완료 근거로 사용하지 않는다.

| 계약 | Schema ID | 소유 문서 | 저장·전달 위치 |
|---|---|---|---|
| GoalSpec | `star.goal-spec` | [목표·단계](goal-and-stage.md) | Goal state, MCP·IPC |
| MultiProjectGoal | `star.multi-project-goal` | [9단계 ChangeBundle](cross-repo-change-bundle.md) | global project relation·step DAG. P7 v1 목표 |
| TaskSpec | `star.task-spec` | [목표·단계](goal-and-stage.md) | CLI-only planning input·Goal task ref |
| ScopeRevision | `star.scope-revision` | [목표·단계](goal-and-stage.md) | planning state·replan lineage |
| ProjectRef | `star.project-ref` | [목표·단계](goal-and-stage.md) | Goal·Context·multi-project |
| Project | `star.project` | [공통 개발 관리](development-management.md) | Git 선언·관리 DB projection |
| ProjectCheckout | `star.project-checkout` | [Project Catalog·Code Index](project-catalog-and-code-index.md) | global store local attachment projection |
| ProjectCatalogSnapshot | `star.project-catalog-snapshot` | [Project Catalog·Code Index](project-catalog-and-code-index.md) | global store discovery generation |
| CodeIndexSnapshot | `star.code-index-snapshot` | [Project Catalog·Code Index](project-catalog-and-code-index.md) | project store index generation·scan evidence |
| ManagedRegistryManifest | `star.managed-registry-manifest` | [Managed Registry](managed-symbol-registry.md) | Git source root manifest. M5 v1 목표 |
| ManagedRegistryFragment | `star.managed-registry-fragment` | [Managed Registry](managed-symbol-registry.md) | Git source declaration fragment. M5 v1 목표 |
| ManagedRegistrySnapshot | `star.managed-registry-snapshot` | [Managed Registry](managed-symbol-registry.md) | project store derived Index·impact/evidence input. M5 v1 목표 |
| ProjectContractManifest | `star.project-contract-manifest` | [계약 호환성·환경](contract-compatibility-and-environment.md) | `.star-control/contracts.toml` Git source. M6 v1 목표 |
| ContractSurfaceSnapshot | `star.contract-surface-snapshot` | [계약 호환성·환경](contract-compatibility-and-environment.md) | baseline/current derived evidence. M6 v1 목표 |
| CompatibilityReport | `star.compatibility-report` | [계약 호환성·환경](contract-compatibility-and-environment.md) | compatibility·consumer·migration derived evidence. M6 v1 목표 |
| DocumentationSnapshot | `star.documentation-snapshot` | [계약 호환성·환경](contract-compatibility-and-environment.md) | docs/config/generated/assumption derived evidence. M6 v1 목표 |
| EnvironmentSnapshot | `star.environment-snapshot` | [계약 호환성·환경](contract-compatibility-and-environment.md) | redacted read-only environment evidence. M6 v1 목표 |
| ProjectDoctorReport | `star.project-doctor-report` | [계약 호환성·환경](contract-compatibility-and-environment.md) | doctor constraint evaluation. M6 v1 목표 |
| CleanRoomSpecification | `star.clean-room-specification` | [계약 호환성·환경](contract-compatibility-and-environment.md) | Git/approved plan clean-room constraint. M6 v1 목표 |
| DependencySecurityInputManifest | `star.dependency-security-input-manifest` | [계약 호환성·환경](contract-compatibility-and-environment.md) | 후속 7단계 read-only discovery handoff. M6 v1 목표 |
| FailureRecord | `star.failure-record` | [7단계 유지보수](failure-security-and-dependency-maintenance.md) | project store failure occurrence·causality. M7 v1 목표 |
| RegressionRecord | `star.regression-record` | [7단계 유지보수](failure-security-and-dependency-maintenance.md) | before/after·recurrence evidence. M7 v1 목표 |
| RecoveryPlan | `star.recovery-plan` | [7단계 유지보수](failure-security-and-dependency-maintenance.md) | rollback·roll-forward·restore plan/attempt. M7 v1 목표 |
| DependencySnapshot | `star.dependency-snapshot` | [7단계 유지보수](failure-security-and-dependency-maintenance.md) | source-derived dependency relation·state. M7 v1 목표 |
| SupplyChainSnapshot | `star.supply-chain-snapshot` | [7단계 유지보수](failure-security-and-dependency-maintenance.md) | security·workflow·release observation. M7 v1 목표 |
| ExternalDataSnapshot | `star.external-data-snapshot` | [7단계 유지보수](failure-security-and-dependency-maintenance.md) | external source provenance·freshness input. M7 v1 목표 |
| DependencyUpdatePlan | `star.dependency-update-plan` | [7단계 유지보수](failure-security-and-dependency-maintenance.md) | candidate·approval·PatchSet·rollback state. M7 v1 목표 |
| MaintenanceRadarSnapshot | `star.maintenance-radar-snapshot` | [7단계 유지보수](failure-security-and-dependency-maintenance.md) | rebuildable deterministic maintenance view. M7 v1 목표 |
| ProjectMigrationManifest | `star.project-migration-manifest` | [8단계 migration·성능·언어](migration-performance-and-platform.md) | `.star-control/migrations.toml` Git source. M8 v1 목표 |
| MigrationPlan | `star.migration-plan` | [8단계 migration·성능·언어](migration-performance-and-platform.md) | 한 Project·target의 immutable plan. M8 v1 목표 |
| MigrationCheckpoint | `star.migration-checkpoint` | [8단계 migration·성능·언어](migration-performance-and-platform.md) | durable step prefix·resume/reconcile input. M8 v1 목표 |
| MigrationAttempt | `star.migration-attempt` | [8단계 migration·성능·언어](migration-performance-and-platform.md) | dry-run·backup·rehearsal·execute·resume·rollback 사실. M8 v1 목표 |
| MigrationValidationReport | `star.migration-validation-report` | [8단계 migration·성능·언어](migration-performance-and-platform.md) | before/after invariant·active state·Gate. M8 v1 목표 |
| RestoreVerificationRecord | `star.restore-verification-record` | [8단계 migration·성능·언어](migration-performance-and-platform.md) | backup integrity·restore rehearsal·behavior evidence. M8 v1 목표 |
| PerformanceWorkloadSpec | `star.performance-workload-spec` | [8단계 migration·성능·언어](migration-performance-and-platform.md) | explicit workload·metric·noise protocol Git source. M8 v1 목표 |
| PerformanceRun | `star.performance-run` | [8단계 migration·성능·언어](migration-performance-and-platform.md) | exact cohort의 raw warmup/measured attempt. M8 v1 목표 |
| PerformanceComparison | `star.performance-comparison` | [8단계 migration·성능·언어](migration-performance-and-platform.md) | comparability·statistics·noise·trade-off. M8 v1 목표 |
| LanguageMigrationPlan | `star.language-migration-plan` | [8단계 migration·성능·언어](migration-performance-and-platform.md) | behavior·coexistence·consumer·cutover·rollback plan. M8 v1 목표 |
| EquivalenceReport | `star.equivalence-report` | [8단계 migration·성능·언어](migration-performance-and-platform.md) | compile과 기능 동등성 dimension 분리. M8 v1 목표 |
| CrossProjectMigrationHandoff | `star.cross-project-migration-handoff` | [8단계 migration·성능·언어](migration-performance-and-platform.md) | 9단계 ChangeBundle의 read-only participant 입력. M8 v1 목표 |
| CrossRepoChangeBundle | `star.cross-repo-change-bundle` | [9단계 ChangeBundle](cross-repo-change-bundle.md) | global participant ref·order·policy·aggregate state. P7 v1 목표 |
| ChangeBundleParticipant | `star.change-bundle-participant` | [9단계 ChangeBundle](cross-repo-change-bundle.md) | project-local base·dirty·Patch·Gate·recovery. P7 v1 목표 |
| WorktreeRecord | `star.worktree-record` | [9단계 ChangeBundle](cross-repo-change-bundle.md) | owned project worktree identity·lifecycle. P6 v1 목표 |
| MergeQueueRecord | `star.merge-queue-record` | [9단계 ChangeBundle](cross-repo-change-bundle.md) | repository별 serial local integration queue. P6 v1 목표 |
| MergeConflictRecord | `star.merge-conflict-record` | [9단계 ChangeBundle](cross-repo-change-bundle.md) | 양쪽 intent·contract·resolution evidence. P6 v1 목표 |
| ProjectMergeResult | `star.project-merge-result` | [9단계 ChangeBundle](cross-repo-change-bundle.md) | local integration actual revision·Gate. P6 v1 목표 |
| RemoteOperationRecord | `star.remote-operation-record` | [9단계 ChangeBundle](cross-repo-change-bundle.md) | approval-bound push·PR·merge·publish effect. P7 v1 목표 |
| ChangeBundleReleaseHandoff | `star.change-bundle-release-handoff` | [9단계 ChangeBundle](cross-repo-change-bundle.md) | 10단계 project revision·artifact·Gate input. P7 v1 목표 |
| ProjectRevision | `star.project-revision` | [공통 개발 관리](development-management.md) | 관리 DB·scan input |
| WorkspaceSnapshot | `star.workspace-snapshot` | [공통 개발 관리](development-management.md) | 관리 DB·artifact manifest |
| ScanRun | `star.scan-run` | [공통 개발 관리](development-management.md) | 관리 DB·scan evidence |
| Rule | `star.rule` | [공통 개발 관리](development-management.md) | Git·Catalog 선언, resolved snapshot |
| Finding | `star.finding` | [공통 개발 관리](development-management.md) | 관리 DB projection |
| Occurrence | `star.occurrence` | [공통 개발 관리](development-management.md) | 관리 DB·evidence reference |
| Symbol | `star.symbol` | [공통 개발 관리](development-management.md) | 관리 DB derived index |
| SymbolReference | `star.symbol-reference` | [공통 개발 관리](development-management.md) | 관리 DB derived edge |
| CanonicalSource | `star.canonical-source` | [공통 개발 관리](development-management.md) | Project source identity |
| Suppression | `star.suppression` | [공통 개발 관리](development-management.md) | Git shared 선언 또는 local DB state. M3 v2는 Diagnostic selector 추가 목표 |
| Baseline | `star.baseline` | [공통 개발 관리](development-management.md) | Git shared 선언 또는 local DB state. M3 v2는 공통 issue entry 추가 목표 |
| Disposition | `star.disposition` | [공통 개발 관리](development-management.md) | local triage state. M3 v2는 Diagnostic subject 추가 목표 |
| ChangePlan | `star.change-plan` | [공통 개발 관리](development-management.md) | local application state |
| PatchSet | `star.patch-set` | [공통 개발 관리](development-management.md), [4단계 엔진](safe-patch-and-codemod.md) | immutable preview summary·`.ai-runs` forward/reverse diff. M4 v2 목표 |
| ChangeRecipe | `star.change-recipe` | [공통 개발 관리](development-management.md), [4단계 엔진](safe-patch-and-codemod.md) | Git·Catalog 선언. M4 descriptor v2 목표 |
| RecipeExecution | `star.recipe-execution` | [4단계 엔진](safe-patch-and-codemod.md) | project repository·preview/tool evidence. M4 v1 목표 |
| PatchApplication | `star.patch-application` | [4단계 엔진](safe-patch-and-codemod.md) | project repository·actual apply/recovery evidence. M4 v1 목표 |
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
| ImpactAnalysis | `star.impact-analysis` | [변경 계획·영향 분석](change-planning-and-impact.md) | local planning state·`.ai-runs` trace |
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
| MergePlan | `star.merge-plan` | [목표·단계](goal-and-stage.md), [9단계 ChangeBundle](cross-repo-change-bundle.md) | project-local merge state. P6 v2 목표 |
| ReproductionPack | `star.reproduction-pack` | [7단계 유지보수](failure-security-and-dependency-maintenance.md), [검증·증거](validation-and-evidence.md) | curated failure reproduction manifest·ArtifactRef. M7 v1 의미 상세화 |
| CostRecord | `star.cost-record` | [검증·증거](validation-and-evidence.md) | evidence·evaluation |
| BudgetSnapshot | `star.budget-snapshot` | [검증·증거](validation-and-evidence.md) | route·permission·gate |
| EvaluationRun | `star.evaluation-run` | [검증·증거](validation-and-evidence.md) | Rule·Check·Profile·Recipe의 CLI/Codex 분리 shadow 비교·규칙 개선. M10 v2 목표 |
| ReleaseManifest | `star.release-manifest` | [검증·증거](validation-and-evidence.md) | build-once artifact·release readiness·approval·published observation. M10 v2 목표 |
| RemoteStateSnapshot | `star.remote-state-snapshot` | [검증·증거](validation-and-evidence.md), [9단계 ChangeBundle](cross-repo-change-bundle.md) | adapter-bound Git·PR·check·release 조회. P7 v2 목표 |
| ErrorEnvelope | `star.error` | [오류·진단](errors-and-diagnostics.md) | CLI·MCP·IPC |
| ReleaseFileManifest | `star.release-file-manifest` | [Windows 설치·Codex 연동](windows-installation-and-codex-integration.md) | architecture별 설치 stage의 파일·hash 정본 |
| InstallationRecord | `star.installation-record` | [Windows 설치·Codex 연동](windows-installation-and-codex-integration.md) | current-user 실제 설치 경로·instance 기록 |
| CodexIntegrationRecord | `star.codex-integration-record` | [Windows 설치·Codex 연동](windows-installation-and-codex-integration.md) | 렌더링된 로컬 Marketplace·등록 상태 기록 |
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
| RiskPathDescriptor | `star.risk-path-descriptor` | [설정·Catalog](config-and-catalog.md) | Catalog |
| ExternalDataSourceDescriptor | `star.external-data-source-descriptor` | [설정·Catalog](config-and-catalog.md) | built-in/project maintenance Catalog. M7 목표 |
| PackageManagerAdapterDescriptor | `star.package-manager-adapter-descriptor` | [설정·Catalog](config-and-catalog.md) | built-in/project maintenance Catalog. M7 목표 |
| ProfileDescriptor | `star.profile-descriptor` | [설정·Catalog](config-and-catalog.md) | Catalog |
| PolicyProfileDescriptor | `star.policy-profile-descriptor` | [설정·Catalog](config-and-catalog.md) | Catalog·permission 설정 |

`ValidationPlan` tracked-path precursor, `ValidationRun`, `GateDecision`, `EvidenceBundle`, `Diagnostic`과 지원 ref·enum의 **현재 v1 공개 구현**은 `crates/foundation/star-contracts`에 있다. precursor보다 넓은 TaskSpec·ScopeRevision·ImpactAnalysis binding과 3단계 문서가 정의한 exact subject binding, completeness·freshness·stability, 공통 Diagnostic v2, baseline·suppression evaluation과 Patch 전·후 Gate는 아직 해당 구현에 반영되지 않았다. 외부 adapter는 이후에도 이 crate의 type과 checked-in Schema만 소비하며 동등한 완료·진단 DTO를 별도로 정의하지 않는다.

## 데이터 연결

```text
GoalSpec
  -> TaskSpec -> ScopeRevision
  -> StageGraph -> StageSpec
       ├─ ContextPack
       ├─ RouteDecision -> CapabilitySnapshot
       ├─ PermissionPlan -> ApprovalRequest
       ├─ ValidationPlan -> TaskInvocation -> ValidationRun -> Diagnostic
       ├─ StageResult -> ChangeSet -> EvidenceBundle
       ├─ Checkpoint -> Handoff
       └─ GateDecision -> EvidenceBundle -> ReviewPack

TaskSpec + ScopeRevision
  + ProjectCatalogSnapshot + ProjectRevision + dirty WorkspaceSnapshot
  + CodeIndexSnapshot + CatalogSnapshot
  -> ChangeSet[] -> ImpactAnalysis
       ├─ direct/transitive + confirmed/possible ImpactEdge
       ├─ RiskPathDescriptor -> risk path
       ├─ TaskDescriptor + CheckDescriptor -> affected selection + fallback
       └─ project별 ChangePlan v2[] + ValidationPlan -> 3단계 runner input

StarConfig + PolicyProfileDescriptor + ProfileDescriptor + 외부 제한
  -> EffectiveConfig -> CatalogSnapshot

TaskDescriptor + CheckDescriptor + RiskPathDescriptor + ProfileDescriptor
  -> CatalogSnapshot -> ImpactAnalysis·ValidationPlan selection evidence

ToolPackageManifest + ToolDescriptor + executable identity
  -> ToolTrustRecord + ToolRegistryCache -> Controller live ToolRegistrySnapshot
       -> star-mcp fixed search·describe·risk call -> typed IPC tool.invoke
       -> Controller hash·lane 검증 -> ExternalToolRequest -> EXE -> ExternalToolResponse

Project + ProjectCheckout + discovery roots
  -> ProjectCatalogSnapshot -> project·checkout·workspace relation
  -> ProjectRevision -> dirty WorkspaceSnapshot -> CanonicalSource
       -> ScanRun -> CodeIndexSnapshot
            -> package·module·Symbol + SymbolReference + contract·dependency graph
       -> ScanRun -> Occurrence -> Finding
             ├─ Baseline + Suppression + Disposition
            └─ ChangeRecipe -> ChangePlan v1/v2 -> RecipeExecution* -> PatchSet
                 -> patch_pre_apply GateDecision -> PatchApplication
                 -> actual ChangeSet -> ValidationResult -> patch_post_apply GateDecision
  -> 큰 diff·log·trace·report는 ArtifactRef -> `.ai-runs`

ManagedRegistryManifest + 명시된 ManagedRegistryFragment[]
  -> ManagedDeclaration[] + namespace claim + tombstone
  -> ScanRun + CodeIndexSnapshot -> ManagedRegistrySnapshot
       ├─ definition·reference·Schema·documentation·generated output binding
       ├─ consumer minimum version·alias window·transition status
       └─ ManagedDeclarationChangeIntent -> ImpactAnalysis·ChangePlan
            -> RecipeExecution dry-run -> PatchSet -> M3 pre/post Gate

ProjectContractManifest + immutable baseline approval
  + baseline/current source + ManagedRegistrySnapshot + CatalogSnapshot
  -> ContractSurfaceSnapshot[baseline,current] -> CompatibilityReport
       ├─ ContractChangeRecord -> ConsumerImpactRecord + migration requirement
       ├─ DocumentationSnapshot -> docs/config/generated/assumption Diagnostic
       ├─ EnvironmentSnapshot -> ProjectDoctorReport + CleanRoomSpecification/result
       ├─ M3 GateDecision -> EvidenceBundle·ReviewPack
       └─ DependencySecurityInputManifest -> 후속 dependency·security 검사

DependencySecurityInputManifest + current Project/Code Index
  + common Finding·Diagnostic·Suppression + ExternalDataSourceDescriptor
  -> DependencySnapshot + SupplyChainSnapshot + ExternalDataSnapshot
       ├─ FailureRecord -> ReproductionPack -> RegressionRecord + RecoveryPlan
       ├─ UpdateCandidate -> DependencyUpdatePlan
       │    -> registered PackageManagerAdapterDescriptor
       │    -> isolated actual diff -> M2 replan -> PatchSet
       │    -> awaiting apply approval -> M4 PatchApplication -> M3 Gate
       └─ MaintenanceRadarSnapshot -> 원본 refs의 deterministic derived view

ProjectMigrationManifest + current MigrationVersionVector
  + M2 plan + M6 consumer/environment + M7 RecoveryPlan
  -> MigrationPlan -> dry-run -> backup/integrity -> RestoreVerificationRecord
       -> migration rehearsal -> MigrationCheckpoint[] + MigrationAttempt[]
       -> M3 migration_pre_execute -> execute/resume -> MigrationValidationReport
       -> migration_post_execute | rollback -> migration_post_rollback

PerformanceWorkloadSpec + exact baseline/candidate subject
  -> PerformanceRun[warmup,measured][]
  -> PerformanceComparison + correctness/trade-off Gate

LanguageMigrationPlan + behavior baseline + M4 Recipe/PatchSet
  -> boundary/coexistence + consumer transition
  -> EquivalenceReport -> language_cutover Gate
  -> CrossProjectMigrationHandoff -> 9단계 ChangeBundle input

GoalSpec + current ProjectRelation + project별 ChangePlan/PatchSet/Gate
  -> MultiProjectGoal + BundleStep DAG + CompatibilityWindow
  -> CrossRepoChangeBundle
       -> ChangeBundleParticipant[]
       -> WorktreeRecord[] -> MergePlan v2 -> MergeQueue/Conflict -> ProjectMergeResult
       -> project별 EvidenceBundle + change_bundle_goal_exit Gate
       -> optional RemoteStateSnapshot/RemoteOperationRecord
  -> ChangeBundleReleaseHandoff -> 10단계 project source·artifact input

모든 변경
  -> EventEnvelope -> RunSnapshot -> OperationSnapshot

병렬 단계
  -> project-local MergePlan -> 통합 ValidationPlan -> project EvidenceBundle
```

## 구현 기능과 계약 대응

| 기능 | 주 계약 |
|---|---|
| A01 목표·작업 계약 | GoalSpec, TaskSpec, ProjectRef, ScopeRevision, PermissionPlan |
| A02 단계 계획·재계획 | TaskSpec, ScopeRevision, StageGraph, StageSpec, StageResult |
| A03 프로젝트 이해·Context | ProjectRef, Project, ProjectRevision, WorkspaceSnapshot, CanonicalSource, ContextPack, SourceRecord |
| A04 변경 영향·위험 분석 | TaskSpec, ScopeRevision, CodeIndexSnapshot, ManagedRegistrySnapshot, ChangeSet, ImpactAnalysis·ImpactEdge, RiskPathDescriptor, ChangePlan, ValidationPlan |
| A05 Codex 능력·배정 | RouteDecision, CapabilitySnapshot, BudgetSnapshot |
| A06 Codex 실행·터미널 | TaskInvocation, ExternalToolRequest·Response, StageResult, OperationSnapshot |
| A07 상태·Checkpoint·복구 | EventEnvelope, RunSnapshot, Checkpoint, Handoff, RecoveryPlan |
| A08 권한·승인·비밀정보 | PermissionPlan, ApprovalRequest, PolicyProfileDescriptor, EffectiveConfig |
| A09 Worktree·병렬·병합 | WorktreeDecision, WorktreeRecord, PatchApplication, MergePlan, MergeQueueRecord, MergeConflictRecord, ProjectMergeResult, ChangeSet, GateDecision |
| A10 Registry | ManagedRegistryManifest·Fragment·Snapshot, ChangeRecipe, ToolPackageManifest, ToolRegistrySnapshot, Task·Tool·Check·RiskPath·ExternalDataSource·PackageManagerAdapter·Profile descriptor, CatalogSnapshot |
| B01 변경·범위·주장·증거 | Finding, Suppression, Baseline, Disposition, ChangePlan, RecipeExecution, PatchSet, PatchApplication, ValidationResult, ChangeSet, EvidenceBundle, ReviewPack |
| B02 테스트 신뢰성 | CheckDescriptor, ValidationRun, Diagnostic |
| B03 검증기 보호·Corpus | Diagnostic, GateDecision, EvaluationRun |
| B04 계약·구조·설정·migration | ManagedRegistrySnapshot, ProjectContractManifest, ContractSurfaceSnapshot, CompatibilityReport, ProjectMigrationManifest, MigrationPlan·Attempt·Checkpoint·ValidationReport, RestoreVerificationRecord, ValidationPlan, EffectiveConfig, version 계약 |
| B05 보안·의존성·공급망 | DependencySecurityInputManifest, DependencySnapshot, SupplyChainSnapshot, ExternalDataSnapshot, DependencyUpdatePlan, Diagnostic, PatchSet, ArtifactRef, ReleaseManifest |
| B06 실패 분석·재현 | ErrorEnvelope, FailureRecord, ReproductionPack, RegressionRecord, RecoveryPlan, Checkpoint, MigrationCheckpoint·Attempt·RestoreVerificationRecord |
| B07 문서·설정·환경 일치 | DocumentationSnapshot, EnvironmentSnapshot, ProjectDoctorReport, CleanRoomSpecification, ManagedRegistrySnapshot, RegistryConsistencyRecord, StarConfig, TaskInvocation, ValidationRun |
| B08 성능·자원·build | PerformanceWorkloadSpec, PerformanceRun, PerformanceComparison, ValidationRun, CostRecord, EvaluationRun |
| B09 CI·Release·배포 준비 | ReleaseManifest, ArtifactRef, ValidationRun, GateDecision, EvidenceBundle, ApprovalRequest, RemoteStateSnapshot, RemoteOperationRecord |
| C01 작업 Profile | ProfileDescriptor, TaskSpec, ScopeRevision, ImpactAnalysis, MigrationPlan, PerformanceComparison, LanguageMigrationPlan·EquivalenceReport, EffectiveConfig, CatalogSnapshot |
| D01 여러 프로젝트·원격·자료조사 | ProjectRef, MultiProjectGoal, CrossRepoChangeBundle, ChangeBundleParticipant, SourceRecord, RemoteStateSnapshot, RemoteOperationRecord, MergePlan, CrossProjectMigrationHandoff, ChangeBundleReleaseHandoff |
| D02 비용·평가·규칙 개선 | CostRecord, BudgetSnapshot, EvaluationRun, MaintenanceRadarSnapshot, Catalog Rule·Check·Profile·Recipe lifecycle |
| D03 Windows 배포·수명주기 | ReleaseManifest, ArtifactRef, GateDecision, ApprovalRequest, RemoteStateSnapshot, RemoteOperationRecord, version·migration, ErrorEnvelope |

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

6단계의 baseline/current 비교, kind별 `unchanged|compatible|additive|breaking|unknown`, 소비자 migration, deprecated window, 문서·config·environment drift는 [계약 호환성·문서·설정·개발 환경 관리](contract-compatibility-and-environment.md)가 소유한다. 이 비교 record는 Managed Registry의 ID·lifecycle을 재선언하지 않고 exact `ManagedDeclarationRef`를 사용한다.

7단계의 family/occurrence fingerprint, ReproductionPack, 외부 자료 freshness, dependency candidate·PatchSet·rollback과 Radar ordering은 [실패 재현·보안·의존성 유지보수](failure-security-and-dependency-maintenance.md)가 소유한다. fingerprint normalization, source maximum-age 또는 Radar sort policy가 바뀌면 기존 document를 조용히 current로 재해석하지 않고 contract/policy version과 compatibility fixture를 갱신한다.

8단계의 target version chain, checkpoint/resume/partial/rollback, backup/restore claim 수준, comparable performance cohort와 behavior equivalence·platform evidence는 [Migration·성능·언어·플랫폼](migration-performance-and-platform.md)이 소유한다. Star-Control 자체 management store migration과 범용 Project migration을 같은 manifest·adapter로 합치지 않으며, M8 Gate ref가 없는 historical M3 v2 evidence를 current M8 성공으로 승격하지 않는다.

10단계의 `ReleaseManifest` v2 상태, build-once artifact 승격, clean Windows x64 Stable install lifecycle, ARM64 `native_unverified` Preview simulation, GitHub Release publish 확인과 `EvaluationRun` v2 comparability·validator guard·Catalog lifecycle은 [CI·Release·평가·최종 제품 완성](ci-release-evaluation-and-product-completion.md)이 application 의미를 소유한다. 직렬화 field·absence·evidence binding은 [검증·증거](validation-and-evidence.md), version 전이는 [Version과 Migration](versioning-and-migrations.md), 설정·descriptor source는 [설정과 Catalog](config-and-catalog.md)가 각각 한 번만 소유한다.

고정 MCP Bootstrap Bridge가 선택하는 Runtime Generation, activation record, candidate review와 update operation 상태는 [Runtime update와 activation](runtime-update-and-activation.md)이 소유한다. 설치 transport·Plugin 렌더링과 fixed MCP wire를 중복 정의하지 않으며, 일상 Runtime update가 Codex 설정 변경으로 확장되지 않게 한다.

11단계의 Rust style workflow·nested type 의미는 [Rust 코드 스타일 자동 교정](../features/rust-code-style-auto-fix.md)이 소유한다. Tool 실행은 [외부 Tool Registry](external-tool-registry.md), policy source는 [설정과 Catalog](config-and-catalog.md), Patch lifecycle은 [M4](safe-patch-and-codemod.md), Gate·evidence binding은 [검증·증거](validation-and-evidence.md)가 기존 계약을 그대로 소유한다. M11은 이 의미를 복제하거나 별도 DB truth·mutable run record를 만들지 않는다.

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
11. M8 migration success·partial·failure·outcome unknown·rollback 상태와 backup/restore claim 수준이 기계적으로 구분된다.
12. 성능 비교는 declared workload·numeric unit/collector·comparable exact cohort를 요구하고 compile-only result는 language equivalence를 만들지 않는다.
13. CrossProjectMigrationHandoff는 9단계 ChangeBundle 입력일 뿐 cross-project apply·approval·성공 계약이 아니다.
14. CrossRepoChangeBundle은 project별 source effect·Git history·Gate·evidence를 유지하며 management coordination을 cross-repository transaction으로 표시하지 않는다.
15. local worktree/commit/branch와 remote push·PR/check/merge 상태가 분리되고 remote effect는 action별 승인·before/after snapshot을 가진다.
16. partial·rollback required·held·outcome unknown participant가 Goal 완료를 막고 ChangeBundleReleaseHandoff가 project별 immutable revision·artifact·Gate를 연결한다.
17. local_quick·target·full·release 계층이 같은 Task·source·config·Catalog·Tool·Profile identity를 유지하고 final artifact는 한 번만 build·package한다.
18. `ready`, `approved`, `published`, `publish_outcome_unknown`, `rollback_required`가 분리되며 verified remote after-state 없이 published가 없다.
19. EvaluationRun은 CLI-only와 Codex-integrated context를 분리하고 실제 결함·false positive·flaky·suppression·재작업·실패·검증된 비용을 comparable case에서만 비교한다.
20. Rule·Check·Profile·Recipe deprecation은 validator guard를 약화하지 않는 migration·replacement evidence를 요구하며 결과 DB가 Catalog source를 직접 수정하지 않는다.
