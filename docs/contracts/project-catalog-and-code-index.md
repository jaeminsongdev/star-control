# 읽기 전용 Project Catalog와 Code Index 계약

## 상태와 문서 소유권

이 문서는 Star-Control 1단계인 **읽기 전용 Project Catalog와 Code Index**의 설계 정본이다. stable `ProjectCheckout`, `ProjectCatalogSnapshot`, `CodeIndexSnapshot`, DB migration·cache·watcher까지 포함한 M1 본체는 **설계 확정, 제품 구현 전**이다. 다만 P-0030에서 아래의 추적 allowlist 기반 운영 precursor와 read-only CLI/MCP action만 구현했다.

이 문서에서 **Project Catalog**는 사용자가 관리하는 project·checkout·workspace와 그 관계의 관찰 snapshot을 뜻한다. [설정과 Catalog 계약](config-and-catalog.md)의 built-in Task·Tool·Rule·Profile descriptor Catalog 및 `CatalogSnapshot`과 다른 domain이다. 이름 충돌을 피하기 위해 wire type은 항상 `ProjectCatalogSnapshot`을 사용한다.

### P-0030 추적 allowlist precursor 경계

- `catalog/projects.toml`은 현재 운영 대상의 Git 추적 정본이다. 정확히 13개 `active_canonical` 프로젝트만 명시하며 `registration_enabled=false`로 고정한다.
- P-0011에서 폐기한 구 관제 저장소·로컬 AI 실험, 하나_프로젝트의 nested Git, `임시문서`, `legacy/`·`래거시/`, backup·sandbox·bootstrap checkout, linked worktree와 Git 정본이 아닌 LAWOS는 명시적 제외 대상이다. 제외 대상은 인접 경로 탐색으로 다시 등록하지 않는다.
- `star.core.project.list`, `star.core.project.status`, `star.core.doctor`는 이 파일에 선언된 정확한 경로만 읽는다. discovery root 재귀 탐색, 인접 checkout 자동 등록, 관리 DB write는 수행하지 않는다.
- probe는 root 존재, Git top-level, checkout kind, origin, `git-common-dir`를 독립 상태로 보고한다. role schema는 `active_canonical`, `linked_worktree`, `read_only_migration_source`, `backup`, `sandbox`, `bootstrap_checkout`을 구분할 수 있지만 현재 운영 allowlist에는 `active_canonical`만 존재하며 unavailable·identity mismatch를 성공으로 숨기지 않는다.
- 출력 `star.project-catalog-view`와 `star.project-status-view`는 P-0030 운영 view이며 M1의 persisted `ProjectCatalogSnapshot`이 아니다. 이후 DB projection을 추가하더라도 Git manifest가 정본이고 DB는 재생성 가능한 파생 상태다.
- `project register`는 이 precursor를 DB에 쓰지 않으며 후속 Slice에서 allowlist membership, idempotency와 13개 활성 목록을 다시 검증한 뒤에만 연다.

책임은 다음처럼 나눈다.

| 책임 | 정본 |
|---|---|
| Project·ProjectRevision·WorkspaceSnapshot·CanonicalSource·ScanRun·Symbol·SymbolReference·Finding의 공통 identity와 DB 경계 | [공통 개발 관리와 로컬 관리 DB 계약](development-management.md) |
| 여러 root·checkout·workspace 발견, source 분류, index tier·graph·freshness·fallback과 read-only CLI query | 이 문서 |
| discovery·scan·ignore·index cache 설정 key와 merge | [설정과 Catalog 계약](config-and-catalog.md) |
| ContextPack과 Goal의 ProjectRef 소비 방식 | [단계 분해와 실행 계약](goal-and-stage.md) |
| TaskSpec·ScopeRevision을 결합한 영향 전파·risk·affected 선택 | [변경 계획·영향 분석 계약](change-planning-and-impact.md) |
| managed declaration·candidate·local implementation constant 분류, Git manifest·binding·consumer·lifecycle | [관리형 Symbol Registry 계약](managed-symbol-registry.md) |
| ScanRun·Finding·freshness가 완료·증거 판단에 미치는 영향 | [검사·완료·증거](validation-and-evidence.md) |
| project store·cache·`.ai-runs` 물리 경계 | [상태 기록과 이어하기](../architecture/state-and-artifacts.md) |
| Package·adapter 소유권과 금지 의존 | [Repository·Package 구조](../architecture/repository-layout.md) |

## 목표와 제외 범위

### 목표

1. 여러 discovery root에서 Git·non-Git Project, nested repository, linked worktree와 build workspace를 중복 없이 발견한다.
2. 경로나 branch 이름과 분리된 ProjectId와 checkout identity로 같은 project의 여러 작업 복사본을 구분한다.
3. dirty working tree의 실제 byte를 HEAD·default branch보다 최신 관찰 사실로 사용한다.
4. source를 목적과 생성 주체에 따라 분류하고 file·package·module·symbol·definition·reference를 계층적으로 색인한다.
5. project·package·contract·dependency graph와 config key·Schema ID·error code·전역 상수·public surface 후보를 제공한다.
6. 하드코딩을 확정 defect가 아니라 근거·confidence·limitation이 있는 Finding 후보로 만든다.
7. 최초 manual full scan과 Git revision·file hash 기반 incremental scan을 같은 snapshot·generation 계약으로 연결한다.
8. text·syntax·semantic index의 정확도와 fallback을 숨기지 않고 query 결과마다 표현한다.
9. 2단계 영향 분석이 fresh snapshot과 graph를 재해석 없이 입력으로 사용할 수 있게 한다.
10. 5단계가 Git manifest를 정본으로 삼아 declaration binding·consumer를 관찰하고 scanner 후보와 local constant를 구분할 수 있게 한다.

### 제외 범위

- project source, 설정, manifest, 문서와 Git index·branch를 수정하지 않는다.
- PatchSet·codemod·rename·format·자동 수정·baseline 생성·suppression 승격을 수행하지 않는다.
- project가 선언한 build, test, package script와 raw command를 실행하지 않는다.
- AI 호출, embedding, LLM 의미 추론과 OpenAI API를 요구하지 않는다.
- 자체 scheduler, cron service와 background periodic scan을 만들지 않는다.
- file watcher를 M1 정확성의 전제로 두지 않는다. 필요성과 이득이 실제 측정된 뒤 별도 선택 기능으로 설계한다.
- remote fetch, pull, clone, hosting API와 remote default branch 조회를 scan의 일부로 수행하지 않는다.
- 특정 parser, language server, indexer, graph DB 또는 persistence backend를 public 계약에 고정하지 않는다.

이 단계의 `read_only`는 **대상 project source에 effect가 없음**을 뜻한다. Controller는 Project Catalog·Code Index projection과 scan evidence를 local management store와 `.ai-runs`에 기록할 수 있다. 그 쓰기는 0단계의 단일 Writer·transaction·redaction 경계를 그대로 사용한다.

## 0단계 선행조건과 호환성 gap

0단계는 다음 선행조건을 충족한다.

- Git source, local management DB projection·local-only state와 `.ai-runs` evidence가 분리돼 있다.
- ProjectId, ProjectRevisionId, WorkspaceSnapshotId, ScanRunId, CanonicalSourceId, SymbolId, SymbolReferenceId와 FindingId가 정의돼 있다.
- invisible scan generation, atomic visible publish, incomplete 처리와 이전 complete generation 유지가 정의돼 있다.
- raw root path는 DB에 저장하지 않고 `root_binding_id`로 분리한다.
- Controller만 repository Writer이며 CLI는 application service를 통한다.

다만 P0 `star.project` v1의 단일 `root_binding_id`는 같은 Project의 main worktree, linked worktree와 별도 clone을 동시에 표현할 수 없다. 1단계 구현 전에 다음 versioned relation migration을 먼저 수행해야 한다.

1. Project stable identity와 local attachment를 분리한 `ProjectCheckout` v1을 추가한다.
2. `star.project` v2는 `root_binding_id`를 소유하지 않고 `attached_checkout_ids`와 derived `registration_state`를 가진다.
3. 기존 attached Project v1 하나마다 새 CheckoutId와 `ProjectCheckout` 하나를 만드는 lossless migration을 제공한다.
4. P0 `root_binding_id`는 migration input으로만 읽고 v2 write에서 다시 만들지 않는다.
5. `ProjectRef` v2는 `root_binding_id` 대신 `checkout_id`로 실제 작업 복사본을 선택한다.
6. v1과 v2가 동시에 들어오면 동일 attachment임을 검증할 수 있을 때만 읽고, 불일치는 `PROJECT_CHECKOUT_IDENTITY_CONFLICT`로 중단한다.

