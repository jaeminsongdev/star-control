# 공통 개발 관리와 로컬 관리 DB 계약

## 상태와 목적

이 문서는 Star-Control 0단계인 공통 개발 관리 계약과 로컬 관리 DB 기반의 의미 정본이다. 이후 scanner, validator, patch 도구, CLI와 Codex 진입점은 이 계약을 공유한다.

현재 상태는 **설계 확정, P0 첫 수직 Slice와 P-0054 실사용 전 복구 Slice 구현·로컬 검증 완료**다. P-0054는 typed backup/restore/rebuild/local-state plan·apply, recovery-only Controller·CLI, active-set 원자 활성화와 disposable 손상 복구 Corpus까지 구현했다. 이 문서는 전체 목표 계약을 소유하며 P-0054보다 넓은 1~11단계 lifecycle·query 항목은 [최종 구현 로드맵](../roadmap/final-implementation.md)과 `PLANS.md`에서 후속 범위로 구분한다. 저장소 topology와 운영 기본값은 [ADR-0007](../decisions/ADR-0007-P0-하이브리드-저장소와-운영-정책.md)이 확정한다.

이 문서가 소유하는 범위는 다음과 같다.

- Project부터 GateDecision까지 이어지는 공통 개발 관리 개념과 불변식
- stable ID, fingerprint, project-relative path, source revision과 config fingerprint의 연결
- Git 정본, 로컬 관리 DB와 `.ai-runs` 증거의 책임 분리
- Controller 단일 Writer, application service와 repository interface 경계
- DB version, migration, backup, 재구축, 손상 감지, 읽기 전용 복구와 retention 원칙
- global store와 project store를 함께 사용할 때의 격리·조정·redaction

설정 key와 병합 규칙은 [설정과 Catalog 계약](config-and-catalog.md), 1단계의 ProjectCheckout·ProjectCatalogSnapshot·CodeIndexSnapshot·tier·freshness는 [Project Catalog·Code Index 계약](project-catalog-and-code-index.md), event와 materialized state 경계는 [이벤트와 상태 계약](events-and-state.md), 큰 출력과 gate 연결은 [검사·완료·증거](validation-and-evidence.md)가 소유한다. 4단계의 selector 정확도, rewrite 보장 수준, dry-run·idempotence·apply·복구 알고리즘은 [안전한 Patch·Refactor·codemod 엔진 계약](safe-patch-and-codemod.md)이 소유한다. 5단계 shared symbol 값의 분류·manifest·binding·lifecycle·consumer와 DB stale 판정은 [관리형 Symbol Registry 계약](managed-symbol-registry.md)이 소유한다.

이 문서의 `star.*` persisted top-level 문서는 [데이터 계약 지도](README.md)의 공통 Envelope를 사용한다. 아래 field 표는 공통 `schema_id`, `schema_version`, document metadata와 producer를 반복하지 않고 domain field만 적는다. Catalog 선언인 Rule·ChangeRecipe는 Catalog 공통 descriptor metadata를 사용한다.

## 세 저장 계층과 정본 경계

| 계층 | 역할 | 정본 성격 | 대표 자료 |
|---|---|---|---|
| Git의 선언·Schema·Catalog·source | 팀이 검토·공유하는 의도와 구현 | 공유 정본 | `.star-control/project.toml`, config, Rule·ChangeRecipe·Managed Registry 선언, suppression·baseline 선언, source code, contract type, generated Schema |
| 로컬 관리 DB | 검색·관계·현재 상태·이력 조회와 로컬 운영 상태 | source에서 다시 계산 가능한 projection 또는 명시적인 local-only state | Project 등록 projection, revision·snapshot, scan generation, symbol graph, ManagedRegistrySnapshot, Finding, local Disposition, active ChangePlan, event·idempotency index |
| 대상 프로젝트의 `.ai-runs` | 크고 독립적으로 검증할 수 있는 실행 증거 | hash로 고정된 evidence | diff, patch, stdout·stderr, trace, report, screenshot, scan export, validation output |

다음 규칙은 예외 없이 적용한다.

1. source code, Rule 정의, ChangeRecipe 정의, ManagedDeclaration과 공유 설정을 DB에서만 만들거나 DB 값으로 덮어쓰지 않는다.
2. DB를 잃어도 연결된 project root와 Git 정본을 다시 제공하면 source-derived projection을 새 scan으로 재구축할 수 있어야 한다.
3. local-only Disposition, local Suppression, 진행 중 ChangePlan과 event 시각·actor 이력은 source만으로 재현되지 않는다. 보존이 필요하면 backup 또는 redacted export가 필요하다.
4. `.ai-runs`의 큰 byte를 DB blob으로 복제하지 않는다. DB에는 ArtifactRef와 조회에 필요한 작은 metadata만 둔다.
5. DB row, cache와 report는 source의 의미를 바꾸지 않는다. 충돌하면 Git 정본을 다시 읽고 projection을 폐기하거나 재구축한다.
6. DB는 application 내부 저장 방식이다. public CLI·MCP·contract에 SQL, table, connection string, database filename이나 backend 이름을 노출하지 않는다.

### 하이브리드 logical store

로컬 관리 DB는 하나의 파일을 뜻하지 않는다. Controller가 한 Writer lease 아래 다음 logical store를 조립한다.

| store | 소유 자료 | 두지 않는 자료 |
|---|---|---|
| global store | Project directory projection, `root_binding_id`, identity scope, project-store locator, cross-project relation, `CoordinatedOperation`, global idempotency와 lifecycle summary; M2 target의 TaskSpec·ScopeRevision·ImpactAnalysis summary·ValidationPlan | project source 절대 경로, source byte, project별 scan graph·Finding·ImpactEdge detail |
| project store | 해당 ProjectId의 revision·snapshot·source graph·scan·finding·local decision·ChangeSet·ImpactEdge·ChangePlan·validation·gate·ArtifactRef index | 다른 project의 detail, 다른 root binding, cross-project coordinator의 최종 상태 |

이 표의 P0 field는 구현된 v1 배치다. P-0041~P-0043은 v2 migration, ProjectCheckout attachment, ProjectCatalogSnapshot·CodeIndexSnapshot partition과 global planning coordinator의 첫 bounded Slice를 구현했다. 모든 version에서 source byte는 저장하지 않으며 후속 field 확장은 공개 migration 없이 기존 row에 덧씌우지 않는다.