이 migration은 이 설계 작업에서 구현하지 않는다. Schema version, old-version fixture, backup과 rollback은 1단계 제품 구현의 첫 gate다. exact dry-run·ID allocation·binding 전환·rollback 순서는 [Version과 Migration 계약](versioning-and-migrations.md#1단계-project-v1v2-checkout-migration-target)이 소유한다. 기존 P0 row를 여러 checkout으로 추측 분할하거나 raw path hash를 identity로 사용하지 않는다.

## 핵심 용어와 경계

| 용어 | 의미 | identity 범위 |
|---|---|---|
| Project | 공유하거나 local-only로 관리하는 source ownership 경계 | stable ProjectId |
| ProjectCheckout | 한 Project가 현재 사용자 PC에 붙은 실제 working copy | local CheckoutId |
| repository observation | Git object store·common dir를 나타내는 local observation | protected binding reference, 공유 ID 아님 |
| worktree | Git main 또는 linked working tree | ProjectCheckout의 Git 세부 상태 |
| workspace | build system이 선언한 package/member 집합 | Project 안의 derived graph node |
| discovery root | 사용자가 한 번의 발견 command에 제공한 탐색 시작점 | command input, raw path persist 금지 |
| ProjectCatalogSnapshot | discovery scope에서 확인한 Project·Checkout·workspace 관계의 immutable snapshot | ProjectCatalogSnapshotId |
| CodeIndexSnapshot | 한 ProjectCheckout의 WorkspaceSnapshot을 분석한 index generation summary | CodeIndexSnapshotId |
| source entry | 한 WorkspaceSnapshot에서 관찰한 file·generated unit·virtual source metadata | CanonicalSourceId + content hash |
| index entity | package·module·symbol·contract·config key 등 query node | type-tagged stable entity key |
| index edge | entity 사이의 확인·추정 관계 | source·tier·confidence가 있는 stable edge key |

Project는 repository나 directory와 동의어가 아니다. 하나의 Project는 여러 checkout을 가질 수 있고, 하나의 checkout은 정확히 한 Project에 속한다. build workspace는 Project 경계를 자동으로 바꾸지 않고 package membership을 표현한다. nested repository가 독립 Project로 판정되면 parent Project는 그 내부 source를 다시 소유하지 않는다.

## 새 persisted 계약

### ProjectCheckout — `star.project-checkout`

ProjectCheckout은 raw path를 노출하지 않고 한 local working copy를 가리킨다.

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `checkout_id` | 예 | `cko_` + 26자 ULID, Controller가 최초 attach 때 발급하고 재사용하지 않음 |
| `project_id` | 예 | stable ProjectId |
| `root_binding_id` | attached일 때 | current-user protected opaque binding, backup·export 제외 |
| `repository_kind` | 예 | `git`, `none` |
| `checkout_kind` | 예 | `main_worktree`, `linked_worktree`, `clone`, `filesystem_root` |
| `repository_binding_id` | Git일 때 | common Git repository를 가리키는 local opaque binding |
| `worktree_binding_id` | Git일 때 | per-worktree Git dir를 가리키는 local opaque binding |
| `object_format` | Git일 때 | 실제 Git storage object format |
| `head_state` | Git일 때 | `branch`, `detached`, `unborn`, `unavailable` |
| `head_ref`, `head_commit_id`, `head_tree_id` | 관찰 가능할 때 | local checkout의 현재 기준, branch 이름을 identity로 사용하지 않음 |
| `upstream_ref`, `default_branch_hint` | 아니요 | local metadata일 뿐 source 기준이 아님 |
| `remote_identity` | 아니요 | credential·query 없는 host·owner·repository 표시 identity |
| `attachment_state` | 예 | `attached`, `detached`, `missing`, `identity_conflict`, `unsupported` |
| `last_observed_at` | 예 | identity에는 제외 |
| `limitations` | 예 | missing common dir, shallow, sparse, unreadable 등 |
| `content_fingerprint` | 예 | path를 제외한 현재 checkout observation의 versioned hash |

ProjectCheckout content fingerprint payload는 identity contract version, CheckoutId, ProjectId, repository·checkout kind, opaque repository/worktree/root binding ID, object format, head state·ref·commit·tree, upstream/default-branch/remote metadata, attachment state와 정렬한 stable limitation code·redacted parameter를 포함한다. raw message, raw path·그 hash와 `last_observed_at`은 제외한다. binding ID는 local opaque token일 뿐 다른 PC의 shared identity로 사용하지 않는다.

CheckoutId는 path에서 만들지 않는다. root 이동은 protected binding을 다시 연결하고 같은 checkout임을 충분히 증명할 수 있을 때만 같은 CheckoutId를 유지한다. 삭제 후 재생성한 linked worktree는 common repository가 같더라도 새 CheckoutId가 기본이다. 사용자가 명시적으로 reattach하고 identity evidence가 일치할 때만 기존 ID를 사용할 수 있다.

자동 reattach는 old attachment가 `detached|missing`이고 Windows final directory/file identity continuity, ProjectId declaration, repository/worktree administrative identity가 모두 일치하며 새 root가 다른 CheckoutId에 붙지 않았을 때만 허용한다. file identity continuity가 없으면 remote URL·HEAD·content 유사성으로 같은 checkout을 추측하지 않는다. 사용자가 old CheckoutId를 명시해 수동 reattach하려면 exact ProjectId, old/new checkout observation 비교, conflict 없음과 actor·reason event가 필요하며 그 뒤에도 기존 index는 `stale_catalog`로 두고 current probe·scan을 다시 수행한다. 삭제 후 새로 만든 worktree나 clone을 수동 reattach하지 않은 경우에는 항상 새 CheckoutId다.

Git common dir·worktree Git dir의 raw path와 그 hash는 DB에 넣지 않는다. Windows adapter가 opaque binding을 만들고 Git adapter는 그 process-memory path에서 typed Git command를 실행한다.

### ProjectCatalogSnapshot — `star.project-catalog-snapshot`

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `project_catalog_snapshot_id` | 예 | `pcs_` + full SHA-256 base32 derived ID |
| `discovery_scope_fingerprint` | 예 | root binding set·scope·limit의 redacted hash |
| `discovery_config_fingerprint` | 예 | discovery·ignore·root marker 규칙 |
| `project_refs` | 예 | 정렬된 ProjectId와 Project content fingerprint |
| `checkout_refs` | 예 | 정렬된 CheckoutId와 ProjectCheckout revision·hash |
| `workspace_nodes` | 예 | build workspace key·kind·member reference |
| `project_edges` | 예 | nested, submodule, workspace_member, same_repository 등 |
| `counts` | 예 | root, project, checkout, workspace, excluded, error 수 |
| `completeness` | 예 | `complete`, `partial`, `unverified` |
| `limitations` | 예 | 탐색하지 못한 subtree와 원인 |
| `captured_at` | 예 | identity에는 제외 |
| `content_fingerprint` | 예 | 정렬된 snapshot content hash |

identity payload는 discovery contract version, scope fingerprint, config fingerprint, 정렬된 Project·Checkout observation fingerprint, workspace key, edge와 completeness를 포함한다. raw root path, 표시 이름, timestamp와 cache hit 여부는 제외한다.

### CodeIndexSnapshot — `star.code-index-snapshot`

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `code_index_snapshot_id` | 예 | `cix_` + full SHA-256 base32 derived ID |
| `project_id`, `checkout_id` | 예 | source와 concrete working copy |
| `project_catalog_snapshot_id` | 예 | checkout·workspace 관계를 선택한 discovery 근거 |
| `checkout_observation_fingerprint` | 예 | 해당 catalog 안 target checkout·ownership edge의 의미 hash |
| `project_revision_id`, `workspace_snapshot_id` | 예 | base와 실제 dirty byte 관찰 |
| `scan_run_id`, `generation_id` | 예 | 실행·atomic publish 근거 |
| `analysis_input_fingerprint` | 예 | source·scope·config·adapter·Rule partition 입력 전체 hash |
| `scan_config_fingerprint` | 예 | scope·classification·Rule 입력 |
| `index_config_fingerprint` | 예 | tier·adapter·limit·fallback 입력 |
| `required_tier`, `max_tier` | 예 | execution completeness와 optional quality 상한 |
| `adapter_set_fingerprint` | 예 | 사용 가능한 language/build index adapter와 version |
| `classification_fingerprint` | 예 | source class·facet 결정 전체 hash |
| `partitions` | 예 | inventory, text, syntax, semantic, graph, finding별 status·fingerprint |
| `coverage` | 예 | class·language·tier별 대상/성공/실패/제외 count |
| `counts` | 예 | source, package, module, symbol, definition, reference, graph edge, Finding 수 |
| `freshness` | 예 | 생성 시점과 마지막 probe의 partition별 상태 |
| `limitations` | 예 | parse failure, unsupported language, unresolved reference, limit 초과 |
| `artifact_refs` | 예 | entries manifest, scan report와 선택적 analyzer report |
| `content_fingerprint` | 예 | visible index content의 canonical hash |

`analysis_input_fingerprint`는 identity contract version, ProjectId, CheckoutId, target checkout observation fingerprint, ProjectRevisionId, WorkspaceSnapshotId, scan·index·classification·Rule·adapter fingerprint와 정렬된 partition input fingerprint를 JCS로 hash한다. target checkout observation fingerprint에는 attachment·repository/worktree identity와 source ownership edge만 넣고 upstream·default branch·remote 표시 hint는 제외한다.

`content_fingerprint`는 정렬된 partition output fingerprint, entity·edge stable key와 content fingerprint, source-derived Finding·Occurrence fingerprint, coverage·completeness·limitation code를 JCS로 hash한다. mutable Suppression·Disposition·assessment revision, CodeIndexSnapshotId, ScanRunId, generation ID, timestamp, ArtifactRef path, cache location·hit와 terminal render는 제외해 순환 hash와 실행 환경 drift를 막는다.

CodeIndexSnapshotId payload는 identity contract version, `analysis_input_fingerprint`, 정렬된 partition output fingerprint와 `content_fingerprint`다. 같은 analysis input fingerprint에서 다른 content fingerprint가 나오면 nondeterminism 또는 adapter drift로 간주하고 `INDEX_IDENTITY_CONFLICT`로 이전 current generation을 유지한다. 같은 ProjectCatalogSnapshot의 무관한 다른 Project·Checkout row와 timestamp·ScanRunId·cache hit는 identity에 넣지 않는다.

### 하위 projection record

다음 record는 project store 내부 projection과 typed query view다. 별도 public top-level document를 만들지 않고 CodeIndexSnapshot generation에 속한다.

| record | 핵심 필드 |
|---|---|
| `SourceEntry` | CanonicalSourceId, path, content hash, class, facets, language, encoding, size, ownership, analysis eligibility |
| `WorkspaceNode` | workspace key, kind, marker source, member package keys, detection evidence |
| `PackageNode` | package key, name, version if declared, manifest ref, workspace key, language/build metadata |
| `ModuleNode` | module key, package key, canonical source, qualified name, detection tier |
| `DefinitionNode` | SymbolId 또는 non-symbol entity key, source range, tier, visibility, confidence |
| `IndexEntity` | tagged key와 `project\|checkout\|workspace\|source\|package\|module\|symbol\|contract\|config_key\|schema_id\|error_code\|constant\|public_surface\|external_dependency` kind |
| `IndexEdge` | from, to 또는 unresolved target, relation, evidence source, tier, resolution, confidence |
| `IndexPartition` | kind, required 여부, execution status, freshness, input/output fingerprint, adapter ref, coverage, limitation, reuse provenance |
| `GuidanceRecord` | AGENTS·README·정본 문서 ref, 적용 scope, priority, hash, conflict·limitation |
| `ToolchainRecord` | language, build system, package manager, lockfile, toolchain file와 command declaration evidence |
| `FindingView` | snapshot Finding·Occurrence ref에 current assessment·decision revision을 query 시 join한 view |

entity key는 `project_id`, entity kind, canonical name 또는 source anchor와 해당 identity contract version의 full SHA-256이다. 표시 이름, line, timestamp와 absolute path를 key에 넣지 않는다.

IndexPartition `kind`은 `inventory|classification|text|syntax:<language-id>|semantic:<language-id>|graph|finding`이고 execution status는 `not_planned|queued|running|succeeded|incomplete|failed|cancelled|reused`다. `reused`는 원래 partition snapshot ref와 동일 input/output fingerprint가 있을 때만 허용한다. execution status와 freshness는 다른 축이므로 과거 `succeeded` partition도 current probe 뒤 `stale_*`가 될 수 있다.

## Project와 checkout identity 결정

### ProjectId 우선순위

1. 유효한 `.star-control/project.toml`의 shared ProjectId를 사용한다.
2. manifest가 없고 Git common repository가 이미 local ProjectId에 연결돼 있으면 그 ProjectId를 같은 repository의 새 checkout에 사용한다.
3. 처음 보는 Git repository 또는 explicit non-Git root에는 Controller가 local-only ProjectId를 발급한다.
4. remote URL, directory 이름, default branch, package 이름과 content hash만으로 두 Project를 자동 병합하지 않는다.
5. 같은 physical checkout에서 서로 다른 shared ProjectId가 발견되면 identity conflict이며 scan하지 않는다.

서로 다른 clone이 같은 shared ProjectId를 선언할 수 있다. 이 경우 Project는 같고 CheckoutId가 다르다. manifest가 없는 서로 다른 clone은 remote identity가 같아도 별도 local Project로 시작한다.

한 discovery command 안에서 처음 보는 main·linked worktree가 함께 발견되면 repository binding identity로 먼저 group하고 canonical group key 순으로 local ProjectId 하나를 발급한 뒤 각 worktree에 별도 CheckoutId를 발급한다. filesystem enumeration 순서가 ID grouping을 바꾸지 않는다. common repository는 같지만 별도 clone인 checkout은 같은 group으로 합치지 않는다.

### Git repository와 linked worktree

Git adapter는 `.git` 파일·directory 내부를 직접 해석해 identity를 만들지 않고 Git의 machine-readable command를 사용한다.

- `git rev-parse --path-format=absolute --show-toplevel --git-dir --git-common-dir --show-object-format`
- `git worktree list --porcelain -z`
- `git status --porcelain=v2 -z --branch --untracked-files=all`

raw output은 bounded parser가 즉시 typed observation으로 바꾸고 path는 root-binding adapter로 전달한 뒤 버린다. unknown porcelain field는 무시한 사실을 limitation으로 남기고 known record를 잘못 해석하지 않는다.

main worktree와 linked worktree는 common repository를 공유하지만 HEAD와 index는 worktree별이다. 따라서 ProjectRevision과 WorkspaceSnapshot은 CheckoutId에 연결하고 다른 worktree의 dirty state를 재사용하지 않는다.

`git worktree list`가 explicit discovery root·기존 approved root binding 밖의 worktree path를 반환해도 자동 attach하거나 그 source를 읽지 않는다. path byte를 버리고 `LINKED_WORKTREE_OUT_OF_SCOPE` limitation count만 남긴다. 사용자가 그 worktree를 별도 discovery root로 제공해 root-binding permission을 얻었을 때만 ProjectCheckout을 만든다. 이미 attached된 checkout도 이번 command scope 밖이면 current source probe 대상에 포함하지 않는다.

### nested repository·submodule·workspace

| 관찰 | 처리 |
|---|---|
| parent 안의 독립 Git root | 별도 Project·Checkout으로 등록하고 parent source recursion은 해당 경계에서 중단 |
| Git submodule | 별도 Project 후보 + parent의 `submodule` edge, parent에는 gitlink commit만 기록 |
| linked worktree | 같은 Project, 별도 Checkout, `same_repository` edge |
| build workspace marker | Project 안 WorkspaceNode와 member PackageNode 생성 |
| workspace가 여러 explicit Project를 묶음 | global graph에 `workspace_member` edge만 생성, Project identity 병합 금지 |
| nested non-Git manifest | 가장 가까운 유효 project marker가 source ownership을 가짐 |
| ownership이 겹치거나 marker가 충돌 | file을 양쪽에 복제하지 않고 `ambiguous_ownership` limitation·review |

parent Project의 source manifest에는 nested Project boundary entry와 child ProjectId만 남긴다. child의 source file·symbol·Finding을 parent project store에 복제하지 않는다.

### non-Git Project

explicit discovery root 또는 신뢰된 project marker가 있으면 non-Git Project를 만들 수 있다. ProjectRevision은 sorted ProjectPathRef, file kind, mode와 content SHA-256 manifest로 만든다. mtime·directory order만으로 revision을 확정하지 않는다.

manifest를 완전히 만들지 못하면 `ProjectRevision.completeness=partial|unverified`이고 CodeIndexSnapshot을 current complete로 publish하지 않는다. 이후 Git init 여부가 바뀌어도 같은 shared ProjectId manifest가 있으면 Project identity를 유지하고 새 checkout revision kind로 전환한다. manifest가 없으면 자동 identity merge를 하지 않는다.

## Project discovery 흐름

### 입력

`project.discover`는 한 개 이상의 user-provided discovery root를 받는다. CLI의 absolute path는 IPC 전에 존재·권한만 확인하고 Controller가 즉시 protected root binding으로 바꾼다. command·event·DB·report에는 raw path를 남기지 않는다.

입력에는 다음을 고정한다.

- root binding set
- `full | incremental` discovery mode
- include/exclude relative scope
- nested repository·linked worktree·non-Git detection policy
- symlink policy와 directory·depth limit
- expected discovery config fingerprint

### deterministic 단계

1. root binding을 resolve하고 final filesystem identity로 alias·중복 root를 제거한다.
2. immutable deny 경계(`.git` 내부, `.ai-runs`, root escape, device path)를 적용한다.
3. explicit Project manifest, Git top level, nested repository와 supported workspace marker를 찾는다.
4. Git common repository·worktree를 typed observation으로 만들고 기존 Project·Checkout attachment와 대조한다.
5. non-Git explicit root와 manifest ownership을 판정한다.
6. nested 경계에서 parent recursion을 중단하고 project·checkout·workspace edge를 만든다.
7. limit, access error, ignored subtree와 unsupported marker를 limitation으로 집계한다.
8. invisible global staging generation에 ProjectCatalogSnapshot을 쓰고 identity·partition·reference를 검증한다.
9. complete면 current catalog pointer를 atomic publish한다. partial이면 명시한 snapshot ID로만 조회할 수 있고 이전 complete catalog를 current로 유지한다.

directory enumeration order는 결과 identity에 영향을 주지 않는다. 모든 root, ProjectId, CheckoutId, workspace key와 edge는 canonical key로 정렬한다.

discovery `incremental`은 이전 ProjectCatalogSnapshot의 root·marker·repository observation을 reuse candidate로 삼는 모드다. explicit Git root는 current `rev-parse`·worktree observation을 항상 다시 확인하고, non-Git/nested 탐색은 directory entry manifest와 marker content hash가 일치할 때만 subtree를 재사용한다. watcher event나 directory mtime만으로 unchanged를 확정하지 않는다. 안전한 subtree equality를 증명할 수 없으면 해당 root만 full discovery로 승격하고 `effective_mode`와 이유를 남긴다.

### discovery ignore와 nested 경계

Project discovery와 project source scan의 ignore는 분리한다. discovery가 source ignore를 그대로 쓰면 ignored parent 아래의 명시적 Project root를 놓칠 수 있고, 반대로 모든 ignored subtree를 내려가면 vendor/cache 안의 repository를 무제한 발견할 수 있다.

- explicit discovery root는 ignore와 관계없이 root 후보로 검사한다.
- recursion은 `project_discovery.exclude_paths_add`, depth·directory limit와 symlink policy를 따른다.
- ignored subtree 안의 nested root는 기본적으로 내려가 찾지 않으며 limitation count를 남긴다.
- 사용자는 그 nested root를 별도 explicit root로 제공할 수 있다.
- `.git` administrative contents에는 절대 재귀하지 않는다.
- symlink·junction은 기본 미추적이며 final identity가 root 밖이면 거부한다.

## source inventory와 분류

### primary class와 facet

각 SourceEntry는 primary class 하나와 0개 이상의 facet을 가진다.

| primary class | 의미 |
|---|---|
| `source` | 제품·library의 사람이 편집하는 구현 source |
| `test` | 테스트 실행 source와 assertion |
| `docs` | 설명·guide·reference 문서 |
| `config` | 사람이 편집하는 설정·manifest·workflow |
| `schema` | 기계 계약 Schema·IDL·API 명세 |
| `migration` | version·data·config migration source |
| `generated` | generator가 소유하는 결과 |
| `vendor` | 외부에서 들여온 third-party source |
| `cache` | 다시 만들 수 있는 local cache |
| `output` | build·test·coverage·release 산출물 |
| `unknown` | 근거가 부족해 안전하게 분류하지 못함 |

facet은 `fixture`, `example`, `docs_example`, `benchmark`, `generated_committed`, `generated_ephemeral`, `third_party`, `binary`, `ignored`, `untracked`, `sparse_absent`, `sensitive_candidate`를 포함한다. fixture와 example을 primary `test` 또는 `docs`에 뭉개지 않고 별도 facet으로 유지한다.

### 분류 우선순위

높은 우선순위부터 적용한다.

1. 유효한 project classification override와 source ownership 선언
2. generator manifest·workspace/build metadata의 명시적 output/vendor 관계
3. VCS tracked·ignored·submodule 상태와 canonical manifest 위치
4. built-in language/build adapter의 versioned path·file evidence
5. 확장자·파일명·경로 heuristic
6. `unknown`

서로 다른 높은 우선순위 근거가 충돌하면 임의로 하나를 택하지 않고 `classification_conflict`를 기록한다. conflict entry에는 text inventory만 허용하고 hardcoding warning·semantic edge를 만들지 않는다.

### class별 기본 분석 경계

| class/facet | inventory | text | syntax | semantic | hardcoding |
|---|---:|---:|---:|---:|---:|
| production `source` | 예 | 예 | 지원 시 | 지원 시 | candidate·warning |
| `test` | 예 | 예 | 지원 시 | 지원 시 | 기본 제외, opt-in candidate·gate 제외 |
| `fixture` | 예 | 제한 | opt-in | 아니요 | 기본 제외, security signature만 별도 |
| `docs` | 예 | 예 | 문서 parser가 있으면 | 아니요 | 기본 제외, docs_example opt-in·제품 warning 금지 |
| `config` | 예 | 예 | 형식 parser가 있으면 | key relation만 | config-aware candidate |
| `schema` | 예 | 예 | 지원 시 | contract relation | schema example 분리 |
| `migration` | 예 | 예 | 지원 시 | 지원 시 | candidate·review 우선 |
| `generated` | 예 | 선택 | 기본 제외 | 기본 제외 | 생성 source로 attribution, warning 금지 |
| `vendor` | 예 | 기본 제외 | 제외 | 제외 | 제외 |
| `cache`, `output` | excluded count만 | 제외 | 제외 | 제외 | 제외 |
| `sensitive_candidate` facet | metadata·redacted count만 | 제외 | 제외 | 제외 | 제외·quarantined |
| `unknown` | 예 | 안전한 text이면 | 제외 | 제외 | review 필요 |

generated·vendor·fixture·docs example에서 같은 literal이 발견돼도 production source와 중복 error string count를 합치지 않는다. generated source가 production public surface를 노출할 수 있으면 원래 generator input과 `generated_from` edge를 우선하고 생성 결과 자체에는 낮은 confidence를 표시한다.

credential filename/manifest marker, protected secret-store 경로 또는 project Rule이 `sensitive_candidate`로 선언한 entry는 content read·hash 전에 격리한다. 그 밖의 file을 bounded pre-index sensitivity detector가 process memory에서 읽다가 private-key·credential signature로 판정하면 즉시 byte를 폐기하고 content hash를 만들지 않는다. 두 경우 모두 relative path의 redacted reference, file kind와 존재 count만 남기며 byte, literal과 그 hash를 WorkspaceSnapshot artifact·index·cache에 넣지 않는다. 해당 scope의 text·syntax·semantic·hardcoding coverage는 `excluded_by_security_policy`이며 빈 결과를 `confirmed_empty`로 만들 수 없다. tracked 또는 explicit include scope의 entry라 exact content observation을 끝내지 못한 경우 WorkspaceSnapshot과 ScanRun completeness도 `unverified` 또는 `incomplete`로 낮춘다.

### ignore 적용 순서

1. immutable deny: VCS administrative data, `.ai-runs`, root escape, protected secret store
2. effective `scan.include_paths` scope
3. `scan.exclude_paths_add` union
4. Git tracked state와 Git ignore precedence
5. build adapter가 확인한 cache·output·vendor classification
6. classification override와 class별 analysis eligibility

Git이 이미 tracked한 file은 ignore pattern 때문에 누락하지 않는다. ignored untracked file은 `scan.include_ignored=false`에서 제외하고 count·reason만 남긴다. ignored file을 포함하려면 explicit widening permission과 expected config fingerprint가 필요하다.

## language·toolchain·guidance 발견

### ToolchainRecord

각 발견은 `declared`, `inferred`, `observed` provenance를 가진다.

| 대상 | 우선 근거 |
|---|---|
| language | workspace/package manifest, toolchain file, source extension·shebang |
| build system | explicit task declaration, build manifest, workspace marker |
| package manager | lockfile와 manifest 조합, project task declaration |
| lockfile | exact ProjectPathRef와 content hash |
| toolchain | version file·manifest constraint·known config |
| 주요 command | `.star-control/tasks.toml`, manifest script, 정본 문서의 exact example |

raw command string은 실행 계약이 아니다. 발견한 command는 `executable_hint`, typed args, cwd scope, source ref, declaration kind와 confidence로 저장하며 이 단계에서 실행하지 않는다. README 예시는 `suggested`, package script는 `declared`, 실제 실행 evidence가 없는 도구 version은 `observed`로 표시하지 않는다.

PATH lookup과 tool version process 실행은 첫 read-only Slice의 필수 조건이 아니다. 향후 `--probe-tools`가 필요하면 별도 typed read permission, bounded timeout과 no-install·no-network 정책을 설계한 뒤 추가한다.

### GuidanceRecord와 정본 우선순위

Project별 규칙과 정본 문서는 다음 순서로 발견한다.

1. `.star-control/project.toml`, `config.toml`, `contracts.toml`의 명시적 source ownership·canonical document 선언
2. discovery root에서 target file까지 적용되는 `AGENTS.md` chain
3. `docs/README.md` 같은 명시적 reading-order index와 그 link graph
4. root `README`, contribution·architecture·contract 문서
5. build manifest와 package metadata
6. filename heuristic으로 찾은 후보

각 record는 source hash, 적용 subtree·entity scope, priority 근거, supersedes link, freshness와 limitation을 가진다. nested `AGENTS.md`는 자기 subtree에만 적용하고 상위 안전 제한을 자동 완화하지 않는다. 두 문서가 같은 책임의 정본을 주장하면 하나를 추측 선택하지 않고 `guidance_conflict`로 ContextPack에 전달한다.

DB에는 문서 전체 byte를 복제하지 않고 CanonicalSourceId, content hash, heading/anchor index와 redacted summary만 둔다. 실제 ContextPack은 current WorkspaceSnapshot에서 hash를 다시 검증한 source를 읽는다.

## Code Index tier와 fallback

adapter capability record는 stable `adapter_id`, version, supported language/mode·source class, 제공 tier, 생성 entity/edge kind, deterministic flag, maximum input, limitation/error code set과 implementation fingerprint를 가진다. `adapter_set_fingerprint`는 실제 선택 가능한 capability record를 adapter ID 순으로 정렬해 JCS hash한다. parser library의 AST·error type·library 이름과 DB/backend 정보는 이 public record에 넣지 않는다.

scan plan은 `required_tier`와 `max_tier`를 함께 고정한다. inventory·classification과 eligible text는 항상 required이며, 각 language에서 required tier 이하 partition을 끝내야 ScanRun이 complete다. required보다 높고 max 이하인 partition은 capability가 있을 때 시도하는 optional quality partition이다. optional tier가 unavailable·partial이어도 required partition과 generation integrity가 complete면 ScanRun은 `succeeded`일 수 있지만 coverage와 query는 그 tier를 제공했다고 주장하지 않는다.

### 정확도 tier

| tier | 제공할 수 있는 것 | 제공한다고 주장하지 않는 것 |
|---|---|---|
| `text` | path·token·literal·exact string occurrence, 파일 수준 후보 | definition/reference 의미, scope·type·dynamic dispatch |
| `syntax` | parser가 확인한 declaration, import, call-like node, lexical scope | build resolution, macro/generated target, runtime dispatch |
| `semantic` | compiler·language service·build-aware adapter가 해석한 cross-file/package definition·reference | adapter가 다루지 못한 dynamic·reflection·runtime 관계 |

`semantic -> syntax -> text` fallback은 결과를 얻기 위한 순서이지 낮은 tier를 높은 tier로 표시하는 승격이 아니다. query는 반드시 `requested_tier`, `used_tier`, `coverage`, `resolution`, `confidence`, `limitations`를 반환한다.

### parse·adapter 실패

- syntax parse가 실패해도 SourceEntry와 text index는 가능한 범위에서 유지한다. syntax가 required tier면 ScanRun은 incomplete, optional이면 syntax partition만 incomplete다.
- semantic adapter가 없거나 environment를 만들 수 없으면 `semantic_unavailable`이며 syntax 또는 text로 fallback한다.
- unsupported language는 `unsupported_language` count와 source 목록을 남기고 빈 성공으로 만들지 않는다.
- parser crash·timeout·resource limit은 file별 error와 adapter health를 남기고 ScanRun completeness를 낮춘다.
- 하나의 language partition 실패 때문에 다른 language의 complete partition을 삭제하지 않는다.
- incomplete generation은 snapshot ID로 진단 조회할 수 있지만 current complete pointer를 대체하지 않는다.

### no-result 표현

| 상태 | 의미 |
|---|---|
| `confirmed_empty` | requested scope가 current·complete하고 적용 가능한 tier에서 결과가 없음 |
| `not_indexed` | 해당 partition을 아직 만들지 않음 |
| `unsupported_language` | language adapter가 없음 |
| `parse_failed` | syntax input은 대상이지만 parse 실패 |
| `semantic_unavailable` | semantic 환경·adapter를 사용할 수 없음 |
| `excluded_by_policy` | ignore·class policy로 분석하지 않음 |
| `stale` | source/config/adapter가 index input과 달라 현재 결과가 아님 |
| `partial` | 일부 scope·file·partition만 성공 |
| `ambiguous` | 둘 이상의 target을 해소하지 못함 |

`[]`만 반환하는 query는 허용하지 않는다. 빈 result에는 위 reason, 확인한 scope, 사용 tier와 coverage가 있어야 한다.

## entity와 graph 계약

### entity 종류

- Project, Checkout, Workspace
- Source, Package, Module, Symbol, Definition
- Contract, ConfigKey, SchemaId, ErrorCode, Constant, PublicSurface
- ExternalDependency와 unresolved target

Contract·ConfigKey·SchemaId·ErrorCode·Constant·PublicSurface는 모두 **candidate entity**로 시작한다. 명시적 manifest·Schema·visibility·export evidence가 있으면 `declared`, syntax evidence만 있으면 `inferred`, text match뿐이면 `candidate`다.

이 `declared|inferred|candidate`는 **Index evidence quality**이고 Managed Registry의 ownership 분류와 다른 축이다. Registry-eligible entity는 별도 `registry_classification=managed_declaration|candidate|local_implementation_constant`와 classification evidence를 가진다. `managed_declaration`은 current Git Managed Registry manifest의 exact declaration ID·source hash가 있을 때만 가능하다. scanner·DB row·중복 literal·`declared` visibility만으로 승격하지 않는다. 명시적인 local ownership evidence가 있으면 `local_implementation_constant`로 유지하며 Registry source 변경 target에서 제외한다.

### edge 종류

| relation | 예 |
|---|---|
| `contains`, `member_of` | Project→Workspace→Package→Module→Source |
| `declares`, `defines` | Source→Symbol·SchemaId·ConfigKey |
| `references`, `calls`, `reads`, `writes`, `inherits` | Symbol·Source 사이 relation |
| `imports`, `depends_on` | module·package·project dependency |
| `tests`, `documents` | test/docs entity가 source·contract를 가리킴 |
| `generates`, `generated_from` | generator input과 output |
| `migrates` | migration이 schema/config version을 이동 |
| `implements`, `exposes` | implementation과 contract/public surface |
| `managed_by`, `binds`, `consumes` | entity·definition·consumer와 ManagedDeclaration 관계 |
| `aliases`, `replaces` | deprecated declaration과 bounded compatibility successor |
| `nested_project`, `submodule`, `same_repository`, `workspace_member` | Project Catalog 관계 |

각 edge는 `evidence_source_id`, source range if available, `tier`, `resolution=resolved|ambiguous|unresolved|external`, confidence와 limitation을 가진다. semantic edge와 text-inferred edge를 같은 certainty로 합치지 않는다. 같은 logical edge에 여러 evidence가 있으면 evidence set을 정렬해 content fingerprint를 계산한다.

### global과 project graph 경계

- project store는 source·package·module·symbol·contract·finding detail을 소유한다.
- global store는 ProjectId, exported entity key, cross-project edge와 각 CodeIndexSnapshot ref만 소유한다.
- 다른 project의 source path, root binding, private symbol과 occurrence detail을 global store나 상대 project store에 복제하지 않는다.
- cross-project target이 fresh snapshot으로 확인되지 않으면 unresolved candidate edge로 유지한다.

## 하드코딩 후보

### 탐지 범주와 최소 evidence

| 범주 | 최소 evidence | 주요 false-positive guard |
|---|---|---|
| absolute path | string/token이 Windows drive·UNC·device 또는 POSIX absolute path grammar와 일치 | test fixture·docs example·schema example 분리, root path 원문 저장 금지 |
| URL·IP·port | URL/IP grammar 또는 host+port context와 source location | example·localhost·test server facet, credential/query redaction |
| timeout·retry·limit | numeric literal + 주변 identifier/API role + 단위 후보 | bare number만으로 Finding 금지, constant/config relation 확인 |
| raw command string | process/shell API argument 또는 command field로 이어지는 syntax/reference edge | docs code block·test fixture 분리, typed args declaration은 제외 |
| duplicate error string | 정규화한 message가 서로 다른 production definition에서 반복 | generated/vendor/test/docs 제외, message code·localization relation 확인 |
| config duplicate literal | config key entity와 source literal 사이 exact/normalized value relation | secret·개인 path는 비교·hash 금지, environment-specific test 분리 |

literal 원문이 secret, 사용자 이름, credential, 개인 absolute path 또는 민감 source로 판정되면 저장도 hash도 하지 않는다. Finding에는 `literal_kind`, redacted shape, length bucket, source context, detector tier와 message code만 둔다.

각 hardcoding evidence record는 Rule ID·version·parameter fingerprint, category, CanonicalSourceId·range·content hash, source class·facet·classification provenance, used tier, matched predicate, related symbol/config entity, 적용한 false-positive guard, confidence, redaction status와 limitation을 가진다. line text 전체나 raw command를 evidence row에 복사하지 않는다.

normalization은 Rule version에 고정한다.

- path와 endpoint는 grammar kind·segment/host shape·port presence만 만들고 실제 개인 path, credential과 query는 폐기한다.
- timeout·retry·limit는 parsed numeric type, 명시 단위와 주변 identifier role이 모두 있을 때만 identity token을 만든다.
- raw command는 syntax/reference edge가 process·shell sink까지 이어질 때만 candidate이며 단순 string 이름 일치는 evidence가 아니다.
- duplicate error string은 안전한 non-sensitive literal의 escape를 decode하고 line ending·연속 horizontal whitespace만 정규화한다. case·문장부호·placeholder 순서는 보존한다.
- config duplicate는 config parser가 만든 typed scalar와 source literal의 같은 type·normalized value relation만 사용한다. text로 우연히 같은 값은 확정 relation이 아니다.

정규화에 필요한 parser가 없으면 text candidate 또는 `review` limitation까지만 만들고 warning threshold를 적용하지 않는다.

### assessment state

| state | 의미 | 생성 조건 |
|---|---|---|
| `candidate` | heuristic evidence가 있으나 확정하지 않음 | 기본 상태 |
| `warning` | versioned Rule의 높은 confidence 조건을 만족한 비차단 위험 관찰 | complete source·Rule·evidence 필요 |
| `review` | ambiguity, public surface 영향 또는 낮은 coverage 때문에 사람 판단 필요 | policy projection, Finding 원문은 유지 |
| `allowed` | active Suppression 또는 `accepted_risk\|false_positive\|deferred` Disposition이 있음 | decision ref·revision·reason·expiry 필수 |

`allowed`는 Finding을 삭제하거나 detector 결과를 pass로 바꾸지 않는다. decision이 stale·expired이면 candidate/warning/review로 돌아가고 stale decision ref를 표시한다. 1단계에는 `confirmed_defect` 자동 상태가 없다.

assessment·Suppression·Disposition은 CodeIndexSnapshot content가 아니라 별도 decision projection이다. decision 변경은 FindingView와 event revision만 갱신하고 scan partition·CodeIndexSnapshot을 invalidate하지 않는다. FindingView는 사용한 CodeIndexSnapshotId와 decision revision set을 함께 반환해 과거 후보 evidence와 현재 판단을 섞지 않는다.

severity와 confidence는 assessment와 별도다. warning threshold, review rule과 class별 gate 기본값은 Rule·Policy snapshot에서 오며 analyzer code에 박아 넣지 않는다.

### source context별 오탐 제어

- production source와 migration은 일반 Rule 적용 대상이다.
- test는 기본 제외이며 explicit opt-in이면 candidate를 보여줄 수 있지만 제품 gate에서는 계속 제외한다.
- fixture, generated, vendor, cache, output은 기본 hardcoding Rule 대상이 아니다.
- docs example과 schema example은 explicit opt-in일 때 별도 context로만 집계하며 production duplicate count에 포함하지 않는다.
- classification이 unknown/conflict이면 warning으로 승격하지 않고 review limitation을 만든다.

## full scan과 incremental scan

### 공통 상태 흐름

```text
requested
  -> discovering
  -> snapshotting
  -> classifying
  -> indexing_text
  -> indexing_syntax
  -> indexing_semantic (available partition only)
  -> building_graph
  -> evaluating_rules
  -> finalizing
  -> succeeded | incomplete | failed | cancelled
```

`requested`부터 `finalizing`까지는 1단계가 추가하는 ScanRun `phase`이며 P0 top-level `status=queued|running|...`을 대체하지 않는다. 각 phase 전이는 ScanRun event와 partition status를 남긴다. 여기서 complete generation은 required partition과 reference integrity가 모두 complete한 generation이다. optional tier limitation은 숨기지 않되 그것만으로 ScanRun을 incomplete로 바꾸지 않는다. terminal `failed`, `cancelled`, `incomplete`는 이전 current complete generation을 바꾸지 않는다.

### 최초 manual full scan

1. 사용자가 CLI에서 discovery root와 scan을 수동 시작한다. `--full`을 명시하거나 current complete snapshot이 없으면 plan이 `effective_mode=full`로 승격한다.
2. ProjectCatalogSnapshot을 refresh하고 target ProjectCheckout을 확정한다.
3. ProjectRevision과 dirty WorkspaceSnapshot을 수집한다.
4. scope 안의 path·kind·mode·size·content hash manifest를 만든다.
5. source class·facet, language·toolchain·guidance를 계산한다.
6. text partition을 만들고 지원 언어의 syntax, available semantic partition을 순서대로 만든다.
7. entity·edge graph와 hardcoding Finding candidate를 계산한다.
8. coverage·limitation·fingerprint를 검증하고 complete generation만 current로 publish한다.

full scan은 자동 주기 실행하지 않는다. 최초 onboarding, 사용자의 `--full`, contract/config/adapter invalidation 또는 손상 복구가 명시된 경우에만 실행한다.

### incremental 후보 계산

Git Project는 이전 ProjectRevision·WorkspaceSnapshot과 현재 local HEAD·porcelain v2 status를 비교한다. tracked modification, staged/unstaged 차이, delete, rename, type change와 untracked를 구분한다. 실제 workspace byte hash가 같으면 content partition을 재사용할 수 있지만 상태 metadata 변화는 WorkspaceSnapshot에 남긴다.

non-Git Project는 path·kind·size·file identity·mtime을 enumeration prefilter로 사용할 수 있으나 **재사용 결정 전 content SHA-256을 확인**한다. timestamp만 같다는 이유로 index를 재사용하지 않는다. 안정적인 enumeration을 보장할 수 없으면 full scan으로 승격한다.

### dirty working tree 우선순위

| source 상태 | scan 입력 |
|---|---|
| clean tracked file | current HEAD tree와 동일함을 확인한 workspace byte |
| staged only | working tree byte가 실제 source, staged blob은 dirty metadata로 별도 기록 |
| unstaged 또는 staged+unstaged | filesystem의 최종 working tree byte |
| deleted | WorkspaceSnapshot에서 absent/tombstone, HEAD byte를 current source로 되살리지 않음 |
| untracked | `scan.include_untracked=true`이면 actual byte 포함 |
| ignored untracked | 기본 제외, explicit widening일 때만 포함 |
| sparse absent | missing limitation, 임의 checkout 금지 |

default branch, upstream과 remote HEAD는 표시·비교 hint일 뿐 현재 source가 아니다. scan은 fetch하지 않으므로 remote가 더 최신인지 `unknown`으로 남긴다.

### scan 중 source 일관성

scan은 서로 다른 시점의 byte를 하나의 complete WorkspaceSnapshot으로 섞지 않는다.

1. 시작할 때 checkout identity, HEAD·status 또는 non-Git enumeration fingerprint를 고정한다.
2. 각 file은 final file identity·size·mtime을 읽고 bounded byte를 hash한 뒤 identity·size·mtime을 다시 확인한다. 달라졌으면 한 번 즉시 다시 읽는다.
3. 두 번째 읽기 중에도 달라지거나 file이 생기고 사라지면 `SCAN_SOURCE_CHANGED_DURING_RUN` limitation과 path ref만 남기고 해당 partition을 partial로 만든다.
4. finalization 직전에 HEAD·status와 scope manifest delta를 다시 probe한다. 시작 fingerprint와 다르면 mixed generation을 publish하지 않고 ScanRun을 `incomplete`로 끝낸다.
5. retry는 새 ScanRun·새 WorkspaceSnapshot으로 시작한다. 이전 run의 일부 partition을 current로 합성하지 않는다.

mtime·size·file identity는 변화 탐지용 precondition일 뿐 content equality 증명이 아니다. 재사용과 snapshot identity에는 실제 content hash가 필요하다.

### partition 재사용 key

| partition | 최소 reuse key | 강제 invalidation 예 |
|---|---|---|
| source inventory | path·kind·mode·content hash·scope fingerprint | include/exclude·symlink·ignored policy 변화 |
| classification | SourceEntry hash + classification Rule fingerprint | marker·override·build metadata 변화 |
| text | source content hash + text analyzer contract | encoding·tokenization contract 변화 |
| syntax | content hash + language ID + parser adapter fingerprint | parser version·language mode 변화 |
| semantic | syntax fingerprint + workspace/package/toolchain fingerprint + adapter fingerprint | lockfile·workspace manifest·toolchain 변화 |
| graph | relevant entity content + edge analyzer fingerprint | source/entity 삭제·target resolution 변화 |
| hardcoding Finding | source/classification + Rule·parameter + required graph tier | Rule·classification·required graph 변화; decision 변화는 재scan 대상 아님 |

workspace manifest, lockfile, toolchain file, AGENTS, canonical docs index와 `.star-control` 설정 변화는 영향 범위가 넓다. 해당 package/workspace·guidance·semantic·Finding partition을 invalidate하며 안전하게 범위를 계산할 수 없으면 full scan으로 승격한다.

### external periodic invocation과 watcher

- 사용자는 OS scheduler나 CI가 `star scan run --mode incremental` 같은 CLI command를 호출하게 할 수 있다.
- Star-Control은 schedule definition, next-run time, retry calendar와 background cron thread를 저장하거나 실행하지 않는다.
- 외부 caller도 같은 IPC·application command, writer lease, idempotency와 freshness contract를 사용한다.
- project watcher는 M1 첫 Slice에 없다. 이후 도입하더라도 change hint만 제공하고 content hash probe를 대체하지 않는다.
- watcher overflow, missed event와 offline 기간은 full/incremental rescan 필요 상태이지 silent current가 아니다.

## freshness 계약

### partition별 상태

| 상태 | 의미 |
|---|---|
| `current` | current source observation과 모든 의미 fingerprint가 index input과 일치 |
| `stale_catalog` | target checkout attachment·identity 또는 source ownership 관계가 달라짐 |
| `stale_source` | ProjectRevision 또는 WorkspaceSnapshot이 달라짐 |
| `stale_config` | scan/index/classification/Rule fingerprint가 달라짐 |
| `stale_adapter` | parser·semantic·graph adapter set이 달라짐 |
| `partial` | 일부 file·language·tier·scope가 incomplete |
| `unverified` | root·Git 상태·hash를 현재 다시 확인하지 못함 |
| `unavailable` | current complete partition이 없음 |

freshness는 Project Catalog, source inventory, text, syntax, semantic, graph, Finding과 Managed Registry partition별로 계산한다. text가 current여도 semantic partition만 stale일 수 있다. Registry partition은 authoritative manifest root·fragment hash, namespace/tombstone set과 binding·consumer 관찰 fingerprint를 별도로 가진다.

### FreshnessProof

각 partition은 다음을 가진다.

- indexed ProjectCatalogSnapshotId·target checkout observation fingerprint
- indexed ProjectRevisionId·WorkspaceSnapshotId
- indexed scan/index/classification/Rule·adapter fingerprint
- indexed content fingerprint와 completed timestamp
- 마지막 probe의 observed HEAD/status/filesystem manifest fingerprint
- probe timestamp와 method
- state와 stable stale reason code array
- 확인하지 못한 scope·file count

### DB가 source보다 오래됐는지 판정

current 결과를 요구하는 query는 bounded freshness probe를 먼저 수행한다.

1. Checkout attachment·identity와 target source ownership relation이 indexed catalog observation과 같은지 확인한다.
2. Git이면 local HEAD, worktree status와 dirty entry hash candidate를 확인한다. non-Git이면 scope manifest delta를 확인한다.
3. Effective scan/index config, classification Rule과 adapter set fingerprint를 다시 계산한다.
4. indexed input과 exact match하면 `current`를 유지한다.
5. 하나라도 다르면 해당 partition을 `stale_*`로 표시하고 이전 index를 current로 반환하지 않는다.
6. probe를 완료하지 못하면 `unverified`이며 empty result나 current로 바꾸지 않는다.

Managed Registry partition은 current source manifest와 indexed manifest hash가 다르면 `stale_source`다. DB snapshot이 더 최근 timestamp를 가져도 source보다 우선하지 않는다. current manifest가 invalid하면 이전 snapshot은 explicit historical query에만 반환하고 current/compatible로 표시하지 않는다.

query option은 `require_current`, `allow_stale_with_warning`, `snapshot_id` 중 하나다. 기본 interactive query는 `require_current`; 진단·과거 비교만 explicit `snapshot_id` 또는 `allow_stale_with_warning`을 사용한다. stale result에는 indexed snapshot과 current observation을 모두 표시한다.

## CLI-only application 계약

### command와 query

| use case | 주요 입력 | 결과 | project source effect |
|---|---|---|---:|
| `project.discover` | root binding set, mode, config fingerprint | ProjectCatalogSnapshot | 없음 |
| `project.list`, `project.get` | catalog snapshot, filter, cursor | Project·Checkout view + freshness | 없음 |
| `project.refresh` | ProjectId, CheckoutId, expected observation | ProjectRevision·WorkspaceSnapshot | 없음 |
| `scan.plan` | ProjectId, CheckoutId, mode, effective config | scope·partition·estimated limit plan | 없음 |
| `scan.run` | plan fingerprint, mode, idempotency key | ScanRun·CodeIndexSnapshot ref | 없음 |
| `scan.status` | ScanRunId 또는 ProjectId | state·partition·coverage·limitation | 없음 |
| `index.status` | ProjectId·CheckoutId | snapshot·freshness·coverage | 없음 |
| `index.search` | query, scope, tier, freshness policy, cursor | entity·source match + quality envelope | 없음 |
| `index.definitions` | entity key 또는 symbol query | definition candidates + tier·resolution | 없음 |
| `index.references` | entity key, direction, scope | reference edges + coverage | 없음 |
| `graph.neighbors` | node key, relation, depth=1 기본 | bounded graph edges | 없음 |
| `finding.list`, `finding.get` | hardcoding category·assessment filter | FindingView + evidence·decision | 없음 |

모든 list/query는 stable cursor와 store revision을 사용한다. graph depth는 resource limit을 넘겨 자동 확장하지 않고 continuation cursor를 반환한다.

### source write 불변식

1. 위 use case DTO에는 patch, replacement text, target write path, delete, rename과 command execution payload가 없다.
2. `star-project`의 1단계 dependency graph에는 `star-execution`, process execution과 write-capable filesystem port가 없다.
3. filesystem port는 enumerate, metadata, bounded read, hash와 final identity 확인만 노출한다.
4. Git port는 discovery·status·object identity read만 제공하고 checkout, add, commit, worktree create/remove, fetch와 merge를 제공하지 않는다.
5. CLI는 `--fix`, `--apply`, `--write`, `--format` 같은 alias를 받지 않는다.
6. hardcoding Finding에서 ChangeRecipe·PatchSet을 자동 만들지 않는다. 이후 write 단계가 별도 command·permission·validation으로 소비한다.

### stable error와 limitation code

- `PROJECT_DISCOVERY_LIMIT`
- `PROJECT_IDENTITY_CONFLICT`
- `PROJECT_CHECKOUT_IDENTITY_CONFLICT`
- `LINKED_WORKTREE_OUT_OF_SCOPE`
- `PROJECT_ROOT_UNAVAILABLE`
- `PROJECT_OWNERSHIP_AMBIGUOUS`
- `SCAN_SCOPE_INCOMPLETE`
- `SCAN_SOURCE_UNREADABLE`
- `SCAN_SOURCE_CHANGED_DURING_RUN`
- `SCAN_CLASSIFICATION_CONFLICT`
- `INDEX_PARSE_FAILED`
- `INDEX_LANGUAGE_UNSUPPORTED`
- `INDEX_SEMANTIC_UNAVAILABLE`
- `INDEX_RESOURCE_LIMIT`
- `INDEX_IDENTITY_CONFLICT`
- `INDEX_STALE_CATALOG`
- `INDEX_STALE_SOURCE`
- `INDEX_STALE_CONFIG`
- `INDEX_STALE_ADAPTER`
- `INDEX_RESULT_PARTIAL`

error는 command 실패를 나타내고 limitation은 성공 snapshot 안의 coverage 제한을 나타낸다. unsupported language 하나가 있더라도 전체 command를 무조건 failed로 만들지 않는다. 그 language가 required tier 대상이면 ScanRun은 incomplete이고, optional tier 대상이면 ScanRun은 succeeded일 수 있지만 해당 partition은 unavailable·coverage 제한이며 그 scope의 no-result를 `confirmed_empty`로 표시할 수 없다.

## persistence·cache·evidence

### 저장 경계

| 위치 | 저장 |
|---|---|
| global management store | Project·Checkout directory, ProjectCatalogSnapshot, cross-project summary edge |
| ProjectId별 management store | SourceEntry, workspace/package/module/symbol/reference, graph, Finding와 CodeIndexSnapshot projection |
| `%LOCALAPPDATA%\Star-Control\cache\project-index\<project-id>\` | adapter별 재생성 가능한 content-addressed cache |
| `<project>\.ai-runs\star-control\management\scans\<scan-run-id>\` | entries manifest, coverage·limitation report와 redacted tool output |

cache는 current truth가 아니다. cache miss·삭제 뒤 같은 input으로 재구축할 수 있어야 하며 management backup·StoreVersionVector에 포함하지 않는다. cache key는 ProjectId, WorkspaceSnapshotId, partition, adapter fingerprint와 index config fingerprint다.

cache directory에는 raw project name·path·사용자 이름을 쓰지 않는다. source file 전체 복사본, secret·개인 path와 민감 literal을 cache에 넣지 않는다. adapter가 안전하게 redaction할 수 없는 intermediate를 요구하면 disk cache를 쓰지 않고 partition limitation을 기록한다.

### generation과 crash

- catalog와 code index는 각각 invisible staging generation에 쓴다.
- batch ordinal·fingerprint·count를 idempotent commit한다.
- finalization은 source manifest, entity/edge reference, coverage와 identity를 검증한다.
- complete generation만 current pointer를 바꾼다.
- crash·cancel·incomplete는 이전 current를 유지하며 staging은 retention 후보가 된다.
- 이전 current가 source보다 오래됐으면 그대로 `stale_source`이지 새 incomplete generation을 current로 가장하지 않는다.

## 2단계 영향 분석에 제공할 입력

2단계 [변경 계획·영향 분석](change-planning-and-impact.md)은 다음 입력을 exact reference로 받는다. `star-project`는 이 자료를 제공할 뿐 task-specific confirmed/possible 판정, risk severity와 Check 선택을 소유하지 않는다.

1. ProjectCatalogSnapshotId와 대상 ProjectId·CheckoutId
2. ProjectRevisionId와 dirty WorkspaceSnapshotId
3. CodeIndexSnapshotId와 partition별 freshness·coverage·limitation
4. SourceEntry classification과 generated/vendor/test/docs-example facet
5. package·module·symbol·definition·reference entity와 resolution tier
6. project·package·contract·dependency graph edge와 confidence
7. config key·Schema ID·error code·constant·public surface candidate
8. hardcoding Finding과 assessment·decision ref
9. unsupported language, parse failure, excluded scope와 no-result reason
10. toolchain·lockfile·canonical docs·AGENTS guidance fingerprint
11. test·docs·generated source의 class/facet과 `tests`·`documents`·`generates`·`generated_from` edge
12. cross-project exported entity key와 provider/consumer edge의 양쪽 snapshot ref
13. Registry task이면 current ManagedRegistrySnapshot, exact source manifest hash, `registry_classification`, declaration·binding·consumer·alias/tombstone edge와 freshness

2단계는 `stale_catalog`, `stale_source`, `stale_config`, `stale_adapter`, `unverified` snapshot을 current 영향 사실로 사용할 수 없다. `partial` 또는 낮은 tier만 있으면 confirmed impact와 possible impact를 분리하고 검사 범위를 넓히거나 review로 보낸다. 1단계 graph를 근거 없이 semantic truth로 승격하지 않는다.

query API는 task-specific traversal 결과를 저장하지 않는다. M2 application이 seed별 bounded `graph.neighbors`·definition/reference query의 snapshot·tier·coverage·limitation을 고정해 pure planning engine에 전달한다. 같은 literal equality는 relation edge가 아니며 ownership·contract identity가 없으면 `unowned_literal` candidate로만 반환한다.

## 외부 공식 근거

확인일은 모두 **2026-07-13**이다.

| 공식 문서 | 선택 이유 |
|---|---|
| [Git worktree](https://git-scm.com/docs/git-worktree) | 한 repository의 main·linked worktree가 공통 repository data와 per-worktree HEAD·index를 나눈다는 경계, `worktree list --porcelain -z` 사용 근거 |
| [Git rev-parse](https://git-scm.com/docs/git-rev-parse) | top-level, Git dir, common dir와 object format을 Git 자체 명령으로 확인하고 `.git` 내부를 임의 parse하지 않기 위한 근거 |
| [Git status](https://git-scm.com/docs/git-status) | staged·unstaged·untracked를 구분하는 stable `--porcelain=v2`와 NUL-delimited parsing 근거 |
| [gitignore](https://git-scm.com/docs/gitignore) | ignore source 우선순위와 tracked file은 ignore 대상이 아니라는 scan scope 근거 |

이 문서는 위 Git CLI surface만 adapter input으로 선택한다. 특정 code indexer, parser framework, language server protocol, graph DB와 cache engine은 비교·fixture·운영 검증 없이 제품 계약에 고정하지 않는다.

## 구현 순서

제품 구현은 별도 승인 뒤 다음 순서로 진행한다.

1. `ProjectCheckout`, `ProjectCatalogSnapshot`, `CodeIndexSnapshot` Rust type·Schema·fixture와 Project v1→v2 migration contract
2. read-only filesystem·Git port conformance와 path/root-binding redaction fixture
3. multiple root·nested repo·linked worktree·non-Git discovery fake와 ProjectCatalogSnapshot
4. source inventory·classification·guidance·toolchain manifest-only adapter
5. text index와 deterministic entity/edge query
6. 첫 지원 언어의 syntax adapter, parse failure·fallback fixture
7. optional semantic adapter capability 경계와 unavailable/partial 표현
8. hardcoding Rule candidate·FindingView와 class별 false-positive fixture
9. full/incremental generation, partition reuse·invalidation과 freshness probe
10. CLI-only E2E, crash·limit·redaction·no source change 검증

## 설계 수용 기준

- 여러 explicit root와 nested Project에서 source ownership 중복이 없다.
- 같은 shared Project의 main·linked worktree는 같은 ProjectId와 다른 CheckoutId를 가진다.
- non-Git Project도 deterministic ProjectRevision·WorkspaceSnapshot을 만들거나 incomplete 이유를 표시한다.
- dirty filesystem byte가 HEAD·default branch보다 우선한다.
- source class·fixture/generated/vendor/docs-example facet과 ignore provenance가 query 가능하다.
- language·build·package manager·lockfile·toolchain·major command의 근거와 confidence가 있다.
- package·module·symbol·definition·reference와 project·contract·dependency graph가 snapshot에 고정된다.
- hardcoding은 candidate/warning/review/allowed로 구분되고 allowed에는 decision ref가 필수다.
- 최초 manual full scan과 revision·hash 기반 incremental scan의 reuse·invalidation 경계가 있다.
- DB/index가 source보다 오래되면 partition이 `stale_*`가 되며 current로 반환되지 않는다.
- parse failure, unsupported language, partial coverage와 no-result가 빈 성공으로 숨겨지지 않는다.
- semantic unavailable 시 syntax/text fallback의 실제 tier와 limitation이 유지된다.
- 자체 scheduler와 필수 watcher가 없고 external caller도 같은 CLI·IPC·Writer 경계를 사용한다.
- read-only use case와 dependency graph에 project source 수정 경로가 없다.
- 2단계가 fresh snapshot·graph·coverage·limitation을 exact input으로 받을 수 있다.