기본 관리 root는 `%LOCALAPPDATA%\Star-Control\management\`다. global store generation은 `global/` 아래, project store generation은 `projects/<project-id>/` 아래 둔다. `<project-id>` 외에 project 이름·repository 이름·사용자 이름·source path segment를 directory 이름에 사용하지 않는다. 이 위치는 배포 adapter 기본값이며 public config에 DB filename이나 backend를 노출한다는 뜻이 아니다.

Project의 상세 상태는 project store가 소유하고 global store의 Project directory는 검색·연결용 작은 projection이다. global projection이 project store와 어긋나면 project store와 Git 선언을 다시 읽어 projection을 고치며, global row를 source 사실로 간주하지 않는다.

한 store의 손상은 다른 store를 자동으로 suspect 처리하지 않는다. 다만 완료되지 않은 cross-store operation이나 generation set을 참조하면 관련 participant만 `recovery_required`로 표시한다.

공유 선언의 기본 위치는 다음과 같다.

| 선언 | Git 정본 위치 |
|---|---|
| Project | `.star-control/project.toml` |
| project config·scan scope | `.star-control/config.toml` |
| project Rule enable·parameter | `.star-control/rules.toml` |
| shared Suppression | `.star-control/suppressions.toml` |
| shared Baseline | `.star-control/baselines/*.toml` |
| project ChangeRecipe | `.star-control/change-recipes/*.toml` |
| Managed Registry root·fragment | `.star-control/managed-registry/manifest.toml`, root `declaration_files`가 명시한 `.star-control/managed-registry/declarations/<fragment>.toml` |
| built-in Rule·Recipe | `catalog/validators`, `catalog/change-recipes` |

DB는 이 파일을 수정하지 않는다. local decision을 shared 선언으로 승격할 때 application service가 별도 PatchSet을 만들고 일반 source 변경·검증·승인 흐름을 거친다.

### shared decision 선언 wire format

`.star-control/suppressions.toml`은 정확히 다음 container를 사용한다.

```toml
schema_version = 1

[[suppressions]]
schema_id = "star.suppression"
schema_version = 1
suppression_id = "sup_01J..."
revision = 1
scope_kind = "shared"
project_id = "prj_01J..."
selector = "rule:star.rule.trailing-whitespace"
reason_code = "REVIEWED_EXCEPTION"
reason = "reviewed temporary exception"
created_at = "2026-07-12T00:00:00Z"
expires_at = "2026-10-10T00:00:00Z"
permanent = false
status = "active"
provenance = "git:.star-control/suppressions.toml"
```

`.star-control/baselines/*.toml`은 file 하나가 `star.baseline` document 하나다. `schema_id`, `schema_version=1`, `baseline_id`, `revision`, `scope_kind="shared"`, Project·revision·snapshot ID, scan·Rule fingerprint, 정렬된 `finding_fingerprints`, `set_fingerprint`, `created_at`, `reason`, `reviewed=true`, `status`를 top-level key로 둔다. 같은 file에 여러 Baseline을 배열로 넣지 않는다.

shared declaration parser는 unknown·duplicate field, 미래 version, 다른 ProjectId, local scope, 중복 stable ID, invalid expiry·review, secret·사용자 이름·절대 경로·민감 literal을 fail-closed로 거부한다. 선언이 invalid면 이전 shared projection을 current 판단에 재사용하지 않고 빈 shared projection으로 교체하며 ScanRun을 `incomplete`로 남긴다. local-only ProjectId가 shared declaration을 가지는 경우도 같은 방식으로 거부한다.

유효한 shared set은 source fingerprint와 함께 한 project-store transaction에서 active projection 전체를 교체한다. Git에서 삭제된 선언은 다음 scan부터 DB active projection에서도 사라진다. shared revision history의 정본은 Git이고, DB는 current 검색 projection만 가진다. local Suppression·Disposition revision history는 별도 local operational state로 보존한다.

## 실행 진입점과 단일 Writer

### 공통 application service

CLI와 이후 Codex 연동은 같은 typed application command와 query를 호출한다.

```text
star CLI ───────┐
                ├─ local IPC ─> Controller ─> ManagementApplicationService
star MCP ───────┘                                 │
                                                  ├─ domain services
향후 Codex entry ─ local IPC 또는 내부 adapter ───┤
                                                   ├─ ManagementRepositorySet port
                                                  └─ ArtifactStore port
```

- `ManagementApplicationService`는 project 등록, scan, finding·Registry derived snapshot 조회·판정, change plan, patch, validation, gate, cross-store coordination과 store lifecycle use case를 소유한다.
- CLI-only command graph에는 Codex, Codex App Server, 다른 AI provider와 OpenAI API client port가 존재하지 않는다.
- 향후 Codex 연동은 별도 entry adapter다. 같은 command DTO와 application service를 호출하며 별도 DB access, 별도 scanner, 별도 gate engine을 만들지 않는다.
- MCP handler와 CLI handler는 argument parse, IPC 변환과 표시만 담당한다. DB 파일, artifact 파일과 project source를 직접 읽거나 쓰지 않는다.
- Controller 한 process만 read-write repository를 열 수 있다. 다른 Controller가 writer lease를 얻지 못하면 시작을 거부한다.
- query도 Controller를 통한다. redaction, store revision, project 격리와 read-only recovery 상태를 우회하는 offline query path를 public CLI에 두지 않는다.

### application command 최소 집합

아래 `mutation`은 local management projection·decision·evidence 변경을 뜻한다. Project source·Git metadata 변경 여부와는 다르다. 1단계의 `project.discover`부터 `graph.neighbors`까지, 2단계 planning과 5단계 Registry query·change-plan command는 모두 `source_effect=none`이며 DTO와 dependency graph에 patch·write-capable filesystem·Git mutation port가 없다. source 변경은 별도 M4 PatchApplication만 수행한다. 다음 표의 1·2·5단계 행은 **목표 계약이며 현재 구현 완료를 뜻하지 않는다**.

| command | 주요 입력 | 성공 결과 | mutation |
|---|---|---|---:|
| `project.register` | shared 또는 local-only ProjectId, protected root binding, expected global revision | Project | 예 |
| `project.discover` | root binding set, mode, discovery config fingerprint | ProjectCatalogSnapshot | 예 |
| `project.list`, `project.get` | catalog snapshot, filter, stable cursor | Project·ProjectCheckout view | 아니요 |
| `project.refresh` | ProjectId, CheckoutId, source precondition | ProjectRevision, WorkspaceSnapshot | 예 |
| `scan.plan` | ProjectId, CheckoutId, mode, effective config | scope·partition·limit plan | 아니요 |
| `scan.run` | plan fingerprint, mode, idempotency key | ScanRun·CodeIndexSnapshot ref | 예 |
| `scan.status` | ScanRunId 또는 ProjectId | partition·coverage·limitation | 아니요 |
| `index.status` | ProjectId, CheckoutId | snapshot·freshness·coverage | 아니요 |
| `index.search`, `index.definitions`, `index.references` | query/entity, scope, tier, freshness policy, cursor | quality envelope이 있는 match·edge | 아니요 |
| `graph.neighbors` | node key, relation, bounded depth, cursor | evidence가 있는 graph edge | 아니요 |
| `registry.status`, `registry.list`, `registry.candidates` M5 | ProjectId, current source/snapshot policy, filter, cursor | source hash·freshness·분류·binding·consumer view | 아니요 |
| `registry.change.plan` M5 | ManagedDeclarationChangeIntent, current snapshot, expected source hash | M2 ImpactAnalysis·ChangePlan·ValidationPlan ref | 예 |
| `task.create`, `task.revise` | 사용자 목표·Project·include/exclude·완료 조건 | TaskSpec revision | 예 |
| `scope.resolve` | TaskSpec, current ProjectCatalogSnapshot, user decision | ScopeRevision | 예 |
| `changes.collect` | TaskSpec·ScopeRevision, project별 Revision·WorkspaceSnapshot·comparison scope | ChangeSet | 예 |
| `impact.analyze` | ScopeRevision, ChangeSet, current CodeIndexSnapshot set | ImpactAnalysis | 예 |
| `affected.select` | ImpactAnalysis, CatalogSnapshot, previous result refs | ValidationPlan | 예 |
| `finding.list`, `finding.get` | ProjectId, filter, page cursor | redacted Finding·Occurrence view | 아니요 |
| `suppression.put`, `baseline.put`, `disposition.set` | target fingerprint, scope, reason, expected revision | 새 decision revision | 예 |
| `change.plan` P0 v1 | FindingId set, target snapshot, ChangeRecipe refs | ChangePlan v1 | 예 |
| `change.plan` M2 v2 | TaskSpec·ScopeRevision·ImpactAnalysis·ValidationPlan refs | ChangePlan v2 | 예 |
| `recipe.validate`, `recipe.describe` M4 | Recipe source 또는 exact ID/version | 정적 validation·resolved descriptor | 아니요 |
| `patch.prepare` P0 v1 | ChangePlan revision, parameters | PatchSet v1 | 예 |
| `patch.prepare` M4 | ChangePlan v2, Recipe·typed input·TargetSelector, current snapshots | RecipeExecution·PatchSet v2 preview | 예 |
| `patch.apply` M4 | PatchSet v2 fingerprint, pre-apply Gate, workspace precondition, approval | PatchApplication·새 WorkspaceSnapshot | 예 |
| `patch.recover` M4 | PatchApplication, actual reconciliation, reverse/discard strategy | 새 recovery revision | 예 |
| `validation.run` | PatchSet 또는 WorkspaceSnapshot, ValidationPlan | ValidationResult | 예 |
| `gate.evaluate` | target revision, ValidationResult refs, policy | GateDecision | 예 |
| `management.status` | 없음 | normal StoreStatus summary 또는 recovery-only RecoveryStatus와 allowed operation | 아니요 |
| `management.backup.plan` | management root 밖의 기존 destination | active-set·store vector·destination에 고정된 BackupPlan | 아니요 |
| `management.backup.apply` | BackupPlan, exact plan fingerprint 승인 | 검증된 BackupSetManifest와 BackupApplyResult | 예 |
| `management.restore.plan` | verified backup-set root | side-by-side candidate와 candidate active-set에 고정된 RestorePlan | 아니요 |
| `management.restore.apply` | RestorePlan, exact plan fingerprint 승인 | 원자 활성화된 RestoreApplyResult | 예 |
| `management.rebuild.plan` | protected root binding·source revision·config·artifact inventory | 재구축 Project와 local-only 손실에 고정된 RebuildPlan | 아니요 |
| `management.rebuild.apply` | RebuildPlan, exact plan fingerprint 승인 | 새 source-derived projection·검증된 ArtifactRef index·loss report | 예 |
| `management.local-state.export.plan/apply` | ProjectId, destination, exact plan fingerprint 승인 | redacted LocalStateBundle과 export result | plan은 아니요, apply는 예 |
| `management.local-state.import.plan/apply` | bundle, current source/config/store revision, exact plan fingerprint 승인 | conflict 또는 imported local-state result | plan은 아니요, apply는 예 |
| `management.migrate` | expected StoreStatus, 승인 범위 | lifecycle result | 예 |
| `management.retention.plan` | policy snapshot | 삭제 후보와 영향 | 아니요 |
| `management.retention.apply` | plan fingerprint, approval | 적용 결과 | 예 |

일반 application command가 두 store 이상을 바꾸면 성공 결과에 `coordinated_operation_id`와 최종 `StoreVersionVector`를 포함한다. generation 전체를 교체하는 backup·restore·rebuild는 별도 lifecycle 계약으로 active-set·backup-set fingerprint와 typed result를 남긴다. 어느 경로든 `completed`가 아닌 operation을 성공으로 표시하지 않는다.

일반 document mutating command는 `idempotency_key`와 stale-write precondition을 가진다. 일반 document 갱신은 `expected_revision`, cross-store command는 `expected_version_vector`, patch는 exact base·before hash와 승인 fingerprint를 precondition으로 사용한다. lifecycle apply는 destination·source/store vector·revision을 포함한 exact plan fingerprint가 idempotency key 역할을 하며 private `recovery-receipts`의 typed result로 crash 뒤 같은 요청을 재생한다. 같은 canonical 요청은 기존 결과를 반환하고 다른 payload·stale state는 충돌로 거부한다. 순수 plan query와 terminal result query에는 별도 idempotency key가 필요 없다.

## 공통 식별과 fingerprint

### ID 분류

| 분류 | 예 | 생성·보존 규칙 |
|---|---|---|
| 공유 선언 ID | ProjectId, RuleId, ChangeRecipeId | Git·Catalog 정본에 선언하며 rename·path와 분리 |
| source-derived ID | ProjectRevisionId, WorkspaceSnapshotId, CanonicalSourceId, SymbolId, SymbolReferenceId, FindingId, OccurrenceId | 아래 identity payload의 full SHA-256에서 결정적으로 생성 |
| 실행 instance ID | ScanRunId, ValidationResultId, PatchSetId | Controller가 재사용하지 않는 typed ID 생성 |
| local decision ID | SuppressionId, BaselineId, DispositionId, ChangePlanId | source 선언이면 선언 ID, local-only면 Controller 생성 |
| DB 내부 key | row key, page key | repository adapter private이며 외부 reference로 사용 금지 |

source-derived ID 형식은 `<prefix><base32>`다. prefix 자체가 `_`로 끝나며 `base32`는 versioned identity payload의 **전체 256-bit SHA-256**을 lowercase RFC 4648 base32, padding 없이 표현한 52자다. digest를 자르거나 다시 hash하지 않는다. 실행 instance와 local decision ID는 26자 uppercase Crockford ULID suffix를 쓰며 두 형식을 parser·Schema에서 혼용하지 않는다.

1단계가 추가하는 CheckoutId(`cko_` ULID), ProjectCatalogSnapshotId(`pcs_` derived)와 CodeIndexSnapshotId(`cix_` derived)의 payload는 [Project Catalog·Code Index 계약](project-catalog-and-code-index.md#새-persisted-계약)이 소유한다. P0 type에 이름만 비슷한 별도 ID를 추가하지 않는다.

Controller instance·local ID prefix는 ScanRun `scn_`, ValidationResult `vrs_`, PatchSet `pat_`, local Suppression `sup_`, Baseline `bas_`, Disposition `dsp_`, ChangePlan `cpl_`, CoordinatedOperation `cop_`, ManagementStore `mst_`다. suffix는 같은 type에서 재사용하지 않는 time-sortable 128-bit value이며 timestamp 의미를 권한·인과 판단에 사용하지 않는다. Occurrence `occ_`는 source-derived full-digest 형식이다.

| type | prefix |
|---|---|
| ProjectRevisionId | `prv_` |
| WorkspaceSnapshotId | `wsp_` |
| CanonicalSourceId | `src_` |
| SymbolId | `sym_` |
| FindingId | `fnd_` |
| SymbolReferenceId | `srf_` |
| OccurrenceId | `occ_` |

derived ID별 v1 identity payload는 다음 field를 정확히 사용한다.

| ID | `contract` | `inputs` |
|---|---|---|
| ProjectRevisionId | `star.identity.project-revision` | `project_id`, `revision_kind`, Git이면 `vcs_object_format`·`commit_id`·`tree_id`, non-Git이면 `manifest_fingerprint` |
| WorkspaceSnapshotId | `star.identity.workspace-snapshot` | `project_id`, `project_revision_id`, normalized `scope`, `entries_fingerprint`, `ignored_policy`, `symlink_policy`, `completeness` |
| CanonicalSourceId | `star.identity.canonical-source` | `project_id`, `source_kind`, file이면 normalized `path`, virtual이면 producer가 선언한 stable key |
| SymbolId | `star.identity.symbol` | `project_id`, `language_id`, `symbol_kind`, redacted canonical `qualified_name`, `canonical_source_id`, optional `signature_fingerprint` |
| FindingId | `star.identity.finding` | `project_id`, `rule_id`, `identity_contract_version`, `identity_anchor`, sorted typed `identity_tokens` |
| SymbolReferenceId | `star.identity.symbol-reference` | `project_id`, `from_source_id`, optional `from_symbol_id`, normalized `from_range`, `reference_kind`, resolved target ID 또는 redacted unresolved target |
| OccurrenceId | `star.identity.occurrence` | `finding_id`, `workspace_snapshot_id`, `source_content_sha256`, normalized `location_range` (`start_line`, `start_column`, `end_line`, `end_column`), redacted `evidence_key` |

각 fingerprint는 JCS object `{"algorithm": <contract>, "contract_version": 1, "payload": <inputs>}`의 SHA-256이다. source-derived ID는 그 digest byte를 직접 base32로 표현한다. 표에 없는 field를 편의상 추가하거나 누락하면 같은 ID contract가 아니다. optional input이 없으면 key 자체를 생략하며 `null`이나 빈 문자열을 넣지 않는다.

같은 derived ID에 서로 다른 identity payload가 관찰되면 merge하지 않고 `MANAGEMENT_IDENTITY_CONFLICT`로 store를 suspect 상태에 둔다.

### canonical hash 규칙

모든 management fingerprint는 [데이터 계약 지도](README.md)의 canonical JSON 규칙과 RFC 8785 JCS를 사용한다.

```text
fingerprint = "sha256:" + lowercase_hex(
  SHA-256(
    JCS({
      "algorithm": "<fingerprint-contract-id>",
      "contract_version": 1,
      "payload": { ...고정 field... }
    })
  )
)
```

- timestamp, display text, 절대 경로, DB key, cache hit 여부와 producer build는 identity fingerprint에서 제외한다.
- `project_id`, 정규화된 project-relative path, source hash, rule·recipe version과 관련 config fingerprint는 해당 계약이 요구하는 범위에서 포함한다.
- secret, 사용자 이름과 민감 literal은 hash input에도 넣지 않는다. hash로 바꿨다는 이유로 저장을 허용하지 않는다.
- fingerprint input field를 바꾸면 fingerprint contract version을 올린다. 과거 fingerprint를 새 의미로 재해석하지 않는다.
- 모든 contract는 `identity_fingerprint`와 변경 가능한 내용을 나타내는 `content_fingerprint`를 구분한다.

### source revision과 config 연결

모든 scan·finding·patch·validation 결과는 다음 사슬 중 필요한 항목을 빠짐없이 가진다.

```text
ProjectId
  -> ProjectRevisionId
  -> WorkspaceSnapshotId
  -> CanonicalSourceId + source content SHA-256
  -> ScanRun(input_fingerprint)
       = WorkspaceSnapshotId
       + scan_config_fingerprint
       + rule_set_fingerprint
       + scanner_contract_version
  -> FindingId -> OccurrenceId
  -> ChangePlan -> PatchSet
  -> ValidationResult -> GateDecision
```

`effective_config_fingerprint`는 전체 EffectiveConfig를 고정한다. `scan_config_fingerprint`는 scan 결과에 영향을 주는 path scope, limits, redaction version, Rule parameters와 incomplete policy만 포함한다. retention과 terminal 표시 설정처럼 scan 결과를 바꾸지 않는 값은 제외한다. 두 fingerprint를 모두 ScanRun에 저장한다.

## Project와 source snapshot

### Project — `star.project`

Project는 하나의 관리 대상 source 경계다. Goal 안에서만 쓰는 기존 ProjectRef는 이 문서의 Project를 가리키는 작은 reference다. 아래 표는 구현된 P0 `star.project` schema v1 shape다.

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `project_id` | 예 | shared manifest에 선언하거나 Controller가 local-only로 발급한 stable ProjectId |
| `identity_scope` | 예 | `shared`, `local`; manifest로 검증된 경우에만 `shared` |
| `display_name` | 예 | redaction을 통과한 project 표시 이름; OS 사용자 이름에서 자동 생성하지 않고 identity에 사용하지 않음 |
| `repository_kind` | 예 | `git`, `none` |
| `source_of_truth` | 예 | 이 project가 소유하는 contract·source 범위 |
| `declaration_fingerprint` | 예 | project 선언의 canonical hash |
| `registration_state` | 예 | `attached`, `detached`, `invalid` |
| `root_binding_id` | attached일 때 | raw path가 아닌 current-user protected binding |
| `latest_revision_id` | 아니요 | 마지막 확인 ProjectRevision |
| `latest_workspace_snapshot_id` | 아니요 | 마지막 실제 workspace 관찰 |

`project_id`가 없는 폴더는 Controller가 새 local-only ProjectId를 발급해 즉시 local scan을 시작할 수 있다. local-only ID는 다른 PC에서 같은 project라는 보장이 없고 shared Baseline·Suppression 또는 장기 cross-project identity의 대상이 될 수 없다.

local-only Project를 공유할 때는 같은 ID를 담은 `.star-control/project.toml` PatchSet을 preview하고 명시적으로 적용·검증한다. 검증된 manifest의 `project_id`가 local ID와 정확히 일치한 뒤에만 `identity_scope=shared`로 바꾼다. 다른 ID가 이미 선언돼 있으면 자동 병합하지 않고 identity conflict로 중단한다.

P0 v1 ProjectRef의 persisted root reference는 raw LocalPathRef가 아니라 `root_binding_id`로 좁힌다. 실제 절대 root는 Controller process memory에서만 ProjectPathRef 해석에 사용한다.

#### 1단계 checkout 분리 target

복수 clone·main/linked worktree를 지원하려면 Project의 공유 identity와 local attachment를 분리해야 한다. 1단계 `star.project` v2는 `root_binding_id`를 제거하고 `attached_checkout_ids`와 derived `registration_state`를 가지며, 각 binding·Git common/worktree observation은 `star.project-checkout`이 소유한다. ProjectRef v2도 `checkout_id`를 사용한다.

P-0041은 기존 v1 attached row 하나를 primary ProjectCheckout 하나로 옮기는 lossless migration, old-version fixture, pre-migration backup·dry-run·resume·rollback을 구현했다. binding이 없거나 manifest ProjectId가 충돌하면 자동으로 여러 checkout을 만들지 않고 `detached` 또는 `PROJECT_CHECKOUT_IDENTITY_CONFLICT`로 중단한다. 정확한 field와 identity는 [Project Catalog·Code Index 계약의 호환성 gap](project-catalog-and-code-index.md#0단계-선행조건과-호환성-gap)을 따른다.

최소 shared 선언은 다음 의미를 가진다.

```toml
schema_version = 1
project_id = "prj_01J..."
display_name = "Star-Control"
repository_kind = "git"
source_of_truth = ["source", "contracts", "catalog"]
```

`project_id` 변경은 rename이 아니라 새 Project다. `display_name` 변경은 identity를 바꾸지 않는다. 알 수 없는 field, 중복 key와 개인 절대 경로는 선언 전체 오류다.

### ProjectRevision — `star.project-revision`

ProjectRevision은 수정되지 않은 source 기준점이다.

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `project_revision_id` | 예 | identity payload에서 결정 |
| `project_id` | 예 | 대상 Project |
| `revision_kind` | 예 | `git_commit`, `filesystem_manifest` |
| `vcs_object_format` | Git일 때 | `sha1`, `sha256` 등 실제 Git object format |
| `commit_id`, `tree_id` | Git일 때 | 검증된 object ID |
| `manifest_fingerprint` | non-Git일 때 | sorted ProjectPathRef·content hash manifest |
| `captured_at` | 예 | 관찰 시각, identity에는 제외 |
| `completeness` | 예 | `complete`, `partial`, `unverified` |
| `limitations` | 필요 시 | shallow clone, unreadable file 등 |

Git dirty state와 untracked file은 ProjectRevision에 섞지 않고 WorkspaceSnapshot이 소유한다.

### WorkspaceSnapshot — `star.workspace-snapshot`

WorkspaceSnapshot은 scan·patch 시점에 실제로 읽은 workspace의 immutable 관찰이다.

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `workspace_snapshot_id` | 예 | 전체 snapshot identity에서 결정 |
| `project_id`, `project_revision_id` | 예 | source 기준 |
| `scope` | 예 | include·exclude ProjectPathRef/glob의 정규화 결과 |
| `entries_manifest_ref` | 예 | path·kind·mode·size·content hash 목록 ArtifactRef |
| `entries_fingerprint` | 예 | 정렬된 manifest의 hash |
| `dirty_summary` | 예 | modified·added·deleted·untracked count |
| `ignored_policy`, `symlink_policy` | 예 | 실제 적용한 scan policy |
| `captured_at` | 예 | 관찰 시각 |
| `completeness`, `limitations` | 예 | 누락과 원인 |

WorkspaceSnapshot ID에는 `captured_at`을 넣지 않는다. 같은 source byte와 scope는 같은 ID가 된다. source file byte를 DB에 넣지 않고 entries metadata와 ArtifactRef만 저장한다.

### CanonicalSource — `star.canonical-source`

CanonicalSource는 한 Project 안에서 file·generated unit·virtual document를 안정적으로 참조하는 source identity다. 외부 자료를 뜻하는 SourceRecord와 다른 계약이다.

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `canonical_source_id` | 예 | ProjectId, source kind와 normalized relative identity에서 결정 |
| `project_id` | 예 | partition key |
| `path` | file일 때 | slash 기반 ProjectPathRef |
| `source_kind` | 예 | `file`, `generated_unit`, `virtual` |
| `language_id` | 아니요 | Catalog stable ID |
| `content_sha256` | 관찰 시 | 실제 읽은 byte hash |
| `project_revision_id`, `workspace_snapshot_id` | 관찰 시 | source context |
| `generated_from_refs` | 필요 시 | 입력 CanonicalSource reference |
| `sensitivity` | 예 | source metadata의 노출 등급 |

line, column과 timestamp는 CanonicalSource identity에 포함하지 않는다. rename은 새 CanonicalSource이며 adapter가 확인한 경우에만 `supersedes` 관계를 별도 기록한다.

## Rule, scan과 code graph

### Rule — `star.rule`

Rule은 source를 읽어 Occurrence를 생산하는 versioned 선언이다. 실행 code나 SQL을 DB에 저장하지 않는다.

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `rule_id` | 예 | namespace가 있는 stable Catalog ID |
| `rule_version` | 예 | SemVer |
| `definition_fingerprint` | 예 | 실행에 영향을 주는 선언 전체 hash |
| `title`, `category` | 예 | 표시와 분류 |
| `default_severity`, `default_confidence` | 예 | 결과 기본값 |
| `supported_languages`, `source_kinds` | 예 | 입력 범위 |
| `analyzer_ref` | 예 | built-in analyzer 또는 trusted ToolDescriptor |
| `parameter_schema_ref` | 예 | typed parameter 계약 |
| `identity_contract_version` | 예 | Finding identity input version |
| `identity_anchor` | 예 | `symbol`, `source`, `project` 중 허용 방식 |
| `redaction_contract_version` | 예 | message parameter 가림 규칙 |
| `remediation_recipe_refs` | 예 | 적용 가능한 ChangeRecipe 목록 |
| `lifecycle` | 예 | `active`, `deprecated`, `disabled` |

Rule set fingerprint는 활성 Rule의 `rule_id`, `rule_version`, `definition_fingerprint`, effective parameter fingerprint를 ID 순으로 정렬해 계산한다.

#### M3 Rule v2 target

P0 Rule v1은 source Finding producer를 표현한다. M3의 test·architecture·hardcoding·docs·security와 validation meta Diagnostic도 같은 stable RuleRef를 사용해야 하므로 `star.rule` v2는 다음 conditional field를 추가한다. 이 target은 아직 Schema·migration·제품 code에 구현되지 않았다.

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `rule_domain` | 예 | `scan_finding`, `validation_diagnostic`, `both` |
| `producer_kind` | Diagnostic domain이면 | `built_in_analyzer`, `external_check_mapping`, `normalizer`, `gate_evaluator` |
| `producer_ref` | Diagnostic domain이면 | built-in producer ID 또는 CheckDescriptor의 mapping ref. 실행 code·command text는 아님 |
| `applies_to_check_families` | Diagnostic domain이면 | 이 Rule을 생산할 수 있는 stable Check family set |
| `fingerprint_contract_version` | Diagnostic domain이면 | Diagnostic stable problem key input version |
| `location_contract`, `evidence_contract` | Diagnostic domain이면 | 허용 LocationRef·EvidenceRef variant와 필수 evidence |
| `remediation_contract` | Diagnostic domain이면 | 허용 action kind·target selector·재검사 family |
| `gate_floor` | Diagnostic domain이면 | default severity/confidence, protected block/review floor와 ratchet eligibility |
| `fixture_manifest_refs` | Diagnostic domain이면 | positive·negative·edge·regression, 필요 시 adversarial fixture |

`scan_finding|both`는 v1의 language/source/analyzer, `identity_contract_version`과 identity anchor를 계속 요구한다. `validation_diagnostic|both`는 위 Diagnostic field를 요구한다. `validation_diagnostic`만인 Rule에 가짜 Finding identity나 ScanRun analyzer를 합성하지 않는다.

RuleRef는 두 domain 모두 `rule_id`, `rule_version`, `definition_fingerprint`, 그리고 사용하는 identity/fingerprint contract version을 고정한다. 같은 Rule ID·version에 다른 producer, severity floor, mapping, fingerprint 또는 fixture manifest가 있으면 Registry conflict다. Rule descriptor에는 raw shell·script, parser code, SQL, AI prompt와 source replacement를 넣지 않는다.

ScanRun의 `rule_set_fingerprint`에는 `scan_finding|both` Rule의 scan 의미만 포함한다. M3 ValidatorRegistry fingerprint에는 `validation_diagnostic|both` Rule의 Diagnostic 의미와 mapping·fixture를 포함한다. validation-only Rule 변경 때문에 과거 source ScanRun byte가 바뀐 것처럼 표시하지 않고, 두 fingerprint를 EvidenceSubjectBinding에서 별도 의미로 결합한다.

### ScanRun — `star.scan-run`

ScanRun은 한 snapshot과 rule set을 검사한 실행 사실이다.

P0 v1의 아래 필드는 유지한다. 1단계 target은 여기에 `checkout_id`, `project_catalog_snapshot_id`, `requested_mode`, `effective_mode`, `analysis_input_fingerprint`, `decision_projection_fingerprint`, `index_config_fingerprint`, `classification_fingerprint`, `adapter_set_fingerprint`, partition별 status·coverage와 `code_index_snapshot_id`를 추가한다. target extension은 [Project Catalog·Code Index 계약](project-catalog-and-code-index.md)에서 소유하며 현재 Schema에 이미 존재하는 것으로 읽지 않는다.

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `scan_run_id` | 예 | 실행마다 새 ID |
| `project_id`, `project_revision_id`, `workspace_snapshot_id` | 예 | 정확한 source 입력 |
| `effective_config_fingerprint`, `scan_config_fingerprint` | 예 | 설정 근거 |
| `rule_set_fingerprint` | 예 | Rule snapshot |
| `input_fingerprint` | 예 | snapshot·scan config·rule set·shared/local decision revision set·completeness input·scanner contract hash |
| `status` | 예 | `queued`, `running`, `succeeded`, `incomplete`, `failed`, `cancelled` |
| `generation_id` | 예 | staging과 atomic publish 경계 |
| `started_at`, `finished_at` | 상태별 | 실행 시각 |
| `reused_from_scan_run_id` | 아니요 | exact cache reuse 근거 |
| `counts` | 완료 시 | source, symbol, reference, occurrence, finding 수 |
| `limitations`, `error` | 필요 시 | 누락·실패 원인 |
| `artifact_refs` | 예 | 큰 report·raw output |

P0 v1 `input_fingerprint`는 command idempotency를 위해 source 분석 입력과 당시 shared/local decision revision set을 함께 hash한다. M1 `analysis_input_fingerprint`는 CodeIndexSnapshot·partition reuse용이라 mutable decision을 제외하고, `decision_projection_fingerprint`는 FindingView·gate가 join한 decision revision set을 고정한다. decision만 바뀌면 source partition을 다시 scan하지 않고 새 query view·event revision을 만들며, legacy `input_fingerprint`를 CodeIndex content identity로 사용하지 않는다.

같은 `idempotency_key`와 같은 input fingerprint의 재전송은 기존 ScanRun을 그대로 반환하며 새 artifact·generation을 만들지 않는다. 다른 command instance가 같은 성공 input을 cache로 재사용하면 새 ScanRun을 만들고 `reused_from_scan_run_id`를 기록한다. referenced artifact hash와 store generation이 유효하지 않으면 재사용하지 않는다.

scan 결과는 generation 단위로 쓴다.

1. `scan.started`와 invisible staging generation을 만든다.
2. batch마다 ordinal, batch fingerprint와 count를 idempotent하게 commit한다.
3. finalization에서 source completeness, Rule 오류, expected batch와 reference 무결성을 검사한다.
4. 한 transaction에서 generation을 visible로 전환하고 current Finding projection과 `scan.finished` event를 갱신한다.
5. 실패·취소·crash면 이전 visible generation을 유지한다. 미완료 staging은 retention 후보가 된다.

한 transaction에 안전하게 들어가는 작은 built-in scan은 ordinal 0의 단일 batch로 바로 finalization할 수 있다. 이때 transaction commit 전 row는 보이지 않고 crash 시 전부 rollback되어야 하며, “batch API를 호출하지 않았다”는 이유로 partial row를 visible하게 만들 수 없다.

secret, 사용자 이름, raw 절대 경로 또는 민감 literal 때문에 필수 Occurrence identity나 evidence를 안전하게 만들 수 없으면 원문과 그 hash를 persistence 전에 폐기하고 해당 item을 `quarantined`로 계수한다. ScanRun은 `incomplete`로 종료하며 이전 complete generation을 current로 유지한다. incomplete ScanRun은 GateDecision의 `auto_pass` 입력이 될 수 없다.

### Symbol — `star.symbol`

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `symbol_id` | 예 | project, language, kind, canonical qualified identity와 source anchor hash |
| `project_id`, `canonical_source_id` | 예 | 소속 |
| `language_id`, `symbol_kind` | 예 | Catalog stable 값 |
| `qualified_name` | 예 | redaction을 통과한 normalized name |
| `signature_fingerprint` | 아니요 | 언어 adapter가 안정적으로 계산할 수 있을 때 |
| `declaration_range` | 예 | 1-based, end-exclusive |
| `visibility` | 아니요 | 언어별 값을 normalized enum으로 변환 |
| `workspace_snapshot_id`, `scan_run_id` | 예 | 관찰 context |
| `content_fingerprint` | 예 | 현재 symbol metadata hash |

line 이동은 SymbolId를 바꾸지 않는다. rename, kind 또는 canonical signature identity 변경은 새 SymbolId다. 민감 이름이면 `qualified_name`을 저장하지 않고 redacted stable token을 사용하며 원문 hash도 저장하지 않는다.

### SymbolReference — `star.symbol-reference`

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `symbol_reference_id` | 예 | source·target·kind·source range identity hash |
| `project_id` | 예 | source project partition |
| `from_symbol_id` | 아니요 | file-level reference면 생략 가능 |
| `from_source_id`, `from_range` | 예 | reference 위치 |
| `to_symbol_id` | 해석 시 | resolved target |
| `unresolved_target` | 미해석 시 | redacted normalized token |
| `reference_kind` | 예 | call, import, read, write, inherit 등 |
| `resolution` | 예 | `resolved`, `ambiguous`, `unresolved`, `external` |
| `workspace_snapshot_id`, `scan_run_id` | 예 | 관찰 context |

다른 Project target은 상대 ProjectId와 exported symbol identity만 저장한다. 대상 root와 절대 경로를 복제하지 않는다.

## Finding과 decision

### Finding — `star.finding`

Finding은 여러 scan에서 이어지는 논리적 **관찰 또는 문제 후보** identity다. 원문 진단 instance인 Diagnostic과 달리 현재·과거 Occurrence를 묶는 materialized aggregate다. Rule 종류에 따라 결함이 아닐 수 있으며 hardcoding Finding은 별도 assessment·decision 없이 확정 문제로 승격하지 않는다.

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `finding_id` | 예 | finding identity fingerprint에서 결정 |
| `finding_fingerprint` | 예 | full SHA-256 |
| `project_id`, `rule_id`, `rule_version` | 예 | 소유 Rule |
| `identity_anchor` | 예 | SymbolId, CanonicalSourceId 또는 project |
| `identity_tokens` | 예 | Rule이 선언한 비민감 normalized 값만 |
| `title_code`, `message_code` | 예 | localizable template ID |
| `severity`, `confidence` | 예 | 현재 계산값 |
| `lifecycle` | 예 | `open`, `not_observed`, `resolved` |
| `first_observed_scan_id`, `last_observed_scan_id` | 예 | 관찰 경계 |
| `current_occurrence_ids` | 예 | latest visible generation의 occurrence |
| `active_disposition_id`, `active_suppression_ids` | 예 | decision projection |
| `content_fingerprint` | 예 | 현재 projection hash |

Finding identity payload는 `project_id`, `rule_id`, Rule identity contract version, anchor와 identity tokens를 포함한다. line, severity, message text, source revision과 config fingerprint는 제외한다. Rule version이 identity 의미를 바꾸면 identity contract version도 올라 새 Finding으로 분리한다.

### Occurrence — `star.occurrence`

Occurrence는 한 ScanRun에서 실제로 관찰한 위치와 증거다.

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `occurrence_id` | 예 | occurrence fingerprint에서 계산한 source-derived ID |
| `occurrence_fingerprint` | 예 | FindingId, snapshot, source hash, normalized range·evidence key |
| `finding_id`, `scan_run_id` | 예 | aggregate와 실행 |
| `project_revision_id`, `workspace_snapshot_id` | 예 | source context |
| `canonical_source_id`, `source_content_sha256` | 예 | 정확한 file byte |
| `location` | 예 | ProjectPathRef와 range |
| `symbol_id` | 아니요 | symbol anchor |
| `message_parameters` | 예 | allowlist·redaction을 통과한 typed 값 |
| `evidence_refs` | 예 | snippet·trace 등 큰 자료 |
| `observed_at` | 예 | ScanRun 시간 |

source snippet, 전체 matching line, stack trace와 tool raw output은 Occurrence row에 넣지 않는다.

### Suppression — `star.suppression`

Suppression은 Finding을 삭제하지 않고 표시·gate 해석을 제한하는 명시적 정책이다.

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `suppression_id`, `revision` | 예 | stable ID와 optimistic revision |
| `scope_kind` | 예 | `shared`, `local` |
| `project_id` | 예 | 다른 project에 적용 금지 |
| `selector` | 예 | 아래 v1 selector 문법의 Finding fingerprint, RuleId, SymbolId 또는 project-relative glob |
| `reason_code`, `reason` | 예 | 빈 이유 금지, 민감 literal 금지 |
| `created_at`, `expires_at` | 예 | 기본 만료 90일; permanent일 때만 `expires_at` 생략 |
| `permanent`, `justification` | 예/조건 | 기본 false; true면 별도 justification 필수 |
| `source_revision_constraint` | 아니요 | scope가 유효한 revision 범위 |
| `config_fingerprint_constraint` | 아니요 | 설정 의미 변경 시 재검토 |
| `status` | 예 | `active`, `expired`, `revoked`, `stale` |
| `provenance` | 예 | shared declaration ref 또는 local event ref |

v1 selector는 공백 없는 `<kind>:<value>` 하나이며 kind별 의미는 다음과 같다.

| kind | value | match |
|---|---|---|
| `finding` | 완전한 `sha256:...` Finding fingerprint | fingerprint exact match |
| `rule` | RuleId | RuleId exact match |
| `symbol` | SymbolId | Finding `identity_anchor` exact match |
| `path` | slash 기반 ProjectPathRef glob | 해당 Finding의 current Occurrence path 중 하나와 match |

path glob에서 `?`는 `/`가 아닌 한 byte, `*`는 한 segment 안의 0개 이상 byte, `**`는 `/`를 포함한 0개 이상 byte다. path는 UTF-8·slash 정규화 뒤 byte 단위로 비교하고 `..`, backslash, drive·UNC path는 parser 전에 거부한다. 알 수 없는 kind와 문법 오류는 match 없음으로 조용히 넘기지 않고 declaration을 invalid로 만든다.

shared suppression은 Git 선언이 정본이고 DB는 projection이다. local suppression은 DB가 local operational 정본이며 backup 없이 rebuild하면 잃는다. 생성 시 `expires_at`을 생략하면 effective policy의 90일 기본값을 적용한다. 영구 suppression은 `permanent=true`, 별도 justification과 명시적 승인을 모두 요구한다. suppression은 severity·Occurrence·ValidationResult를 바꾸지 않고 GateDecision이 적용 사실을 별도 기록한다. source revision·config constraint 불일치나 만료는 active match에서 제외하고 stale decision reference를 GateDecision 입력에 남긴다.

#### M3 Suppression v2 target

3단계 공통 Gate는 ValidationRun이 만든 Diagnostic에도 같은 예외 lifecycle을 사용한다. 따라서 `star.suppression` v2는 v1 Finding selector를 유지하면서 다음을 추가한다. 이 target은 아직 Schema·migration·제품 code에 구현되지 않았다.

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `subject_kind` | 예 | `finding`, `diagnostic` |
| `subject_fingerprint` | exact selector일 때 | Finding 또는 Diagnostic full SHA-256 |
| `rule_ref_constraint` | Rule selector일 때 | Rule ID·version·definition fingerprint·fingerprint contract version |
| `scope_constraint` | 예 | ProjectId, optional package/workspace, project-relative path/symbol과 Gate phase |
| `subject_binding_constraint` | 필요 시 | source·config·Catalog 의미가 바뀌면 stale로 만들 fingerprint subset |
| `approved_by` | 예 | ActorRef. raw 사용자 이름은 저장하지 않음 |
| `review_evidence_refs` | 예 | 사용자가 본 redacted Diagnostic·artifact·reason 근거 |

v2 selector kind에는 `diagnostic:sha256:...`을 추가한다. `rule:<RuleId>` suppression은 반드시 `rule_ref_constraint`, bounded `scope_constraint`, 이유와 `expires_at`을 가져야 한다. project 전체·모든 Rule wildcard, 빈 reason, fingerprint contract 없는 diagnostic suppression은 invalid다. permanent suppression은 v1과 같이 별도 justification·approval을 요구하며 validator guard 대상이다.

Gate engine은 `active|expired|stale|revoked|invalid`를 별도 DiagnosticEvaluation에 기록한다. expired·stale suppression은 원래 issue를 unsuppressed로 평가하고 history ref만 남긴다. suppression으로 Diagnostic·ValidationRun·ValidationResult를 `pass`로 다시 쓰지 않는다.

v1→v2 migration은 `finding:<fingerprint>`를 `subject_kind=finding` exact selector로 lossless 변환한다. Rule/path/symbol selector는 v1 decision 당시 보존된 Rule/Catalog·scope snapshot에서 exact하게 해석할 수 있을 때만 승격하고, 현재 Catalog를 과거 정의로 대신 사용하지 않는다. 증명할 수 없으면 `stale`로 보존해 사용자 재검토를 요구한다.

### Baseline — `star.baseline`

Baseline은 특정 입력에서 이미 존재한다고 인정한 Finding fingerprint 집합이다. suppression이나 pass가 아니다.

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `baseline_id`, `revision` | 예 | stable ID |
| `scope_kind`, `project_id` | 예 | shared 또는 local |
| `project_revision_id`, `workspace_snapshot_id` | 예 | 생성 기준 |
| `scan_config_fingerprint`, `rule_set_fingerprint` | 예 | 비교 의미 |
| `finding_fingerprints` | 예 | 정렬된 immutable set 또는 ArtifactRef |
| `set_fingerprint` | 예 | fingerprint 집합 hash |
| `created_at`, `reason` | 예 | 근거 |
| `status` | 예 | `active`, `superseded`, `invalid` |

Baseline candidate는 complete ScanRun에서만 만들 수 있으며 자동으로 active가 되지 않는다. 사용자가 finding set·scope·config·Rule fingerprint를 검토하고 명시적으로 생성해야 한다. shared Baseline은 `.star-control/baselines/*.toml`이 정본이고 DB는 projection이다. 새 scan은 Finding을 `baseline`, `new`, `changed`, `not_observed`로 비교할 수 있다. Rule identity, path scope 또는 redaction 의미가 달라지면 자동으로 같은 baseline을 적용하지 않는다.

#### M3 Baseline v2 target

`star.baseline` v2는 Finding 전용 `finding_fingerprints`를 읽기 compatibility로 유지하고, 공통 issue entry set을 정본으로 사용한다.

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `evidence_subject` | 예 | baseline을 만든 ProjectRevision·WorkspaceSnapshot·ValidationPlan/config/Catalog binding |
| `coverage_scope` | 예 | 관찰한 Project·package/workspace·path·Rule/Check family와 completeness |
| `entries` | 예 | 정렬된 BaselineEntry 또는 ArtifactRef |
| `comparison_contract_version` | 예 | existing/new/worsened 계산 의미 |
| `validator_registry_fingerprint` | Diagnostic entry가 있으면 | Rule·fingerprint contract set |

`BaselineEntry`는 `subject_kind=finding|diagnostic`, issue fingerprint, RuleRef, severity·confidence at baseline, stable ownership/scope key와 optional occurrence/count summary를 가진다. source literal·message text·absolute path는 넣지 않는다.

M3 비교 결과는 다음과 같다.

- `new`: compatible active baseline에 없음
- `existing_unchanged`: 같은 fingerprint·severity·scope
- `worsened`: severity 상승, scope 확대 또는 Rule이 선언한 count threshold 악화
- `improved`: severity·scope·count 감소
- `not_observed`: current complete scope에서 관찰되지 않음
- `incompatible`: Rule/fingerprint/comparison/config/scope 의미가 달라 비교 불가
- `unbaselined`: active baseline이 없거나 current scope를 포함하지 않음

Baseline은 pass·suppression·waiver가 아니다. 기본 ratchet은 `new|worsened`를 policy threshold에 따라 차단하고 `existing_unchanged`를 remaining risk와 raw 결과에 계속 표시한다. `not_observed|improved` 때문에 baseline을 자동 수정·삭제하지 않는다.

Baseline v2 candidate는 complete current ScanRun 또는 ValidationResult와 ValidatorRegistry·config fingerprint에서만 만들 수 있고 자동 active 전환은 금지한다. v1→v2 migration은 각 `finding_fingerprint`를 `subject_kind=finding` entry로 lossless 이동한다. RuleRef·severity를 원본 scan/Catalog에서 확인할 수 없으면 baseline을 active 승격하지 않고 `invalid|superseded` 검토 대상으로 둔다.

### Disposition — `star.disposition`

Disposition은 Finding에 대한 triage 결정이다.

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `disposition_id`, `revision` | 예 | decision identity |
| `finding_id`, `finding_fingerprint` | 예 | 대상 |
| `decision` | 예 | `needs_action`, `accepted_risk`, `false_positive`, `deferred`, `duplicate`, `fixed` |
| `reason_code`, `reason` | 예 | 판단 근거 |
| `scope_revision`, `expires_at` | 필요 시 | stale 판정 경계 |
| `duplicate_of_finding_id` | duplicate일 때 | 같은 project 또는 명시적 cross-project reference |
| `decided_at`, `provenance` | 예 | actor 원문 이름 없이 event ref |
| `status` | 예 | `active`, `stale`, `revoked` |

Disposition은 local operational state이며 관찰 사실을 수정하지 않는다. 공유 판단이 필요하면 application service가 Suppression 또는 Baseline 선언 PatchSet을 별도로 만들고 일반 source review를 거친다. `fixed`는 후속 complete scan에서 Finding이 관찰되지 않을 때만 resolved projection으로 이어질 수 있다.

#### M3 Disposition v2 target

Diagnostic false positive·accepted risk triage를 숨기지 않고 추적하기 위해 `star.disposition` v2는 `finding_id` 전용 field를 read compatibility로 유지하면서 `subject_kind=finding|diagnostic`, exact subject fingerprint, RuleRef, subject binding constraint와 reviewed evidence refs를 추가한다. Diagnostic 대상 `fixed`는 허용하지 않고 후속 complete run의 `not_observed` evaluation을 사용한다.

Disposition은 분류와 metric input이지 Gate 예외가 아니다. `false_positive|accepted_risk|deferred`여도 raw Finding·Diagnostic과 Gate effect를 바꾸지 않는다. blocking issue를 일시 허용하려면 별도 bounded Suppression ID·reason·expiry·approval이 필요하며 DiagnosticEvaluation이 두 ref를 모두 남긴다.

v1→v2 migration은 Finding ID·fingerprint와 decision을 `subject_kind=finding`으로 lossless 이동한다. source fingerprint·scope를 증명하지 못한 active decision은 `stale`로 보존하고 현재 Diagnostic에 적용하지 않는다. 이 target과 migration은 아직 제품에 구현되지 않았다.

## 변경과 검증

### ChangeRecipe — `star.change-recipe`

ChangeRecipe는 반복 가능한 변경 방법의 shared Catalog 선언이다.

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `recipe_id`, `recipe_version` | 예 | stable Catalog ID와 SemVer |
| `definition_fingerprint` | 예 | 실행 의미 hash |
| `finding_selectors` | 예 | 적용 가능한 Rule·Finding 조건 |
| `preconditions` | 예 | language, source, revision, clean/dirty 허용 범위 |
| `parameter_schema_ref` | 예 | typed 입력 |
| `transformer_ref` | 예 | built-in transformer 또는 trusted ToolDescriptor |
| `allowed_path_scope` | 예 | project-relative 범위 |
| `idempotency_contract` | 예 | 재적용 결과와 탐지 방법 |
| `validation_requirements` | 예 | 필요한 Check·Gate |
| `risk_class`, `permission_actions` | 예 | 정책 입력 |
| `rollback_contract` | 예 | reverse patch 또는 restore precondition |

Recipe에는 raw shell, 동적 script text, backend SQL과 AI prompt를 persisted 실행 logic으로 넣지 않는다.

#### 4단계 ChangeRecipe v2 target

4단계는 위 P0 field를 폐기하지 않고 `target_languages`, `rewrite_kind`, `assurance_contract`, typed `target_selector_contract`, 기계적인 `expected_postconditions`, `transformer_input_binding`, `dirty_policy`, resource limit과 `supported_execution_contexts`를 추가한 descriptor `format_version=2`를 요구한다. 같은 ID/version에 다른 실행 의미 hash가 있으면 충돌이며 historical v1 Recipe를 자동 apply 대상으로 재해석하지 않는다.

Recipe target은 raw literal이 아니라 `managed_declaration|contract|symbol|path_range|finding_occurrence|generator_input` selector를 사용한다. text·syntax·symbol-aware·codegen 보장 수준, replay idempotence와 exact field는 [4단계 엔진 계약](safe-patch-and-codemod.md#changerecipe-m4-target)이 소유한다.

### ChangePlan — `star.change-plan`

아래 표는 P0 첫 수직 Slice에서 구현한 Finding·Recipe 중심 v1 shape다.

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `change_plan_id`, `revision` | 예 | local operational document |
| `project_id`, `target_workspace_snapshot_id` | 예 | stale 방지 |
| `finding_refs` | 예 | FindingId와 fingerprint |
| `recipe_refs` | 예 | typed `{recipe_id, recipe_version, definition_fingerprint}` 배열; 문자열 축약 금지 |
| `parameters` | 예 | typed·redacted 값 |
| `expected_paths` | 예 | ProjectPathRef set |
| `preconditions` | 예 | source·config·rule·approval fingerprint |
| `risk`, `permission_plan_ref` | 예 | 실행 경계 |
| `validation_plan_ref` | 예 | patch 뒤 검사 |
| `status` | 예 | `draft`, `ready`, `applied`, `validated`, `blocked`, `abandoned` |
| `created_at`, `updated_at` | 예 | 표시·audit |

ChangePlan은 source를 수정하지 않는다. workspace snapshot이나 recipe fingerprint가 달라지면 `ready` plan은 stale이 되어 재계획해야 한다.

#### 2단계 ChangePlan v2 target

2단계 **변경 계획·영향 분석·affected 선택**은 사용자가 직접 지정한 변경도 표현해야 하므로 `star.change-plan` v2를 요구한다. 이 절은 **목표 계약이며 아직 Schema·DB migration·제품 code에 구현되지 않았다**. v1 row를 자동으로 일반 사용자 계획으로 재해석하지 않는다.

v2는 v1 lifecycle을 유지하면서 다음 필드를 추가·변경한다.

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `task_spec_ref` | 예 | 사용자 입력 TaskSpec revision·hash |
| `scope_revision_ref` | 예 | accepted ScopeRevision. proposed면 `ready` 금지 |
| `impact_analysis_ref` | 예 | 계산 fingerprint가 있는 ImpactAnalysis |
| `change_origin` | 예 | `user_planned`, `finding_recipe`, `mixed` |
| `project_id`, `target_checkout_id` | 예 | 이 ChangePlan 하나가 수정 의도를 소유할 Project·working copy |
| `target_project_revision_id`, `target_workspace_snapshot_id`, `change_set_ref` | 예 | project-scoped stale 방지와 actual dirty 근거 |
| `related_project_impacts` | 예 | read-only provider/consumer ImpactAnalysis ref. 다른 Project의 change unit은 넣지 않음 |
| `planned_change_units` | 예 | stable unit ID, target selector, intended operation·postcondition, reason, provenance와 unit precondition |
| `change_graph` | 예 | unit 사이 requires·must_precede·same_atomic_group edge와 deterministic topological order |
| `expected_impact_refs` | 예 | unit별 accepted direct/transitive ImpactEdge와 unresolved frontier ref |
| `completion_criteria_mapping` | 예 | TaskSpec success criterion을 unit·CheckPlan·manual observation에 연결 |
| `expected_paths` | 예 | v1과 같은 ProjectPathRef set. path 미확정 unit은 별도 selector로 보존 |
| `finding_refs` | 조건부 | `finding_recipe` 또는 `mixed`일 때 FindingId와 fingerprint. user plan이면 빈 목록 허용 |
| `recipe_refs` | 조건부 | deterministic recipe를 쓸 unit에만 typed ref. user plan이면 빈 목록 허용 |
| `parameters` | 조건부 | recipe가 있을 때만 typed·redacted 값 |
| `risk_path_refs` | 예 | risk ID/version, ImpactEdge path와 severity floor |
| `preconditions` | 예 | source·workspace·ChangeSet·Catalog·config·scope·approval fingerprint |
| `unresolved_impacts` | 예 | possible frontier, missing descriptor·downstream과 limitation |
| `permission_requirements` | 예 | 이후 source effect가 요구할 action. 현재 승인을 얻었다는 뜻이 아님 |
| `permission_plan_ref` | source effect Stage 연결 시 | M2 planning만으로는 생략. 이후 실행 직전 requirements를 확정한 PermissionPlan ref |
| `validation_plan_ref` | 예 | 같은 TaskSpec·ScopeRevision·ImpactAnalysis를 참조하는 plan |
| `readiness` | 예 | `draft`, `ready`, `blocked`, `invalidated` |
| `status` | 예 | 기존 lifecycle에 `superseded` 추가 |

각 `PlannedChangeUnit`은 `unit_id`, Project-local target selector, `change_kind`, intended postcondition, source `user|accepted_scope_revision`, reason, expected path set 또는 unresolved target, unit precondition, permission requirement, risk·impact ref와 completion criterion ref를 가진다. 수정 text, raw patch, shell command와 다른 Project의 change unit은 넣지 않는다.

`change_graph` node는 같은 ChangePlan의 unit만 가리킨다. `requires`와 `must_precede` closure는 acyclic이어야 하고 `same_atomic_group`은 서로 겹치지 않는 group을 만든다. stable key byte-order로 위상 정렬하되 이 순서는 source를 실행했다는 뜻이 아니며 이후 write 단계가 source drift를 발견하면 재계획한다. TaskSpec의 각 intended change·required success criterion은 unit 또는 explicit user-decision omission에 매핑돼야 하고, 모든 unit target은 accepted planned change scope 안에 있어야 한다.

자동 영향 계산은 analysis·validation scope를 넓힐 수 있지만 `planned_change_units`와 planned change scope를 사용자 결정 없이 추가하지 않는다. 실제 PatchSet은 이후 write 단계에서 별도 생성한다.

ChangePlan v2도 project-scoped다. TaskSpec이 여러 Project에 planned change를 지정하면 Project별 ChangePlan을 만들고 ScopeRevision·ImpactAnalysis·ValidationPlan이 그 ref를 묶는다. 2단계는 이 plan들을 적용·병합하지 않는다.

v2 `ready`는 다음을 모두 요구한다.

1. TaskSpec과 ScopeRevision이 current·accepted다.
2. project target의 WorkspaceSnapshot·ChangeSet fingerprint가 current probe와 같다.
3. ImpactAnalysis가 required input을 설명하고 ChangePlan·ValidationPlan output coherence가 맞다.
4. required ValidationPlan이 `ready`이고 unresolved required Check가 없다.
5. source effect에 필요한 permission은 아직 실행 전 확인 대상으로 명시돼 있다.
6. change_graph가 acyclic이고 모든 intended change·required completion criterion이 unit·CheckPlan 또는 explicit user decision으로 설명된다.

ChangePlan v1→v2는 local operational state 의미 변경이므로 [Version·Migration 계약](versioning-and-migrations.md)에 따른 backup·dry-run·verification이 필요하다. v1 row는 `change_origin=finding_recipe`로만 lossless 승격할 수 있고 TaskSpec·ScopeRevision·ImpactAnalysis가 없는 active row는 `blocked`로 두고 사용자가 새 planning input을 만들어야 한다.

### PatchSet — `star.patch-set`

PatchSet은 ChangePlan에서 생성한 immutable 변경 제안이다. 기존 ChangeSet이 관찰된 실제 변경 전체를 뜻하는 것과 구분한다.

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `patch_set_id` | 예 | instance ID |
| `change_plan_ref` | 예 | plan ID와 revision |
| `project_id`, `base_workspace_snapshot_id` | 예 | 적용 precondition |
| `patch_fingerprint` | 예 | file operation manifest와 patch artifact hash |
| `operations` | 예 | add, modify, delete, rename과 전·후 hash 요약 |
| `patch_artifact_refs` | 예 | 큰 unified diff·binary delta |
| `affected_finding_ids` | 예 | 해결 대상으로 한 Finding |
| `expected_result_fingerprint` | 가능 시 | deterministic recipe 결과 |
| `status` | 예 | `proposed`, `applied`, `partially_applied`, `failed`, `reverted` |
| `applied_workspace_snapshot_id` | 적용 시 | 실제 결과 |
| `rollback_artifact_refs` | 필요 시 | 복구 자료 |

PatchSet은 immutable preview다. `patch.prepare`만으로 source를 수정하지 않으며 사용자가 patch fingerprint·대상·permission을 확인한 뒤 `patch.apply`를 명시적으로 호출한다.

dirty workspace에서도 다음 조건을 모두 만족하면 적용할 수 있다.

1. 모든 대상 file의 exact before hash·존재 여부·mode가 PatchSet과 일치한다.
2. 기존 dirty change와 PatchSet의 byte range·rename·delete 대상이 겹치지 않는다.
3. 대상 밖 기존 변경을 보존한 결과 manifest를 만들 수 있다.
4. base snapshot, config·plan revision과 approval scope가 일치한다.

겹침을 발견하면 별도 worktree에서 적용하거나 `blocked`로 종료하며 기존 byte를 덮어쓰지 않는다. apply 뒤 recipe가 요구한 validation은 같은 application workflow가 자동 실행한다. exact reverse operation, 적용 직후 hash와 대상 밖 workspace 불변을 모두 증명할 때만 자동 rollback한다. 그 밖의 부분 적용은 성공으로 만들지 않고 `partially_applied`, 열린 effect와 recovery ArtifactRef를 기록한다.

#### 4단계 PatchSet v2와 실행 기록

M4 `star.patch-set` v2는 적용 상태를 갱신하지 않는 immutable proposal이다. exact RecipeExecution, typed selector binding, preview ChangeSet·reconciled ImpactAnalysis, pre/post ValidationPlan, WorktreeDecision, deterministic operation manifest, expected-after, forward/reverse artifact, idempotence와 permission requirement를 fingerprint에 포함한다. v1의 `status`, `applied_workspace_snapshot_id`, rollback field는 historical reader에서만 유지하며 v2 writer는 runtime 상태를 별도 문서에 둔다.

- `star.recipe-execution` v1은 Recipe·input·selector·base subject, built-in/external transformer identity, ToolDescriptor·executable version/hash, process outcome, preview ChangeSet·artifact와 postcondition 평가를 소유한다.
- `star.patch-application` v1은 pre-apply Gate, target Checkout, operation receipt, actual ChangeSet, post-apply Gate, partial/outcome-unknown과 reverse/discard recovery lifecycle을 소유한다.

두 문서의 full field·state machine과 partial apply 알고리즘은 [4단계 엔진 계약](safe-patch-and-codemod.md#recipeexecution-evidence), evidence 결합은 [검사·완료·증거 계약](validation-and-evidence.md)이 소유한다. PatchSet v1→v2는 migration dry-run·backup·rollback과 old-version fixture 없이는 current automatic apply에 사용하지 않는다.

### ValidationResult — `star.validation-result`

기존 ValidationRun은 CheckPlan 한 항목의 실제 process 시도다. ValidationResult는 하나의 snapshot 또는 PatchSet에 대한 여러 ValidationRun을 정규화한 immutable 결과다.

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `validation_result_id` | 예 | instance ID |
| `subject` | 예 | WorkspaceSnapshotId, ScanRunId 또는 PatchSetId |
| `project_id`, `project_revision_id`, `workspace_snapshot_id` | 예 | 검증 source |
| `validation_plan_ref` | 예 | 계획 revision |
| `validation_run_refs` | 예 | pass·fail·not_run을 포함한 전체 시도 |
| `effective_config_fingerprint` | 예 | gate 입력 |
| `outcome` | 예 | `pass`, `fail`, `incomplete`, `error`, `cancelled` |
| `completeness` | 예 | `complete`, `partial`, `unverified` |
| `finding_refs`, `diagnostic_refs` | 예 | 정규화 결과 |
| `artifact_refs` | 예 | log·report·trace |
| `result_fingerprint` | 예 | subject·plan·run 결과 hash |
| `started_at`, `finished_at` | 예 | 실행 구간 |

ValidationResult가 pass여도 source revision이나 config fingerprint가 달라진 새 workspace에 재사용할 수 없다.

M3 target은 위 v1 field를 유지하면서 [EvidenceSubjectBinding](validation-and-evidence.md#evidence-subject-binding), `freshness`, `stability`, 전체 attempt summary와 normalizer fingerprint를 추가한다. exact shape와 `clean_pass|ratchet_satisfied` 분리는 [검사·완료·증거 계약](validation-and-evidence.md#validationresult-계약)이 소유한다. Baseline·Suppression은 이 record의 raw outcome을 변경하지 않는다.

### GateDecision과 ArtifactRef 연결

관리 계층은 동명의 병렬 DTO를 만들지 않는다. 공개 정본은
`star-contracts::evidence::GateDecision`(`star.gate-decision`)과
`star-contracts::evidence::ArtifactRef`(`star.artifact-ref`)이며,
`management`·`star-validation`·`star-state`·`star-evidence`는 이 타입을 직접
소비한다.

GateDecision은 다음을 모두 고정해야 한다.

- project·Check별 EvidenceSubjectBinding과 multi-project binding set fingerprint
- 적용한 Baseline, Suppression과 Disposition revision
- required ValidationResult와 그 completeness
- unresolved Finding의 fingerprint, severity, baseline relation과 disposition
- 모든 Diagnostic의 RuleRef·fingerprint, Baseline v2 relation, Suppression state와 gate effect
- CompletionClaim evaluation과 required Check별 RunSatisfaction
- policy·EffectiveConfig fingerprint
- `auto_pass`, `human_review`, `block`의 근거

P0 v1 관리 Slice의 project 전용 입력은 `GateDecision.extensions["star.management"]`에
`subject_kind`, `subject_id`, `project_revision_id`,
`workspace_snapshot_id`, `subject_fingerprint`, `baseline_ids`,
`suppression_ids`, `disposition_ids`, `validation_result_ids`,
`unresolved_finding_ids`, `policy_fingerprint`,
`effective_config_fingerprint`, `reason_codes`로 고정한다. 이 extension은
`scope`, `decision`, `required_run_refs`, `policy_snapshot` 같은 공개 필드를
대체하거나 재해석할 수 없다. 완료 판정은 항상 `GateDecision::authoritative_state()`를
따른다.

M3 GateDecision v2 writer는 subject binding, RunSatisfaction, DiagnosticEvaluation과 claim을 public typed field로 기록하고 `star.management` extension에 새 필수 의미를 넣지 않는다. v1 extension은 historical read compatibility일 뿐 multi-project binding set이나 ratchet 만족을 합성하는 입력이 아니다.

GateDecision은 raw Finding을 삭제하거나 ValidationResult outcome을 바꾸지 않는다. waiver·suppression·accepted risk는 별도 입력으로 남긴다.

DB에는 공통 ArtifactRef와 subject relation만 저장한다. subject relation은 enclosing
management document와 repository index가 소유하며 ArtifactRef에 별도 subject field를
복제하지 않는다. diff, patch, source manifest, stdout·stderr, trace, screenshot과
report byte는 `.ai-runs`에 둔다.

### 관리 entity 상태 전이

| entity | 허용 전이 | 규칙 |
|---|---|---|
| ScanRun | `queued -> running ->` `succeeded`, `incomplete`, `failed` 또는 `cancelled` | terminal ScanRun을 다시 열지 않고 retry는 새 ScanRunId |
| Suppression | `active ->` `expired`, `revoked` 또는 `stale` | selector·reason 변경은 새 revision이며 과거 revision 보존 |
| Baseline | `active ->` `superseded` 또는 `invalid` | set 변경은 새 Baseline revision 또는 새 ID |
| Disposition | `active ->` `stale` 또는 `revoked` | 새 판단은 새 revision이며 Finding 관찰을 수정하지 않음 |
| ChangePlan | v1 `draft -> ready -> applied -> validated`; 각 non-terminal에서 `blocked` 또는 `abandoned`. v2는 readiness `draft -> ready`, input change 시 `invalidated`, 새 revision 뒤 이전 것은 `superseded` | stale precondition은 같은 revision을 다시 ready로 만들지 않고 plan revision 증가; source apply lifecycle은 이후 PatchSet 단계만 전이 |
| PatchSet | `proposed ->` `applied`, `partially_applied` 또는 `failed`; 적용된 상태에서 `reverted` | byte가 바뀐 PatchSet은 새 ID; partial은 성공 아님 |
| ValidationResult | 생성 시 terminal `pass`, `fail`, `incomplete`, `error` 또는 `cancelled` | retry·재검증은 새 ValidationResultId |
| GateDecision | 생성 시 terminal `auto_pass`, `human_review` 또는 `block` | subject·decision input이 달라지면 새 GateDecision |
| CoordinatedOperation | `prepared -> applying -> completed`; non-terminal에서 `blocked` 또는 `outcome_unknown` | receipt가 없는 participant를 완료로 추측하지 않음; 보상은 새 operation |

표에 없는 전이는 event를 쓰기 전에 거부한다. terminal entity를 수정하는 대신 superseding document와 causation reference를 만든다.

### CoordinatedOperation — `star.coordinated-operation`

CoordinatedOperation은 두 store 이상을 바꾸는 application command의 복구 가능한 상태다. 분산 transaction 성공을 뜻하지 않는다.

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `coordinated_operation_id` | 예 | `cop_` instance ID |
| `idempotency_key` | 예 | 1~128자 non-NUL command key; 같은 key·input은 같은 operation을 복구 |
| `command_kind` | 예 | stable application command ID |
| `input_fingerprint` | 예 | redacted canonical command input hash |
| `permission_scope_fingerprint` | 예 | 승인된 host·project·path·action 범위 |
| `expected_version_vector` | 예 | prepare 시 global과 participants의 store ID·generation·revision |
| `participants` | 예 | ProjectId 정렬, required flag, payload fingerprint, state, optional receipt |
| `state` | 예 | `prepared`, `applying`, `completed`, `blocked`, `outcome_unknown` |
| `result_fingerprint` | completed일 때 | participant result와 committed version vector hash |
| `committed_version_vector` | completed일 때 | 실제 commit된 각 store revision |
| `diagnostic_refs`, `artifact_refs` | 예 | 실패·복구 근거; 큰 byte는 ArtifactRef |
| `created_at`, `updated_at` | 예 | 표시·retention, identity 제외 |

participant receipt는 `project_id`, `operation_id`, `payload_fingerprint`, `result_fingerprint`, `committed_store_revision`과 local event reference를 가진다. global store에는 receipt summary만 복제하고 project detail을 넣지 않는다.

## repository interface

### port와 adapter

`star-ports`의 `ManagementRepositorySet`은 backend-neutral aggregate port다. concrete adapter는 `star-state` 안에서만 선택하고 Controller composition root가 주입한다.

- `GlobalManagementRepository`는 project directory, cross-project relation, coordination과 global lifecycle을 소유한다.
- `ProjectManagementRepository`는 정확히 한 ProjectId partition의 상세 상태와 lifecycle을 소유한다.
- `ManagementRepositorySet`은 global handle과 ProjectId로 연 project handle을 제공하지만 cross-store ACID transaction을 노출하지 않는다.

port가 노출할 최소 기능은 다음과 같다.

| 영역 | operation |
|---|---|
| lifecycle | global/project별 `open`, `status`, `plan_migration`, `backup`, `verify_integrity`, `open_read_only`, `rebuild`, `close` |
| transaction | `begin(expected_store_revision, idempotency_key)`, `commit`, `rollback` |
| project/source | Project, ProjectCheckout, ProjectRevision, WorkspaceSnapshot, CanonicalSource read·write |
| project catalog | ProjectCatalogSnapshot generation start, batch stage, finalize, current/explicit snapshot query |
| scan graph | ScanRun·CodeIndexSnapshot·ManagedRegistrySnapshot start, partition batch stage, generation finalize, Finding·Occurrence·Symbol·Reference·Registry binding/consumer·graph query |
| decision | Suppression, Baseline, Disposition optimistic update |
| change | ChangePlan, PatchSet, ValidationResult, GateDecision commit |
| event | EventEnvelope append와 projection update |
| query | stable cursor pagination, exact fingerprint lookup, at-store-revision snapshot read |
| retention | candidate plan, hold 확인, apply 결과 |
| coordination | `prepare`, participant receipt 조회·commit, `complete`, `mark_blocked`, startup recovery |

public port에는 SQL transaction, row, table, pragma, journal mode, connection pool와 backend error type을 넣지 않는다.

### transaction 불변식

1. event append, 현재 projection, idempotency record와 store revision 증가는 **같은 logical store 안에서** 한 repository transaction이다.
2. 모든 project-scoped record는 ProjectId partition key를 가진다. 다른 project ID의 foreign reference는 허용된 cross-project reference type만 사용한다.
3. query는 committed generation만 반환한다. scan staging row는 일반 query에 보이지 않는다.
4. optimistic conflict 시 아무 mutation도 commit하지 않고 현재 revision을 반환한다.
5. cursor pagination은 `(sort_key, stable_id, store_revision)`을 서명한 opaque cursor를 사용한다. offset pagination에 의존하지 않는다.
6. read snapshot은 `latest_committed` 또는 명시한 `store_revision`이다. 한 page 도중 store revision이 바뀌면 새 cursor를 요구한다.
7. artifact는 temp write, redaction, size·hash 검증과 atomic finalize 뒤 DB transaction에서 참조한다.
8. DB commit에 실패한 finalized artifact는 orphan으로 표시해 retention이 격리한다. DB가 아직 참조하지 않은 byte를 성공 증거로 보고하지 않는다.

### cross-store coordination 불변식

1. global store에 `CoordinatedOperation`을 `prepared`로 commit하기 전에는 participant store를 변경하지 않는다.
2. operation은 ID, command kind, canonical input fingerprint, expected `StoreVersionVector`, 정렬된 participant ProjectId와 permission scope fingerprint를 가진다.
3. 각 project transaction은 domain mutation, local event·idempotency와 `(operation_id, participant_payload_fingerprint, result_fingerprint, committed_store_revision)` receipt를 함께 commit한다.
4. 같은 operation과 같은 payload의 재전송은 기존 receipt를 반환하고, payload가 다르면 `idempotency_conflict`다.
5. 모든 필수 receipt와 postcondition이 확인된 뒤에만 global operation을 `completed`로 바꾼다.
6. crash recovery는 global `prepared`·`applying` operation을 열거하고 receipt를 대조한다. 안전한 missing participant만 재시도한다.
7. precondition 변화, receipt 불일치 또는 결과 판정 불가이면 `blocked` 또는 `outcome_unknown`이며 auto-pass하지 않는다.
8. 이미 commit된 participant를 숨은 rollback으로 되돌리지 않는다. 보상은 새 ID·approval을 가진 explicit compensating operation이다.

`StoreVersionVector`는 `global: {store_id, generation, revision}`과 정렬된 `projects: [{project_id, store_id, generation, revision}]`다. 포함되지 않은 project revision을 `0`으로 추측하지 않는다.

### repository error

adapter 오류는 다음 stable category로 정규화한다.

| category | 의미 |
|---|---|
| `unavailable` | store를 열거나 I/O할 수 없음 |
| `busy` | writer lease 또는 bounded retry를 넘긴 contention |
| `revision_conflict` | expected revision 불일치 |
| `idempotency_conflict` | 같은 key, 다른 payload |
| `migration_required` | 지원하는 과거 version |
| `incompatible_version` | 미래 또는 지원 밖 version |
| `integrity_failed` | 구조·reference·fingerprint 손상 |
| `read_only` | recovery mode에서 mutation 요청 |
| `quota_exceeded` | 설정된 resource limit 초과 |
| `corrupt` | 안전한 query도 보장할 수 없음 |

backend 고유 code는 redaction한 cause metadata로만 보존하며 CLI가 문자열을 parse하지 않는다.

## path와 redaction

### persisted path

- DB와 `.ai-runs` contract에는 ProjectId와 slash 기반 project-relative path만 저장한다.
- project root의 raw 절대 경로는 DB, event, log, artifact manifest와 report에 저장하지 않는다.
- DB는 `root_binding_id`만 가진다. current-user protected `ProjectRootBindingStore`가 opaque binding을 해석하고 raw path는 Controller process memory에서만 사용한다.
- `ProjectRootBindingStore`가 재시작 뒤 연결을 유지해야 하면 OS current-user protection으로 암호화한 opaque locator만 저장할 수 있다. plaintext·검색 가능한 path segment·사용자 이름을 metadata에 두지 않고 management backup·export에서 제외한다.
- 다른 project row에 root binding을 복제하지 않는다. cross-project edge는 ProjectId와 상대 identity만 가진다.
- persisted path는 UTF-8, `/` separator, leading·trailing slash와 빈 segment 없음으로 고정한다. `.`·`..`, drive prefix, UNC, device path, ADS, NUL과 root escape를 거부한다.
- filename Unicode code point와 표시 case는 source가 제공한 spelling을 보존하며 임의 NFC·case 변환을 하지 않는다. Windows adapter가 final path·file identity로 alias와 case-insensitive collision을 탐지하고, 두 ProjectPathRef가 같은 file을 가리키면 하나를 추측 선택하지 않고 `MANAGEMENT_IDENTITY_CONFLICT`로 거부한다.

Windows v1 root-binding adapter는 다음을 지킨다.

1. filename은 `<root_binding_id>.binding`이며 project 이름이나 path segment를 쓰지 않는다.
2. envelope에는 `schema_version`, `root_binding_id`, `project_id`, `protection_kind=windows_current_user`, ciphertext, created_at만 둔다. plaintext path와 그 hash는 envelope·DB·log에 없다.
3. locator plaintext는 absolute final path와 locator format version만 포함하고 Windows current-user data protection으로 암호화한다. UI prompt를 금지하고 product ID와 binding ID를 additional entropy에 묶는다.
4. root-binding directory와 atomic temp/final file은 current user와 SYSTEM만 허용하는 DACL을 적용한다. inherited broad ACE가 남으면 attach를 실패시킨다.
5. decrypt 뒤 final fixed-local directory, reparse point 정책과 Project manifest identity를 다시 확인한다. decrypt 실패·다른 사용자 context·이동된 root는 `detached`이며 사용자가 reattach해야 한다.
6. backup, diagnostics, export와 crash dump용 구조체에는 ciphertext도 복제하지 않는다. binding 삭제는 project detach와 별도 permission을 요구한다.

위 envelope는 P0 v1 형식이다. 1단계 v2는 같은 보호·redaction 규칙을 유지하면서 `checkout_id`를 attachment owner로 추가하고, Project row에 binding을 다시 쓰지 않는다. v1→v2 candidate·rollback 순서는 [Version과 Migration 계약](versioning-and-migrations.md#1단계-project-v1v2-checkout-migration-target)을 따른다.

v1 management DB 전체 암호화는 요구하지 않는다. DB, journal·temporary auxiliary file, backup과 recovery copy는 같은 current-user DACL과 redaction을 적용한다. 이 선택은 다른 일반 사용자에 대한 local protection이며 관리자 또는 이미 침해된 current-user process에 대한 기밀성을 보장하지 않는다.

### 저장 금지 값

다음 값은 DB payload와 fingerprint input에 넣지 않는다.

- secret, token, password, credential, private key와 인증 header
- Windows account name, home directory 이름과 개인 email
- raw 개인 절대 경로, credential이 포함된 remote URL과 query string
- source의 민감 literal, 전체 source line, prompt·대화 원문
- 환경 변수 값과 process 전체 environment
- redaction 전 tool stdout·stderr

Rule과 adapter는 자유 문자열 대신 `message_code`와 typed `message_parameters`를 반환한다. parameter마다 `path`, `identifier`, `count`, `enum`, `safe_text`, `sensitive` 종류를 선언한다. `sensitive` 값은 저장 전에 제거하며 hash도 남기지 않는다.

가림 결과는 `not_needed`, `redacted`, `quarantined`, `rejected` 중 하나다. `quarantined`는 원문을 DB나 `.ai-runs`에 보존한다는 뜻이 아니라 정상 query·report로 승격하지 못한 입력이 있었다는 상태다. secret·사용자 이름·raw 절대 경로·민감 literal byte와 그 hash는 persistence 전에 폐기한다. mandatory identity가 금지 값에 의존하면 위치·Rule 기반의 덜 정밀한 identity를 사용하거나 Occurrence를 quarantined 처리하고 scan completeness를 낮춘다.

## store version과 lifecycle

### ManagementStoreStatus — `star.management-store-status`

logical management store는 backend와 독립적인 metadata를 가진다.

| 필드 | 의미 |
|---|---|
| `store_id` | 한 store generation의 stable ID |
| `management_store_version` | logical model·invariant version |
| `min_reader_version`, `writer_version` | compatibility 범위 |
| `store_scope` | `global` 또는 정확히 한 `project_id` |
| `store_revision` | 해당 logical store commit마다 증가하는 revision; 전체 제품 global revision이 아님 |
| `generation` | rebuild·restore로 새 store를 만들 때 증가 |
| `created_by_product_version`, `last_opened_by_product_version` | 진단 |
| `last_clean_shutdown` | unclean open 검사 입력 |
| `integrity_state` | structural·relation 판정인 `healthy`, `suspect`, `corrupt` |
| `open_mode` | `read_write`, `migration_required`, `read_only_recovery`, `quarantined` |
| `last_verified_at`, `last_backup_ref` | lifecycle 근거 |
| `redaction_contract_version` | 저장 허용 의미 |

실제 backend schema version은 adapter private일 수 있지만 `management_store_version` 하나에 대응하는 migration과 invariant fixture가 있어야 한다.

### open과 migration

1. Controller가 exclusive writer lease를 얻는다.
2. 최소 header와 StoreStatus를 bounded read로 확인한다.
3. 미래 version이면 write를 열지 않고 read-only recovery만 제안한다.
4. unclean shutdown, backend 오류 또는 hash mismatch가 있으면 integrity check를 먼저 수행한다.
5. 지원하는 과거 version이면 migration plan과 필요한 공간을 계산한다.
6. source-derived index만 바꾸는 non-destructive migration은 설정이 허용하면 backup 뒤 자동 적용할 수 있다.
7. local decision 의미, redaction 또는 data loss 가능성이 있는 migration은 명시적 승인 전 실행하지 않는다.
8. transactional migration이 보장되지 않으면 새 store generation에 변환·검증한 뒤 atomic active pointer를 바꾼다.
9. migration 실패 시 기존 active store를 그대로 유지한다.

network, AI 판단과 현재 source file 내용에 migration 의미를 의존시키지 않는다.

### backup

- migration·repair·active pointer 교체 전에는 consistent backup이 필수다.
- 수동 backup과 startup opportunistic backup은 자체 예약 실행이 아니다. 별도 scheduler를 만들지 않는다.
- `BackupPlan`은 source active-set fingerprint, `StoreVersionVector`, destination fingerprint와 각 store의 scope·ID·generation·version·revision을 고정하고 apply는 exact `plan_fingerprint` 승인을 요구한다.
- `BackupSetManifest`는 backup set ID·생성 시각, 각 store의 상대 locator·size·SHA-256과 전체 set fingerprint를 가진다. global과 관련 project가 같은 plan vector에 없으면 consistent backup이라고 부르지 않는다.
- project raw root binding과 secret store는 management backup에 포함하지 않는다.
- destination은 absolute normalized path여야 하고 이미 존재하는 parent 아래 management root 밖에 있어야 하며 plan 시점에는 존재하지 않아야 한다. 열린 SQLite WAL 파일을 복사하지 않고 online backup API로 store별 정지점을 만든 뒤 manifest를 마지막에 쓴다.
- manifest를 공개하기 전에 각 store를 read-only로 열어 backend integrity, event chain, project relation, version·revision과 byte hash를 검증한다. 하나라도 실패하면 restore 후보가 아니다.
- apply가 manifest 작성 뒤 중단되거나 응답 전달 전에 중단돼도 같은 plan은 검증된 set과 durable result receipt를 대조해 같은 typed result로 수렴한다.
- 최소한 latest known-good와 각 pre-migration backup을 hold한다. 삭제는 retention plan과 permission을 거친다.
- backend가 online consistent backup을 보장하지 않으면 writer를 quiesce하고 shadow copy를 만든다.

### 손상과 읽기 전용 복구

integrity check는 다음 순서로 수행한다.

1. backend 자체 structural check
2. StoreStatus와 version header
3. required relation, uniqueness와 ProjectId partition
4. event sequence·hash와 projection store revision
5. derived ID와 fingerprint 재계산 sample 또는 전체 검사
6. ArtifactRef path, size와 hash 존재 검사

실패하면 read-write handle을 닫고 상태를 `suspect` 또는 `corrupt`로 둔다. Controller recovery component만 read-only handle을 열 수 있다.

복구 순서는 다음과 같다.

1. 손상 store를 덮어쓰지 않고 immutable recovery copy와 Diagnostic을 만든다.
2. latest verified backup을 별도 generation으로 restore하고 integrity를 확인한다.
3. backup이 없거나 불완전하면 Git 정본, attached project와 `.ai-runs` manifest에서 새 store를 rebuild한다.
4. 새 generation이 검증된 뒤에만 active pointer를 교체한다.
5. 원래 store와 backup은 retention 승인 전 삭제하지 않는다.

recovery-only Controller는 인증 IPC와 single-writer lease를 유지하면서 `status`, `restore.plan/apply`, `rebuild.plan/apply`, `local-state.export.plan/apply`만 허용한다. 일반 project·scan·decision·patch·validation·migration·backup과 local-state import는 차단한다. import는 검증된 generation을 활성화하고 normal mode로 다시 연 뒤 current ProjectId·source revision·config fingerprint·store revision을 재확인해 수행한다.

### 재구축 가능 범위

| 자료 | source·artifact만으로 재구축 | 재구축 방법 | 잃을 수 있는 것 |
|---|---:|---|---|
| Project shared identity | 예 | project 선언 재탐색 | local root attachment와 local-only Project directory |
| ProjectRevision·WorkspaceSnapshot | 예 | Git·filesystem 재관찰 | 과거 dirty byte가 artifact에 없으면 과거 snapshot |
| Rule·ChangeRecipe snapshot | 예 | Git·Catalog reload | 당시 제거된 Catalog byte가 없으면 과거 해석 |
| ManagedDeclaration·ManagedRegistrySnapshot | 현재 source는 예 | Git manifest reload와 current source rescan | 과거 snapshot 시각·사라진 binding 관찰 |
| Symbol·Reference·Finding·Occurrence | 현재 source는 예 | 같은 config·Rule로 rescan | 과거 scan 시각·instance ID와 사라진 source occurrence |
| shared Suppression·Baseline | 예 | Git 선언 import | local-only revision |
| local Suppression·Disposition | 아니요 | backup·export 필요 | actor·reason·decision 전체 |
| ChangePlan·PatchSet | ChangePlan은 export가 있으면 일부 | local-state import와 verified ArtifactRef reindex | PatchSet 의미·진행 상태·idempotency history |
| ValidationResult·GateDecision | 현재 P-0054에서는 아니요 | 검증된 byte의 ArtifactRef만 reindex | semantic document·DB-only query/event history |
| 큰 evidence | DB와 무관 | `.ai-runs` hash 검증 | artifact 자체가 삭제됐으면 복구 불가 |

rebuild는 새 ScanRunId와 event 시각을 만들며 과거 실행을 한 것처럼 재현하지 않는다. reconstructed record에는 원본 event가 아니라 `reconstructed_from` provenance와 completeness를 둔다.

`management.rebuild.plan`은 protected root-binding store를 열거하되 raw root를 반환하지 않는다. 각 input은 ProjectId·CheckoutId·RootBindingId·current source revision·effective config fingerprint와 `.ai-runs` verified/rejected inventory fingerprint·count를 고정한다. apply는 exact fingerprint 승인 뒤 각 opaque binding을 Controller 내부에서 resolve해 새 candidate generation에서 `project.register -> scan.run`을 같은 application service로 수행한다. 검증된 `.artifact-ref.json` sidecar만 reindex하고 hash·partition·redaction 검증에 실패한 ref는 구조화된 `ArtifactReference/Lost` 항목으로 보고한다.

global DB가 단순히 없으면 새 empty generation을 열 수 있다. corrupt·future-version·active-set mismatch이면 normal read-write service를 만들지 않고 recovery application만 연다. recovery component는 손상 generation을 그대로 보존하면서 side-by-side candidate를 준비하고, 사용자가 exact restore/rebuild plan을 승인한 뒤 전체 candidate set을 검증해 top-level `active-set.json`을 flush·atomic replace한다. activation 전 crash는 이전 set, activation 뒤 crash는 새 set만 열며 자동으로 최신 directory를 추측하지 않는다. CLI·MCP가 DB 파일을 직접 열거나 바꾸는 fallback은 없다.

### local-only state export/import

`LocalStateBundle` v1이 포함하는 값은 ProjectId·source revision·effective config fingerprint·redaction contract version, local Suppression·Baseline·Disposition과 `draft|ready|blocked` active ChangePlan이다. source byte, shared 선언, root locator·절대 경로, secret·사용자 이름·전체 source line, 과거 actor·event timestamp·idempotency history와 이미 terminal인 plan은 의도적으로 제외한다. shared 값은 Git에서 다시 투영하고 제외된 역사 자료는 current decision 복구에 필요하지 않거나 안전한 portable payload가 아니기 때문이다.

export/import는 모두 versioned JSON, payload SHA-256, plan fingerprint와 typed result를 사용한다. export destination과 import source는 absolute normalized JSON path이며 plan/apply 사이 ProjectId·source revision·config fingerprint·schema version·store revision·payload가 달라지면 적용하지 않는다. 동일 ID가 이미 있거나 bundle scope가 다르면 `LocalStateConflict`를 반환하고 자동 merge·overwrite하지 않는다. exact apply 재시도는 private receipt와 현재 imported entity set을 대조해 같은 결과로 수렴한다.

## retention

retention은 source, 공유 declaration과 `.ai-runs` byte를 DB row 삭제와 함께 자동 삭제하지 않는다.

| class | 보존 설정·불변식 | 제거 조건 |
|---|---|---|
| active project·latest snapshot | 계속 | project detach 뒤에도 hold 확인 |
| latest successful scan generation | `management.keep_latest_successful_scans` | 더 최신 complete generation과 reference 없음 |
| incomplete staging generation | `management.incomplete_staging_retention_days` | 실행 중 아님, recovery reference 없음 |
| old Symbol·Reference projection | successful generation 최소 보존 수와 함께 유지 | baseline·plan·validation reference 없음 |
| resolved Finding summary | `management.resolved_finding_retention_days` | active disposition·suppression·baseline·evidence reference 없음 |
| old Occurrence·ScanRun detail | `management.scan_detail_retention_days` | hold·open finding·change·gate reference 없음 |
| local decision | `management.local_decision_retention_days` | audit/export/hold와 active reference 없음 |
| migration backup | latest known-good + `management.migration_backup_min_count` | 검증된 newer backup과 승인 |
| event·idempotency | 관련 state retention 이상 | open effect·audit·evidence reference 없음 |

정확한 기본값과 merge 전략은 [설정과 Catalog 계약](config-and-catalog.md)이 소유한다. cleanup은 startup 또는 수동 command에서만 실행한다. 기본값은 complete scan 2세대, incomplete staging 7일, scan detail 90일, resolved finding·local decision 180일, latest known-good와 pre-migration backup 최소 2개다.

retention 적용 전 deterministic candidate list, 예상 byte·row 수, protected reason과 plan fingerprint를 만든다. apply는 같은 store revision과 plan fingerprint에서만 실행하고 `retention.applied` event를 기록한다.

## 구현 소유권

| 책임 | 소유 Package |
|---|---|
| 이 문서의 직렬화 type·ID·fingerprint payload | `star-contracts` |
| ID·state·cross-field invariant | `star-domain` |
| global/project repository set·coordination·artifact·root binding port | `star-ports` |
| project·checkout discovery, revision·snapshot·source·index tier·symbol/dependency graph 생성 | `star-project` |
| Rule resolution·Finding·Suppression·Baseline·Disposition·gate | `star-validation` |
| ChangePlan 조정과 command/query | `star-application` |
| Recipe prepare·Patch preview·idempotence·PatchApplication/recovery lifecycle | `star-execution` |
| global/project DB adapter, transaction, coordination receipt, migration, backup, recovery, retention | `star-state` |
| diff·report·redaction과 `.ai-runs` | `star-evidence` |
| concrete filesystem·Git·root binding | Windows·Git adapter |
| 단일 composition root와 Writer lease | `star-controller` |

새 Package를 만들지 않는다. backend dependency는 `star-state`의 private adapter에만 들어가며 `star-contracts`, domain, application, CLI와 MCP로 전파하지 않는다.

## 구현 수용 기준

0단계 구현은 다음 fixture와 검사를 모두 가져야 한다.

1. 각 새 contract의 minimal/full valid, invalid, future-version fixture
2. fingerprint golden vector와 field 포함·제외 회귀
3. Windows path 정규화, root escape와 cross-project reference 거부
4. secret·사용자 이름·절대 경로·민감 literal 저장 거부와 redaction fixture
5. 같은 scan batch 재전송의 idempotency와 다른 payload 충돌
6. scan crash 전·중·finalize 뒤 visible generation 불변식
7. Controller 이중 writer 거부와 CLI·MCP direct repository dependency 금지
8. source 삭제 뒤 clean rebuild와 local-only state loss 보고
9. 미래 store version read-only, 과거 version migration, 중단·rollback
10. corrupt store 격리, verified backup restore와 side-by-side rebuild
11. ArtifactRef hash 불일치·orphan·quarantine
12. retention hold·active reference·plan fingerprint 보호
13. CLI-only E2E에서 Codex, App Server, 다른 AI와 network API 호출 0회
14. global/project store 단독 transaction과 cross-store crash point별 coordination recovery
15. 호환되지 않는 global/project backup generation 조합 활성화 거부
16. backend 교체 fake conformance에서 같은 repository contract 결과

backend 선정 전에는 in-memory fake로 contract conformance를 설계할 수 있지만, 이를 실제 persistence 검증으로 보고하지 않는다.
