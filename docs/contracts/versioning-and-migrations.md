# Version과 Migration 계약

## 목적

제품 version, 저장 자료, 설정, Catalog와 통신 protocol은 바뀌는 속도와 호환 범위가 다르다. 하나의 version 숫자로 모두 묶지 않고 각 경계를 독립적으로 판정한다.

## Version 축

| 대상 | 형식 | 증가 기준 |
|---|---|---|
| Star-Control 제품 | SemVer | 사용자 기능·호환성·수정 release |
| 개별 데이터 계약 | positive integer `schema_version` | 직렬화 shape 또는 의미가 바뀔 때 |
| 설정 | `star.config`의 독립 schema version | key·type·병합 의미가 바뀔 때 |
| 로컬 관리 DB | positive integer `management_store_version` | global/project logical relation·coordination·invariant·redaction·projection 의미가 바뀔 때 |
| Catalog descriptor | `format_version` integer | descriptor 형식이 바뀔 때 |
| Catalog 항목 | `item_version` SemVer | 한 Task·Tool·Check·Profile의 내용이 바뀔 때 |
| Managed Registry manifest 형식 | positive integer `schema_version` | root·fragment wire shape 또는 해석 규칙이 바뀔 때 |
| Managed Registry 내용 집합 | `registry_version` SemVer | declaration·namespace·tombstone 또는 호환 정책 집합이 바뀔 때 |
| ManagedDeclaration | `item_version` SemVer | 한 stable ID의 type·binding·lifecycle·consumer 계약이 바뀔 때 |
| ManagedRegistrySnapshot | positive integer `schema_version` | derived Index·관찰·drift record shape가 바뀔 때 |
| ProjectContractManifest | `manifest_version` SemVer | target project surface·baseline·docs·environment policy 집합이 바뀔 때 |
| Contract/Documentation/Environment snapshot | contract별 positive integer `schema_version` | derived observation·fingerprint 의미가 바뀔 때 |
| public surface | surface별 native version 또는 SemVer | API·CLI·Schema·file format·config·error 의미가 바뀔 때 |
| ProjectMigrationManifest | `manifest_version` SemVer | 대상 Project의 target·chain·invariant·adapter 정책 집합이 바뀔 때 |
| 대상 Project migration axis | axis owner가 선언한 native version | data·config·DB·state·file-format reader/writer 의미가 바뀔 때 |
| PerformanceWorkloadSpec | `item_version` SemVer | workload·input·tool·environment·metric·noise protocol이 바뀔 때 |
| LanguageMigrationPlan·EquivalenceReport | 계약별 positive integer `schema_version` | 공존·consumer·equivalence·platform evidence 의미가 바뀔 때 |
| ReleaseManifest | positive integer `schema_version` | release identity·artifact set·상태·Gate·remote proof 의미가 바뀔 때 |
| EvaluationRun | positive integer `schema_version` | subject·cohort·metric·adjudication·recommendation 의미가 바뀔 때 |
| Rule·Check·Profile·Recipe item lifecycle | item SemVer + lifecycle revision | active/deprecated/retired/rejected·replacement·migration 의미가 바뀔 때 |
| ToolPackageManifest | 독립 `format_version` integer | backend·binding·trust 형식이 바뀔 때 |
| 외부 Tool protocol | positive integer | `star_json_stdio_v1` request·response 의미가 바뀔 때 |
| MCP 구현 계약 | `mcp_contract_version` integer | 고정 surface·hash·risk lane·state machine 의미가 바뀔 때 |
| Tool trust·cache | 각 `schema_version` integer | trust scope·durable LKG shape가 바뀔 때 |
| Local IPC | `major.minor` | major는 breaking, minor는 additive negotiation |
| MCP tool | tool별 schema integer | input·result shape 또는 의미가 바뀔 때 |
| Codex Plugin | manifest의 제품 version | 설치 묶음·Skill·Hook·MCP 구성이 바뀔 때 |

한 계약의 `schema_version`을 올렸다고 다른 계약 version을 자동으로 올리지 않는다. 다만 reference 의미가 달라지면 영향을 받는 계약을 명시적으로 함께 올린다.

## 계약 변경 분류

| 변경 | version·호환 처리 |
|---|---|
| 설명·오탈자만 수정, wire 의미 동일 | schema version 유지 |
| optional field 추가 또는 validation 완화 | schema version 증가, older reader의 unknown-field 보존·default·round-trip이 증명될 때만 additive/compatible 후보 |
| enum 값·overload·CLI option 추가 | resolution·exhaustive consumer·unknown-value 정책을 확인해 additive/breaking/unknown 판정 |
| 필수 field 추가, type·기본 의미 변경, enum 제거 | schema version 증가와 migration 필요 |
| ID 의미 변경, secret·permission 경계 변경 | 새 계약 ID 또는 명시적 breaking migration |
| ManagedDeclaration 설명·display message만 변경 | stable ID와 public value 유지, 필요한 문서 binding만 갱신 |
| ManagedDeclaration 의미·owner·type·public stable value 변경 | 새 declaration과 새 public ID, 기존 항목 deprecate·alias·consumer migration |
| removed/reserved Registry ID 또는 public value 재사용 | version과 무관하게 금지, tombstone 유지 |
| 외부 protocol field만 변경 | adapter에서 흡수, core 계약이 같으면 version 유지 |

같은 schema version에서 다른 shape를 배포하지 않는다. Schema와 fixture hash가 달라졌는데 version이 같으면 build를 실패시킨다.

## Compatibility manifest

`specs/compatibility.toml`은 reader·writer별 지원 범위를 기계가 읽는 정본으로 가진다.

```text
contract_id
reader_min_version
reader_max_version
writer_version
migratable_from_versions
preserve_unknown
migration_id
```

제품 시작, state 열기, IPC handshake와 Catalog load에서 이 manifest를 사용한다. 코드 곳곳에 version 비교를 복사하지 않는다.

`specs/compatibility.toml`은 Star-Control 자체 reader/writer 지원 범위의 정본이다. 대상 Project의 public surface baseline과 consumer window는 `.star-control/contracts.toml`의 `ProjectContractManifest`가 소유한다. 두 파일을 합치거나 local DB의 최신 row로 어느 한쪽을 대체하지 않는다.

관리 DB 항목은 `global`과 `project` store kind별로 최소 `reader_min_store_version`, `reader_max_store_version`, `writer_store_version`, `migratable_from_store_versions`, `rebuildable_from_store_versions`와 migration chain을 가진다. 같은 `management_store_version`이라도 지원 store kind가 다르면 compatible하다고 추측하지 않는다. concrete backend schema version은 adapter private이며 public compatibility manifest에 backend 이름을 넣지 않는다.

## 기본 호환 행동

| 입력 상태 | 읽기 | 실행·수정 | 행동 |
|---|---:|---:|---|
| 현재 writer version | 예 | 예 | 정상 처리 |
| 지원 범위 안의 과거 version | 예 | migration 뒤 | dry-run·backup·검증 후 올림 |
| 읽을 수 있지만 migration 미지원 | 제한적 | 아니요 | export·진단만 제공 |
| 현재보다 높은 미래 version | opaque inspection | 아니요 | 원문 보존, 명확한 비호환 오류 |
| schema ID 불명 | metadata만 | 아니요 | 격리하고 실행 거부 |
| 손상·hash 불일치 | 아니요 | 아니요 | 격리, 복구 또는 backup 선택 요청 |

미래 version을 현재 기본값으로 억지 해석하지 않는다. read-only inspection은 ID, version, 크기, hash와 비민감 metadata만 보여주며 내용을 다시 저장하지 않는다.

## Unknown field와 extensions

- 현재 version의 Schema에 없는 일반 field는 producer 결함으로 거부한다.
- 명시된 `extensions` map 안에서는 등록된 namespace만 허용한다.
- 높은 미래 version은 전체 JSON object를 opaque byte로 보존하고 current type으로 round-trip하지 않는다.
- migration은 이해하지 못하는 extension을 보존할 수 있을 때만 자동 진행한다.
- enum의 unknown 값을 임의의 `other`나 기본값으로 바꾸지 않는다.

## Migration 단위

| 대상 | 방식 |
|---|---|
| 설정 | 원본 backup 후 새 파일을 별도 생성하고 effective diff를 보여줌 |
| Snapshot | Event log에서 재생성하거나 version별 변환 |
| Event log | 원본 append-only log는 보존하고 새 version export를 별도 생성 |
| Artifact manifest | artifact byte는 건드리지 않고 metadata manifest를 변환 |
| Catalog | 원본 descriptor를 수정하지 않고 새 snapshot으로 다시 resolution |
| Controller index | 재구축 가능한 index는 migration보다 rebuild 우선 |
| 관리 DB | store별 backup+검증 migration 또는 side-by-side rebuild; 여러 store를 바꾸면 active generation set을 함께 검증 |

Event history를 in-place로 다시 쓰지 않는다. 과거 payload decoder를 유지하거나 검증된 변환 copy와 원본 hash를 함께 보관한다.

## 로컬 관리 DB version

상세 저장 경계와 StoreStatus는 [공통 개발 관리와 로컬 관리 DB 계약](development-management.md)이 소유한다.

### version 증가 기준

다음 변경은 `management_store_version`을 올린다.

- ProjectId partition, relation cardinality, uniqueness 또는 transaction 불변식 변경
- global/project 책임 배치, `StoreVersionVector`, coordination receipt 또는 active generation set 의미 변경
- Finding·Symbol identity와 fingerprint input 의미 변경
- scan generation publish, event·projection commit 또는 idempotency 의미 변경
- redaction contract가 저장 허용 field를 좁히거나 넓히는 변경
- local Suppression·Baseline·Disposition와 ChangePlan 보존 의미 변경
- backup·integrity·read-only recovery가 해석해야 하는 logical metadata 변경

index 추가, query plan과 backend 내부 page layout처럼 public relation 의미를 바꾸지 않는 최적화는 logical version을 유지할 수 있다. 단, 같은 logical version의 store를 이전 binary가 안전하게 읽을 수 있어야 한다.

### open mode

| 상태 | open mode | 허용 동작 |
|---|---|---|
| 현재 version, integrity healthy | `read_write` | 정상 command·query |
| 지원 과거 version | `migration_required` | status; 별도 승인된 migration path 또는 restore/rebuild 선택 |
| 읽을 수 있는 미래 version | `recovery_only` | status, restore/rebuild plan·apply, 가능한 local-state export |
| suspect·active-set mismatch | `recovery_only` | status, verified restore, side-by-side rebuild, 가능한 local-state export |
| corrupt | `recovery_only` | 원본 generation 보존, verified restore 또는 source rebuild |

CLI·MCP는 open mode를 선택하거나 DB handle을 열지 않는다. Controller lifecycle이 mode를 결정하고 application service가 허용 command를 제한한다.

global store와 각 project store는 별도 `ManagementStoreStatus`와 revision을 가진다. Controller는 top-level `active-set` manifest가 가리키는 `(store_id, generation, version, relative_locator, header_fingerprint)` 조합만 함께 연다. `header_fingerprint`는 immutable generation header·locator의 canonical hash이며 쓰기 중 변하는 live DB file byte hash가 아니다. manifest에 없는 directory를 최신이라는 이유로 자동 선택하지 않는다. 정지점이 고정된 backup·migration candidate에는 별도의 file `byte_sha256`을 반드시 기록한다.

### migration class

| class | 예 | 자동 적용 |
|---|---|---:|
| `rebuildable_projection` | index·Symbol edge·source-derived Finding projection 재계산 | backup 뒤 설정이 허용하면 가능 |
| `lossless_local_state` | local decision field의 결정적 shape 변환 | 명시적 계획·backup·검증 필요 |
| `semantic_local_state` | disposition·suppression 의미 또는 redaction 경계 변화 | 사용자 승인 필요 |
| `unsupported` | 의미 손실, unknown extension, future version | 쓰기 금지 |

각 migration은 전·후 StoreStatus, 예상 row·byte, 필요한 임시 공간, 영향받는 local-only state, rollback generation과 검증 항목을 dry-run 결과로 반환한다.

### 1단계 Project v1→v2 checkout migration target

`migrate.star.project.v1-to-v2`는 [Project Catalog·Code Index 계약](project-catalog-and-code-index.md)의 `ProjectCheckout` 도입을 위한 current migration 계약이며 P-0041에서 code·Schema·DB migration과 CLI 경계를 구현했다. Project/root-binding cardinality와 global/project reference 의미를 바꾸므로 `lossless_local_state`이며 `management.auto_migrate_rebuildable` 대상이 아니다.

dry-run plan은 expected global/project `StoreVersionVector`, source `management_store_version`, 정렬된 ProjectId, 각 v1 `root_binding_id`, immutable `checkout_id_allocations`, 변환할 ProjectRef·event/projection count, invalidation 대상 scan generation, candidate binding envelope 목록, backup-set 위치, 예상 공간과 rollback active-set을 가진다. CheckoutId는 dry-run에서 한 번 발급해 plan fingerprint에 넣고 apply·retry가 같은 allocation을 재사용한다. apply 중 새 ID를 다시 뽑지 않는다.

변환은 다음 순서다.

1. Writer lease를 잡고 scan·project attachment mutation을 quiesce한 뒤 expected StoreVersionVector를 다시 확인한다.
2. global store, 영향받는 모든 project store와 protected binding의 consistent backup-set을 별도 root에 만들고 각 file digest와 backup fingerprint를 검증한다. active-set은 derived manifest이므로 migration 뒤 Controller 재시작 때 current store header에서 재생성한다.
3. v1 binding을 current-user context에서 열어 ProjectId와 final filesystem identity를 검증한다. plaintext path·그 hash는 plan, event, DB와 backup-set에 넣지 않는다.
4. attached v1 Project마다 plan에 고정한 CheckoutId로 `ProjectCheckout` 하나를 candidate global generation에 만든다. detached Project는 checkout을 합성하지 않는다.
5. protected binding store에는 같은 `root_binding_id`를 가리키되 ProjectId·CheckoutId를 가진 v2 envelope를 atomic replace한다. 이 checkpoint 전에 verified v1 backup이 반드시 존재하며, 아직 global store가 v1인 중간 상태는 `MigrationRequired`라서 Controller가 application service를 열지 않는다.
6. Project v2에서 `root_binding_id`를 제거하고 정렬된 `attached_checkout_ids`와 derived registration state를 쓴다. 같은 binding이 여러 ProjectId에 연결되거나 manifest identity가 다르면 전체 migration을 block한다.
7. active Goal·Context의 ProjectRef v1은 exact ProjectId와 유일한 matching checkout을 증명할 수 있을 때만 v2 `checkout_id`로 바꾼다. 0개·2개 이상이면 해당 run을 임의 변환하지 않고 migration을 block한다.
8. 기존 P0 ScanRun·Symbol·Reference·Finding record는 역사 evidence로 보존할 수 있지만 CodeIndexSnapshot으로 승격하지 않는다. 새 index partition은 `unavailable`이며 checkout current probe와 첫 full scan 뒤에만 current가 된다.
9. candidate global/project relation, ProjectId partition, binding envelope, event/projection revision, redaction과 ArtifactRef integrity를 검증한다.
10. project partition과 binding checkpoint가 모두 통과한 뒤 global store를 마지막 단일 transaction으로 v2 전환한다. startup은 global/project/binding set을 다시 검증하고 active-set을 재생성한다. 실패하거나 사용자가 rollback을 승인하면 exact backup fingerprint로 v1 DB와 binding을 복원한다.

공개 CLI 흐름은 `star management migrate project-v1-v2 plan --json`으로 dry-run 문서를 얻고, 그 JSON을 보존한 뒤 `apply <plan-json> --approve <plan_fingerprint>` 또는 `rollback <plan-json> --approve <backup_fingerprint>`를 호출하는 순서다. plan은 duplicate/unknown field를 거부하며 Controller는 `%LOCALAPPDATA%/Star-Control/migration-backups/` 아래 plan fingerprint별 독립 backup root만 사용한다. apply/rollback 뒤에는 Controller 재시작이 필요하다.

dry-run과 apply 결과는 migrated/attached/detached/blocked Project count, allocation fingerprint, preserved legacy scan count, required full-scan ProjectId와 loss report를 가진다. partial candidate는 active가 아니며 retention 전 보존한다. migration 성공만으로 ProjectCatalogSnapshot·CodeIndexSnapshot을 만들거나 freshness를 current로 표시하지 않는다.

### 2단계 ChangePlan v1→v2 planning migration target

`migrate.star.change-plan.v1-to-v2`는 [공통 개발 관리 계약](development-management.md#2단계-changeplan-v2-target)의 일반 사용자 계획·ScopeRevision·ImpactAnalysis 연결을 위한 **목표 migration 계약**이다. 현재 code·Schema·DB migration이 구현됐다는 뜻이 아니다.

새 TaskSpec·ScopeRevision·ImpactAnalysis는 신규 document이므로 과거 row를 만들 필요가 없다. 그러나 active ChangePlan v1의 의미를 바꾸므로 다음처럼 처리한다.

1. `finding_refs`, typed `recipe_refs`, target WorkspaceSnapshot과 validation ref가 valid한 v1 row만 candidate로 읽는다.
2. v1 row는 `change_origin=finding_recipe`로만 변환하고 user-planned 의도나 broader scope를 추측하지 않는다.
3. TaskSpec·ScopeRevision·ImpactAnalysis가 없으므로 `readiness=blocked`, `status`는 원래 terminal 여부를 보존한다.
4. v1 `ready`·`applied` active row는 사용자가 TaskSpec을 만들고 current WorkspaceSnapshot에서 2단계 재계획하기 전에는 PatchSet prepare/apply 입력으로 재사용하지 않는다.
5. terminal `validated|abandoned` row는 historical evidence로 보존하고 새 plan graph의 current ref로 승격하지 않는다.
6. migration은 v1 field·event·ArtifactRef hash를 보존하고 v2 추가 field의 absent reason을 `legacy_planning_context_missing`으로 기록한다.

dry-run은 row별 convertible/blocked/terminal count, active PatchSet relation, current WorkspaceSnapshot availability, required user replan 목록, candidate output hash와 rollback generation을 반환한다. local operational state 의미 변경이므로 `lossless_local_state`이고 backup·검증 없이 자동 migration하지 않는다.

### 3단계 Rule·Baseline·Suppression·Disposition v1→v2 migration target

`migrate.star.rule.v1-to-v2`, `migrate.star.baseline.v1-to-v2`, `migrate.star.suppression.v1-to-v2`와 `migrate.star.disposition.v1-to-v2`는 [공통 개발 관리 계약](development-management.md)의 M3 목표를 위한 **목표 migration 계약**이다. 현재 migration code·Schema·DB migration이 구현됐다는 뜻이 아니다. 네 migration은 같은 release에서 제공하더라도 별도 candidate와 migration ID를 가지며, 참조 graph가 함께 검증되기 전에는 일부만 active publish하지 않는다.

Rule migration은 기존 source Rule을 `rule_domain=scan_finding`으로 명시하고 v1 analyzer·identity·redaction 의미를 보존한다. 같은 Rule ID에 v2 descriptor byte가 달라지므로 manifest가 지정한 호환 SemVer로 올리고 새 definition fingerprint를 만든다. identity contract가 같다는 old→new RuleRef compatibility mapping을 함께 생성한다. producer, severity/confidence, applicability, identity input 또는 redaction 의미가 하나라도 달라지면 compatible mapping을 만들지 않고 관련 Baseline·Suppression을 `incompatible|stale`로 둔다. `validation_diagnostic` Rule은 기존 source Rule에서 합성하지 않고 검토된 built-in Registry source와 fixture를 통해 신규 추가한다.

Baseline migration은 다음 순서와 판정을 따른다.

1. v1 Baseline의 source ProjectRevision·WorkspaceSnapshot, scan config, Rule set, `finding_fingerprints`, status와 정본 origin을 읽고 hash를 고정한다.
2. 각 `finding_fingerprint`를 `subject_kind=finding`인 v2 `BaselineEntry`로 옮긴다. fingerprint byte와 BaselineId는 바꾸지 않는다.
3. 원본 ScanRun·Rule snapshot에서 RuleRef, severity, ownership/scope와 comparison 의미를 증명할 수 있으면 채운다. 현재 Catalog를 과거 Catalog처럼 사용하거나 누락 값을 추측하지 않는다.
4. 모든 entry와 complete coverage를 증명하면 candidate status를 원래 `active|superseded` 의미에 맞게 보존할 수 있다. 하나라도 RuleRef·coverage·fingerprint contract를 증명하지 못하면 candidate를 `invalid`로 만들고 `requires_baseline_recreation` reason을 남긴다.
5. v1과 v2 set fingerprint, entry count, Project·scope binding과 정렬 determinism을 검증한다. Diagnostic entry를 합성하지 않는다.

Suppression migration은 다음 순서와 판정을 따른다.

1. v1 exact `finding:<fingerprint>` selector는 `subject_kind=finding`, 동일 full fingerprint와 동일 reason·actor·expiry·origin을 가진 v2 exact selector로 lossless 변환한다.
2. Rule·path·symbol selector는 당시 Rule definition fingerprint와 bounded scope를 보존된 snapshot에서 exact하게 복원할 수 있을 때만 v2 active candidate가 된다.
3. 의미를 exact하게 복원할 수 없으면 삭제하거나 넓혀 해석하지 않고 `stale`로 보존한다. source v1이 이미 `expired`이면 그대로 보존하고, migration이 wall clock을 읽어 새 expired 판정을 만들지 않는다. `expires_at`은 lossless 이동하며 post-migration Gate evaluator가 명시적 evaluation input으로 현재 상태를 계산한다.
4. `subject_kind=diagnostic` suppression, wildcard, permanent 승인 또는 새 expiry를 migration이 합성하지 않는다.
5. source binding, selector match set과 redaction을 검증한다. migration 뒤 match 대상이 하나라도 넓어지면 전체 candidate를 block한다.

Disposition migration은 v1 Finding ID·fingerprint·decision·reason·scope·expiry와 provenance를 `subject_kind=finding`인 v2 record로 lossless 이동한다. Diagnostic disposition을 합성하지 않고, 원본 Finding fingerprint와 scope를 증명할 수 없으면 `stale`로 보존한다. migration은 false-positive Disposition을 Suppression으로 바꾸거나 Gate 예외를 새로 만들지 않는다.

네 dry-run은 source/target schema version, 정렬된 RuleId·BaselineId·SuppressionId·DispositionId, compatible RuleRef mapping, convertible/stale/expired/invalid count, 의미를 증명한 snapshot ref, active GateDecision reference count, candidate document hash, 예상 DB row·byte, backup-set과 rollback generation을 반환한다. active GateDecision과 EvidenceBundle은 역사 evidence로 다시 쓰지 않는다. migration 뒤 새 Gate 평가만 v2 revision을 참조한다.

Baseline은 기존 부채를 허용하는 보안 우회가 아니고 Suppression은 pass가 아니므로, migration 성공만으로 Gate 결과를 `auto_pass`로 만들 수 없다. 새 Validator Registry·EffectiveConfig·CatalogSnapshot에서 current validation을 실행하고 [공통 검증·품질 Gate](../features/common-validation-gate.md)의 subject binding과 ratchet 판정을 통과해야 한다. local/shared 결정 의미가 바뀌는 `semantic_local_state` migration이므로 dry-run·backup·사용자 승인·post-migration conformance 없이 자동 활성화하지 않는다.

#### M3 validation evidence version 전이

M3가 확장하는 validation evidence는 역사 사실을 새 current evidence처럼 승격하지 않는다.

| source | target 처리 | 자동 Gate 재사용 |
|---|---|---:|
| Diagnostic v1 | 원본 byte를 보존하고 historical CatalogSnapshot에서 exact RuleRef를 증명할 때만 v2 observation projection 생성. `suppressed\|resolved`는 raw observation과 suppression/lifecycle projection으로 분리 | 아니요 |
| ValidationRun·ValidationResult v1 | exact ProjectRevision·WorkspaceSnapshot·plan·config·Catalog·Tool binding을 원본에서 모두 증명할 수 없으면 historical/unverified로 유지 | 아니요 |
| GateDecision v1 | 당시 decision과 required/satisfied run ref를 역사 evidence로 보존. v2 RunSatisfaction·multi-project binding set을 합성하지 않음 | 아니요 |
| EvidenceBundle v1 | 기존 manifest와 GateDecisionRef를 보존. v2 단방향 graph로 in-place 재작성하지 않음 | 아니요 |
| ReviewPack | `star.review-pack` v1 신규 생성. current EvidenceBundle v2를 입력으로만 생성 | 해당 없음 |

`migrate.star.diagnostic.v1-to-v2-projection`은 append-only/side-by-side projection migration이다. RuleRef, 원래 observation status, producer와 evidence relation을 모두 증명한 항목만 `confirmed|suspected`를 보존한다. 하나라도 없으면 `observation_status=unverified`, migration reason과 source v1 ref를 남긴다. source binding을 추측하거나 현재 Catalog를 과거 Rule 정의처럼 사용하지 않는다.

GateDecision v2, EvidenceBundle v2와 ReviewPack v1은 migration output이 아니라 current M3 evaluation/packaging output이다. 과거 v1 pass를 current source의 `clean_pass`나 `ratchet_satisfied`로 변환하지 않고 새 ValidationPlan·current probe·ValidationRun을 요구한다.

### 4단계 ChangeRecipe·PatchSet v1→v2 migration target

`migrate.star.change-recipe.v1-to-v2`와 `migrate.star.patch-set.v1-to-v2-history`는 [안전한 Patch·Refactor·codemod 엔진 계약](safe-patch-and-codemod.md)의 **목표 migration 계약**이다. 현재 migration code·Schema·DB migration이 구현됐다는 뜻이 아니다. 새 `RecipeExecution` v1과 `PatchApplication` v1은 과거 row에서 합성하지 않고 4단계 current command가 신규 생성한다.

ChangeRecipe v1은 stable Recipe ID·version·origin·definition byte를 보존하되 다음 v2 필드를 과거 설명이나 command text에서 추측하지 않는다.

- input JSON Schema와 default 적용 후 fingerprint
- 대상 language·capability, `rewrite_kind`와 assurance
- typed TargetSelector와 resolution requirement
- machine-checkable precondition·expected postcondition
- registered transformer binding과 idempotence contract

위 항목을 review된 source descriptor로 모두 보완하고 새 definition fingerprint·SemVer를 발행한 Recipe만 v2 active candidate가 된다. raw literal 전역 치환, shell·PowerShell·`cmd` 문자열, 동적 script 또는 live target mutator를 가진 v1 Recipe는 자동 변환하지 않고 `blocked_reauthor_required`로 보존한다. migration이 command string을 ToolDescriptor나 typed argument로 포장해 신뢰를 합성하지 않는다.

PatchSet v1은 역사 preview·application evidence로만 읽는다. diff byte만 보고 before hash·mode·selector·RecipeExecution·M2 reconciliation·reverse operation을 합성하지 않는다. v1 `prepared|applied` row를 v2 apply input이나 current Gate evidence로 재사용하지 않으며, 사용자가 같은 변경을 원하면 current source에서 Recipe prepare·M2 reconciliation·M3 pre Gate를 다시 수행해 새 PatchSet v2를 만든다. 원본 v1 ID·status·artifact hash·event relation은 historical namespace에 보존한다.

두 migration dry-run은 convertible/blocked/historical count, active ChangePlan·PatchSet reference, 누락된 v2 field, raw command 존재 여부의 redacted reason, source/target definition fingerprint, candidate output hash, 예상 store 변경과 backup·rollback generation을 반환한다. Recipe Catalog source와 local PatchSet history는 소유자가 다르므로 하나의 원자적 migration인 것처럼 묶지 않으며, 각 candidate를 side-by-side 검증한 뒤 reference compatibility manifest로 연결한다. source 의미와 local application eligibility가 바뀌는 `semantic_local_state` migration이므로 backup·사용자 승인·post-migration conformance 없이 자동 활성화하지 않는다.

### 5단계 Managed Registry onboarding·lifecycle migration target

5단계 migration의 정본은 DB row 변환이 아니라 Git manifest와 consumer source의 review 가능한 변화다. exact field와 상태 전이는 [관리형 Symbol·상수·에러 코드 Registry 계약](managed-symbol-registry.md)이 소유한다.

1. scanner 결과는 `candidate` evidence로만 보존한다. 사용자가 ownership·namespace·type·stable public value와 consumer를 승인한 뒤에만 `ManagedDeclarationChangeIntent`를 만든다.
2. candidate promotion은 M2 영향 분석, M4 dry-run `PatchSet`, 승인과 M3 pre/post Gate를 거쳐 새 manifest declaration을 추가한다. 과거 DB row를 manifest 정본으로 승격하거나 source 문자열을 직접 바꾸지 않는다.
3. 기존 shared value를 onboarding할 때 실제 definition/reference를 M1 Index로 관찰한다. 같은 raw 값이라도 의미·owner가 다르면 별도 declaration 또는 local constant로 남긴다.
4. public identifier 변경은 새 declaration을 active로 만들고 기존 declaration을 deprecated로 전이한다. bounded AliasRecord, consumer별 최소 지원 version·전환 deadline과 accepted declaration version을 명시한다.
5. 제거는 모든 current required consumer의 전환, alias window 종료, removed-reference 0건, M3 post Gate와 complete evidence 뒤에만 허용한다. removed ID·public value와 namespace tombstone은 영구 보존한다.
6. generated output은 source manifest와 generator identity에서 재생성하며 직접 migration하지 않는다. existing handwritten consumer source는 typed codemod 또는 manual-review PatchSet으로 전환한다.
7. 여러 Project의 compatibility와 미전환 consumer는 read-only로 계산한다. cross-repo 실제 적용은 9단계 migration workflow까지 거부한다.

`ManagedRegistrySnapshot`은 rebuildable derived Index다. source manifest와 다르면 과거 snapshot을 current로 migrate하지 않고 `stale`로 표시한 뒤 source에서 재계산한다. invalid current manifest가 있으면 last-known snapshot은 historical query에만 남고 current compatibility 근거가 되지 않는다.

### 6단계 public contract compatibility·lifecycle target

6단계는 persisted DB migration과 public consumer migration을 구분한다. exact 비교 계약은 [계약 호환성·문서·설정·개발 환경 관리](contract-compatibility-and-environment.md)가 소유한다.

1. baseline은 승인된 release/Git artifact의 immutable hash와 activation approval을 가진다. branch 이름, mutable tag, current dirty checkout과 DB 최신 row는 baseline이 아니다.
2. baseline/current surface는 API·CLI·Schema·file format·config·error code kind별 canonical observation으로 만든다. snapshot 자체 version과 대상 public surface version을 혼합하지 않는다.
3. 변화는 `unchanged|compatible|additive|breaking|unknown`으로 분류한다. confirmed breaking이 하나라도 있으면 report는 breaking이고, required surface 하나라도 partial/unverified이면 compatible보다 강한 결론을 만들지 않는다.
4. additive는 기존 소비자 행동 보존과 의도된 public expansion을 모두 요구한다. optional field·enum·overload·command 추가만 보고 자동 additive로 판정하지 않는다.
5. breaking change는 replacement, finite compatibility window, owner, 소비자별 migration requirement와 migration guide를 요구한다. 해당 필드가 없는 report는 migration plan이 아니다.
6. deprecation 시작 version·마지막 지원 version·선택적 UTC boundary를 명시한다. 기간 종료만으로 제거하지 않고 current complete consumer coverage, old reference 0건과 M3 post Gate를 요구한다.
7. public source, Schema/file descriptor, generated reference, docs, compatibility metadata와 required consumer migration guide는 하나의 ChangePlan·PatchSet lineage에서 동시 변경한다. 적용 불필요 항목은 evidence가 있는 `not_applicable`이어야 한다.
8. historical `CompatibilityReport`는 immutable evidence다. baseline·source·Registry·Catalog·Tool·environment가 바뀌면 기존 report를 migration해 current pass로 승격하지 않고 새 report를 만든다.
9. 6단계 top-level 계약 8개의 v1은 과거 DB row에서 합성하지 않는다. current source에서 신규 생성하며 valid/invalid/future Schema fixture와 canonical fingerprint golden을 먼저 구현한다.

config key는 declaration 존재 `declared`, M5 lifecycle `active→deprecated→removed`와 current 관찰 `documented|read|overridden`을 분리한다. migration은 deprecated/removed 상태를 문서나 reader presence로 추측하지 않으며, `EffectiveConfig` override actual value나 environment variable 값을 migration evidence에 복사하지 않는다.

### backup·교체·rollback

- migration·repair 전 exact active-set·StoreVersionVector에 고정된 `BackupPlan`을 만들고 승인 뒤 consistent online backup과 byte hash manifest를 만든다.
- backup set manifest는 global/project entry 전체의 version·revision·relative locator·size·SHA-256을 고정하고 각 store의 read-only integrity·event chain·관계를 확인한 뒤 마지막에 쓴다.
- backend가 전체 migration transaction을 보장하지 않으면 새 store generation을 만들고 변환·검증한 뒤 top-level active-set을 atomic replace한다.
- 새 generation은 relation·partition·fingerprint·event/projection·ArtifactRef integrity를 통과해야 한다.
- 교체 뒤 startup 검사에 실패하면 이전 active generation으로 돌아가고 실패 evidence를 남긴다.
- backup, 손상 store와 이전 generation은 retention plan·permission 전 삭제하지 않는다.
- backup·restore apply는 plan fingerprint별 durable typed result receipt를 사용한다. manifest 또는 activation 뒤 receipt 전 crash는 byte/set을 재검증해 같은 결과로 수렴하며 새 generation이나 backup을 중복 생성하지 않는다.

### rebuild

관리 DB가 없거나 복구할 수 없으면 attached Project root, Git 선언·source, Catalog와 검증된 `.ai-runs` manifest에서 새 generation을 만든다.

- source-derived ProjectRevision, WorkspaceSnapshot, Symbol, Reference, Finding은 새 scan으로 계산한다.
- shared Suppression·Baseline은 Git 선언에서 import한다.
- `.ai-runs`에서는 strict `.artifact-ref.json` sidecar와 실제 byte의 ProjectId·path·size·hash·redaction이 모두 맞는 ArtifactRef만 reindex한다. P-0054는 artifact byte를 ValidationResult·GateDecision semantic document로 복원하지 않는다.
- local-only Disposition, local Suppression, 과거 idempotency와 actor event는 backup·export가 없으면 복구하지 못했다고 명시한다.
- rebuild는 과거 ScanRunId·timestamp를 재생성하지 않고 `reconstructed_from`과 completeness를 기록한다.
- rebuild plan은 root binding ID, current source revision·config fingerprint와 verified/rejected artifact inventory fingerprint를 고정한다. apply 전 inventory가 달라지면 stale plan으로 거부한다.

### backend와 dependency gate

public 계약은 특정 DB를 선택하지 않는다. P0 private adapter의 concrete 선택은 [ADR-0008](../decisions/ADR-0008-P0-embedded-relational-backend.md)에 따라 `star-state` 내부의 `rusqlite 0.40.1` bundled backend로 확정됐다. 선택 전에 다음 항목을 비교했고, dependency를 추가한 뒤에도 같은 항목을 release gate로 검사한다.

- Windows x64·ARM64 지원과 process crash 내구성
- single-writer transaction, consistent backup와 integrity 검사
- side-by-side migration·read-only open 가능성
- license, 보안 update, binary 크기와 유지보수 상태
- Rust adapter의 오류·cancellation·threading 경계

선정 결과는 `star-state` private adapter에만 반영한다. StarConfig, CLI, MCP와 persisted domain contract에는 backend 이름, SQL, pragma, connection string이나 DB filename을 추가하지 않는다. backend 교체는 repository conformance를 다시 통과해야 하지만 public document migration을 요구해서는 안 된다.

## Migration 절차

이 절의 기본 대상은 Star-Control이 소유한 config·persisted document·Catalog·management store다. 대상 Project의 범용 data·config·DB migration은 같은 안전 순서를 재사용하되 [8단계 Migration·성능·언어·플랫폼 계약](migration-performance-and-platform.md)의 `ProjectMigrationManifest`, `MigrationPlan`, checkpoint·attempt·invariant·Gate 계약을 따른다. 범용 migration을 `star-state` private DB migration으로 실행하거나 Star-Control 자체 management store를 project manifest로 제어하지 않는다.

1. **발견**: 모든 관련 파일의 schema ID, version, hash, 크기와 reference graph를 읽는다.
2. **잠금**: 대상 Goal·state store의 새 mutation을 막고 실행 중 effect가 없는지 확인한다.
3. **계획**: 적용할 migration chain, 필요한 disk, 예상 변경, 호환 불가 extension을 계산한다.
4. **Dry-run**: 실제 쓰기 없이 변환하고 validation·reference·EffectiveConfig diff를 만든다.
5. **Backup**: 같은 volume의 versioned backup 또는 새 store를 만들고 manifest와 hash를 기록한다.
6. **변환**: 각 migration을 순서대로 새 임시 위치에 적용한다.
7. **검증**: 새 Schema, 불변식, reference, event sequence·hash와 sample replay를 검사한다.
8. **교체**: 검증된 결과만 atomic pointer 또는 directory rename으로 활성화한다.
9. **기록**: migration ID, 제품 version, actor, 전·후 hash와 결과 event를 남긴다.
10. **정리**: retention과 사용자 승인 전에는 backup을 삭제하지 않는다.

실패하면 활성 pointer를 바꾸지 않는다. 교체 뒤 시작 검사에서 실패하면 backup으로 rollback하고 실패 evidence를 보존한다.

### 하이브리드 store migration

여러 store가 영향을 받는 migration은 다음 규칙을 추가한다.

1. global store와 영향받는 ProjectId를 정렬하고 expected `StoreVersionVector`를 고정한다.
2. 새 mutation을 quiesce하고 각 store의 consistent backup과 하나의 backup-set manifest를 만든다.
3. project store를 side-by-side candidate generation으로 변환·검증하고, 그동안 기존 active set만 query에 노출한다.
4. global store를 candidate generation으로 변환하고 모든 project receipt·relation·version compatibility를 검증한다.
5. candidate 전체의 hash를 가진 새 `active-set` manifest를 작성·flush한 뒤 top-level pointer를 atomic replace한다.
6. startup 재검증이 끝날 때까지 이전 active set과 backup set을 보존한다.

부분 변환된 candidate directory는 active가 아니며 retention의 orphan candidate다. 자동 migration은 source-derived projection만 바꾸고 data loss·redaction 확대가 없는 경우에 한하며 항상 pre-migration backup을 먼저 만든다. local decision 의미, redaction 의미, Project identity 또는 cross-store relation을 바꾸는 migration은 dry-run 결과와 손실 가능성을 보여주고 명시적 승인을 받는다.

손상·미래 version 또는 active-set 불일치를 발견하면 Controller는 read-write handle을 닫는다. read-only recovery, verified backup restore, source rebuild 중 어느 generation을 활성화할지는 진단을 제시한 뒤 사용자 선택을 받으며 자동 전환하지 않는다.

## Migration 구현 규칙

- migration ID는 `migrate.<contract-id>.vN-to-vM` 형식의 stable ID다.
- 각 migration은 같은 입력에 같은 출력이 나오는 deterministic 변환이어야 한다.
- 이미 변환된 입력에 재적용해도 중복 side effect가 없어야 한다.
- version을 건너뛰는 migration도 내부적으로 검증된 연속 chain을 사용한다.
- migration에서 네트워크, AI 판단과 현재 시각에 의존하지 않는다.
- 의미를 자동 결정할 수 없으면 placeholder를 만들지 않고 사용자 선택이 필요한 Diagnostic을 반환한다.
- secret 원문, 사용자 절대 경로와 기존 evidence를 새로 노출하지 않는다.

## IPC 호환

- handshake 전에 application message를 보내지 않는다.
- protocol major가 다르면 `CODEX_PROTOCOL_MISMATCH`가 아니라 `IPC_PROTOCOL_MISMATCH`로 연결을 종료한다.
- 같은 major의 minor 차이는 양쪽 feature set 교집합만 사용한다.
- 새 command와 optional result field는 minor 증가로 추가할 수 있다.
- 기존 command 의미, 필수 field와 error 처리 변경은 major 증가가 필요하다.
- Controller가 새 protocol로 update될 때 기존 연결은 draining하고 새 handshake를 요구한다.

## MCP tool 호환

MCP client에 공개하는 search·describe·risk lane·Operation·승인 surface는 process lifetime 동안 고정한다. 외부 EXE와 Registry action 추가는 MCP tool Schema 변경이 아니다.

contract v1의 exact 기준은 [MCP 구현 동결 계약](mcp-implementation-contract.md)이다. 기준 protocol은 `2025-11-25`, 최소 호환은 `2025-06-18`이며 공식 Rust SDK의 protocol conformance fixture로 확인한다.

- 고정 MCP input·result의 additive optional field는 해당 schema version을 올린다.
- 고정 field 의미나 risk lane annotation의 breaking change는 새 tool 이름을 병행하거나 제품 major에서 교체한다.
- ToolResult는 항상 자체 `schema_version`을 반환한다.
- MCP adapter와 Controller IPC가 서로 지원하지 않는 고정 surface version이면 실행 전에 readiness를 실패시킨다.
- 실제 action Schema 호환성은 `star_tool_describe`의 descriptor version·hash로 판정한다.
- fixed tools/list 12개, annotation, server instructions와 MCP Tasks 미광고 결정이 바뀌면 ADR과 `mcp_contract_version` 증가가 필요하다.

## 외부 Tool Registry 호환

- 같은 manifest `format_version`에서는 알려진 backend·binding kind만 허용한다.
- 더 높은 미래 format은 package를 실행하지 않고 ID, version과 hash만 진단한다.
- ToolDescriptor input·output Schema가 바뀌면 package version과 RegistrySnapshot hash가 바뀐다.
- Controller는 검증된 package candidate로 새 immutable RegistrySnapshot을 atomic publish하며 MCP connection에 snapshot을 고정하지 않는다.
- invoke는 describe에서 받은 `descriptor_hash`가 최신인지 확인하고 다르면 재설명을 요구한다.
- 이미 시작한 invoke는 lease한 descriptor와 executable identity를 유지하고 새 invoke만 새 snapshot을 사용한다.
- 잘못된 optional package migration은 package last-known-good를 유지하며 다른 package update를 막지 않는다.
- `star_json_stdio_v1` adapter EXE는 request와 response의 `protocol_version=1`을 확인하고 다른 version을 추측해 처리하지 않는다.
- manifest migration은 외부 action을 실행하지 않는다. path·Schema·permission·protocol 의미가 달라지면 다시 trust하며 executable byte 변경은 `pinned_hash`, `version_compatible`, `follow_path` 정책대로 판정한다.
- v1 TOML의 exact key·enum·default는 [ToolPackageManifest Reference](tool-package-manifest-reference.md)를 따른다. 같은 `format_version=1`에서 다른 기본값을 배포하지 않는다.
- manifest·package·descriptor·arguments·snapshot·scope hash는 contract v1에서 RFC 8785 JCS + SHA-256이다. 알고리즘 변경은 기존 hash를 재해석하지 않고 새 hash contract version을 만든다.
- ToolTrustRecord와 ToolRegistryCache의 future version은 실행에 사용하지 않고 source·trust에서 다시 구축하거나 migration한다.

## Config migration의 추가 규칙

- migration 전후 EffectiveConfig를 field별 provenance와 함께 비교한다.
- 권한이 넓어지거나 비용 한도가 커지는 변화는 자동 적용하지 않고 사용자에게 차이를 보여준다.
- 없어진 key는 조용히 버리지 않고 replacement 또는 제거 이유를 진단에 남긴다.
- config key declaration·Schema·docs·semantic reader·override provenance·consumer와 lifecycle을 `ConfigKeyTrace`로 연결한다.
- complete semantic reader coverage가 없으면 key를 unused로 확정하거나 migration에서 자동 제거하지 않는다.
- environment variable은 stable 이름·owner·scope·presence contract만 migration하고 실제 값·secret을 저장하지 않는다.
- unknown key가 있는 파일은 오타인지 미래 version인지 확인되기 전 migration하지 않는다.
- `personal_auto` 선택과 secret reference는 migration으로 임의 생성하지 않는다.

## 8단계 범용 Project migration

### 0단계 자체 DB와의 경계

0단계는 Star-Control 자체 global/project management store가 제품 update를 견디기 위한 최소 `plan_migration`, consistent backup, side-by-side generation, integrity, rollback과 read-only recovery를 `star-state`에 둔다. 8단계는 이 내부 구현을 대체하지 않고, 서로 다른 대상 프로젝트가 선언한 migration framework를 공통 Profile로 계획·검증하는 범용 계층을 완성한다.

| 항목 | 0단계 자체 DB | 8단계 범용 Project |
|---|---|---|
| version | `management_store_version` | target별 `MigrationVersionVector` axis |
| chain source | `specs/compatibility.toml`과 private migration source | `.star-control/migrations.toml` 목표 manifest |
| adapter | `star-state` private backend adapter | registered Project ToolDescriptor |
| write owner | Controller가 주입한 state repository | Controller M8 use case가 승인된 target adapter 호출 |
| multi-target | internal active-set generation vector | 한 plan은 한 Project·한 target; 여러 Project는 9단계 handoff |

양쪽이 같은 migration ID namespace나 DB row를 공유할 필요는 없다. 공통된 것은 version source, dry-run, backup proof, deterministic chain, invariant, attempt, M3 Gate와 approval 규칙이다.

### version vector와 chain

범용 migration은 product version 하나가 아니라 적용되는 axis만 가진 `MigrationVersionVector`를 사용한다. v1 axis는 `project_data`, `project_config`, `project_database`, `project_state`, `file_format`, `public_contract`, `toolchain_runtime`, `ipc_protocol`, `plugin_format`이다.

- 각 axis는 owner, version scheme, observed version, source ref·fingerprint, coverage와 observation state를 가진다.
- version을 찾지 못하면 `unknown`이며 `0`, `latest`, product version 또는 DB 최신 row로 채우지 않는다.
- 실행 chain은 연속 `from_version -> to_version` edge의 유일한 ordered sequence여야 한다.
- gap·cycle·중복 edge·ambiguous branch는 migration plan을 차단한다.
- direct skip step도 검증된 내부 연속 chain을 plan에 펼쳐야 한다.
- step/tool/invariant definition이 바뀌면 기존 dry-run·approval·checkpoint는 stale다.

### workflow와 상태

범용 workflow는 `dry_run -> backup_create -> backup_verify -> restore_rehearsal -> migration_rehearsal -> pre_execute_gate -> execute/resume -> validate -> activate -> post_execute_gate` 순서다. destructive migration은 앞 phase를 생략할 수 없다.

상태는 mutable row 한 칸이 아니라 immutable `MigrationAttempt`, `MigrationCheckpoint`, `MigrationValidationReport`, `RestoreVerificationRecord`와 Gate에서 계산한다.

| 상태 | versioning 의미 |
|---|---|
| `succeeded` | target vector 도달과 required invariant·consumer Gate 통과 |
| `partially_succeeded` | chain prefix는 durable하지만 target vector/Gate 미도달 |
| `failed` | outcome은 알려졌지만 step·invariant·Gate 실패 |
| `outcome_unknown` | effect commit 여부를 판정할 수 없음 |
| `rolled_back` | before-compatible vector와 post-rollback Gate 통과 |
| `rollback_failed` | rollback 또는 복귀 검증 실패 |

partial/outcome unknown 상태에서 target version header만 올리거나 current pointer를 새 candidate로 바꾸지 않는다. resume는 actual target이 checkpoint before 또는 expected after와 일치하는지 재관찰한 뒤에만 가능하다.

### backup claim과 restore claim

`created_unverified`, `integrity_verified`, `restore_rehearsed`, `restore_validated`를 서로 다른 상태로 유지한다. backup byte와 checksum이 있더라도 restore tool, 새 target, structural invariant와 required behavior Check를 실제 수행하지 않았으면 “검증된 restore”가 아니다.

destructive step은 최소 `integrity_verified` backup과 `restore_rehearsed` evidence를 요구하고 project policy가 더 강하면 `restore_validated`를 요구한다. consistent backup이 불가능하거나 unknown field loss를 피할 수 없으면 exact loss scope·irreversible boundary를 승인받기 전 live write를 금지한다.

### version 축별 규칙

- config migration은 original byte backup, EffectiveConfig provenance diff, unknown key 보존과 permission 확대 거부를 유지한다.
- persisted state document는 각 `schema_id/schema_version`별 reader·writer 범위를 사용하며 management DB version으로 추측하지 않는다.
- project DB logical version과 concrete DB engine file format을 분리한다. Star-Control core는 SQL·connection string을 해석하지 않는다.
- IPC는 handshake negotiation을 먼저 끝내며 DB migration 성공을 protocol compatibility로 사용하지 않는다.
- Plugin·Catalog는 manifest/descriptor version을 독립적으로 판정하고 config migration이 source를 다시 쓰지 않는다.
- 한 release가 여러 axis를 바꾸면 dependency order와 각각의 rollback/compatibility window를 명시한다.

### M8 evidence 계약 version 전이

M8 top-level 12개 계약은 모두 `schema_version=1` 목표다. M3 v2 target에 M8 phase와 ref를 추가할 때는 다음을 `schema_version=3` 목표로 올린다.

- `star.validation-plan`: `migration_pre_execute|migration_post_execute|migration_post_rollback|performance_compare|language_cutover` phase
- `star.validation-run`: 확장된 `EvidenceSubjectBinding`
- `star.gate-decision`: M8 phase·domain result evaluation refs
- `star.evidence-bundle`: migration/performance/language/handoff refs와 새 subject role
- `star.review-pack`: 기존 stable section 안의 M8 typed summary. section order는 유지 가능하면 v3에서 field만 확장

v2 historical evidence는 source·plan·tool·environment가 같더라도 M8 result ref와 phase가 없으므로 current M8 pre/post/cutover Gate로 자동 승격하지 않는다. raw run·ArtifactRef는 provenance가 맞으면 새 v3 evaluation의 input ref가 될 수 있지만 새 subject probe, result document와 GateDecision을 만들어야 한다.

### 9단계 ChangeBundle 계약 version 전이

[9단계 정본](cross-repo-change-bundle.md)의 새 top-level 계약 9개는 모두 `schema_version=1` 목표다.

- `star.multi-project-goal`
- `star.cross-repo-change-bundle`
- `star.change-bundle-participant`
- `star.worktree-record`
- `star.merge-queue-record`
- `star.merge-conflict-record`
- `star.project-merge-result`
- `star.remote-operation-record`
- `star.change-bundle-release-handoff`

기존 계약은 다음 target version으로 올린다.

| 계약 | target | 추가 의미 |
|---|---:|---|
| `star.merge-plan` | v2 | project/repository, integration worktree, queue·permission·stale fingerprint |
| `star.remote-state-snapshot` | v2 | adapter descriptor, exact commit/PR/check/release subject, completeness·valid_until |
| `star.validation-plan` | v4 | `change_bundle_prepare\|change_bundle_goal_exit` phase와 bundle subject |
| `star.validation-run` | v4 | ChangeBundle/worktree/merge/remote EvidenceSubjectBinding |
| `star.gate-decision` | v4 | `change_bundle` scope·participant binding set |
| `star.evidence-bundle` | v4 | participant/merge/remote/release handoff ref·subject role |
| `star.review-pack` | v4 | 기존 9개 section 안의 project local/remote/partial typed summary; section order 유지 |

v1 MergePlan은 owning Project·repository·target base를 exact하게 표현하지 못하므로 current P6 queue input으로 자동 승격하지 않는다. v1 RemoteStateSnapshot은 adapter·commit subject·completeness/valid-until이 부족하므로 remote write precondition이나 current merge evidence가 아니다.

v2/v3 historical validation evidence는 project-local provenance가 맞으면 history ref로 보존할 수 있지만 ChangeBundle/participant/merge/remote binding과 v4 phase가 없으므로 current `change_bundle_prepare|change_bundle_goal_exit` Gate positive evidence가 아니다. 새 current probe·project Gate·v4 GateDecision을 만든다.

CrossProjectMigrationHandoff v1을 CrossRepoChangeBundle v1로 변환하는 migration은 없다. 9단계 application service가 current Project/Checkout/base/dirty/PatchSet/Gate/recovery를 다시 관찰해 새 bundle을 생성한다. approval token, success state와 remote status를 복사하지 않는다.

### 10단계 Release·Evaluation 계약 version 전이

[10단계 정본](ci-release-evaluation-and-product-completion.md)은 기존 top-level 계약 두 개를 `schema_version=2` 목표로 올린다.

| 계약 | target | 추가 의미 |
|---|---:|---|
| `star.release-manifest` | v2 | Task/source/config/Catalog/Tool/Profile binding, verification layer, build invocation, artifact set digest, metadata·supply-chain applicability, install lifecycle, ready/approved/published와 role별 remote action proof·rollback |
| `star.evaluation-run` | v2 | Rule·Check·Profile·Recipe subject, cli/Codex context, case protocol·adjudication, finding/FP/flaky/suppression·rework·duration·verified cost, comparability·protected metric·Radar |
| `star.validation-plan` | v5 | release 8 phase와 ReleaseManifest/artifact subject |
| `star.validation-run` | v5 | release layer·phase·environment·artifact digest binding |
| `star.gate-decision` | v5 | release readiness·publish preflight/verify scope |
| `star.evidence-bundle` | v5 | artifact entry·file manifest·metadata/license·supply-chain·install lifecycle·approval·remote before/after ref |
| `star.review-pack` | v5 | 기존 stable section 안의 release/evaluation typed subsection; section order 유지 |

ReleaseManifest v1은 artifact set digest, resolved Profile·environment binding, `approved` 상태와 action/target별 remote before/after proof가 부족하다. 따라서 v1의 `ready|published`를 v2 current 상태로 자동 migration하지 않는다.

1. 원본 v1 byte와 당시 ArtifactRef·Gate·remote response를 historical evidence로 보존한다.
2. source와 final artifact byte가 현재 접근 가능하면 SHA-256을 다시 계산하고 current config·Catalog·Tool·Profile·environment를 probe한다.
3. 새 v2 `draft|candidate` revision을 만들고 release phase v5 Gate를 새로 실행한다.
4. v1 approval token, adapter success와 `published` label을 복사하지 않는다.
5. exact provider after snapshot이 없으면 historical publish state는 `unverified`로 표시한다.

EvaluationRun v1은 Rule/Check/Recipe 단위, evaluation context, adjudication denominator와 protected metric이 부족하다. v1 case result는 provenance가 확인되면 v2 case input ref로 사용할 수 있지만 false positive·actual defect·cost를 추정해 채우지 않는다. 새 case protocol과 adjudication 없이 recommendation을 v2 `accept`로 승격하지 않는다.

Rule·Check·Profile·Recipe Catalog item은 다음 lifecycle migration 규칙을 사용한다.

- `active -> deprecated -> retired`; trial candidate는 `rejected` tombstone을 가질 수 있다.
- deprecated에는 replacement ID/version, compatibility window, migration guide·owner·deadline이 필수다.
- retired item은 새 plan에서 resolve하지 않지만 archived CatalogSnapshot과 historical Recipe recovery에는 exact byte를 유지한다.
- Rule baseline·suppression·Finding, Check scope/output, Profile closure·permission·Gate와 Recipe partial recovery를 새 item으로 자동 재해석하지 않는다.
- stable ID·version 의미와 rejected/retired ID를 재사용하지 않는다.
- protected validator coverage가 줄면 lifecycle migration 자체를 B03·release Gate가 block한다.

제품 binary rollback과 ReleaseManifest revision rollback은 data downgrade가 아니다. 이전 binary가 current management store·config를 읽지 못하면 compatible pre-update generation으로 pointer를 전환하거나 read-only recovery를 제공하고, current user data를 삭제·손실 변환하지 않는다.

### 11단계 Rust style evidence 계약 version 전이

[M11 Rust 코드 스타일 자동 교정](../features/rust-code-style-auto-fix.md)은 새 top-level run 계약을 만들지 않고 기존 M4/M3 문서에 nested binding을 추가한다. P9 final writer는 다음 target version을 사용한다.

| 계약 | target | 추가 의미 |
|---|---:|---|
| `star.validation-plan` | v6 | `rust_style_candidate` phase, package/workspace scope와 required coverage expectation |
| `star.validation-run` | v6 | RustToolchainBinding·policy·coverage cell·normalizer/step subject binding |
| `star.gate-decision` | v6 | candidate/pre/post Rust binding, complete coverage·hunk·side-effect·replay와 exact auto policy reason |
| `star.evidence-bundle` | v6 | RustToolchainBinding·RustStylePolicySnapshot·RustStyleCoverageMatrix·RustStyleStepExecution EvidenceRefSet |
| `star.review-pack` | v6 | 기존 stable section 안의 toolchain/config/coverage/suggestion/diff/approval/recovery typed subsection |

`RecipeExecution` v1, `PatchSet` v2와 `PatchApplication` v1은 새 Rust 전용 top-level document로 교체하지 않는다. existing transformer/output artifact, recipe execution, validation plan, operation/evidence ref를 사용하고 v6 EvidenceSubjectBinding이 exact Rust nested fingerprint를 연결한다. 이 existing ref로 표현할 수 없는 field가 실제 implementation fixture에서 확인될 때만 해당 contract version을 별도 변경하며, M11을 이유로 mutable `RustStyleRun`을 만들지 않는다.

v5 release evidence는 source/artifact release history로 보존할 수 있지만 toolchain/style policy/coverage/step binding이 없으므로 M11 candidate·auto apply 또는 final 16 Profile P9 Gate의 positive evidence가 아니다. migration은 값을 추정해 채우지 않는다.

1. historical v5 byte·Gate·artifact ref를 그대로 보존한다.
2. current Project/Checkout/source/config/Catalog/Tool을 다시 resolve한다.
3. `rust_style_v1` inspect/check/prepare가 필요한 phase를 새 실행 ID로 수행한다.
4. v6 candidate/pre/post Gate와 EvidenceBundle을 새로 만든다.
5. 과거 cargo/rustfmt/Clippy exit 0, Star-Control 자체 `--all-features` run과 DB row를 complete coverage·allowlist·approval로 승격하지 않는다.

Rust source/config migration도 M11이 수행하지 않는다. `rust-toolchain.toml`, rustfmt/Clippy config, Cargo lint level, edition/MSRV와 dependency 변경은 별도 ChangePlan/Profile이 소유하며 M11 DB migration이 파일을 생성·수정하지 않는다.

### M8 migration ID

- internal store: `migrate.star.<contract-or-store>.vN-to-vM`
- project manifest step: project namespace를 가진 `migrate.<project-or-domain>.<target>.vN-to-vM`
- config/source PatchSet과 live data attempt는 서로 다른 ID·approval·evidence를 가진다.
- 여러 Project를 한 ID의 atomic migration으로 가장하지 않는다. 9단계 ChangeBundle이 participant ID와 compensation을 별도로 조정한다.

## Fixture와 검증

각 contract version은 다음 fixture를 가진다.

- 최소 valid
- 모든 optional field를 포함한 valid
- type·range·reference가 잘못된 invalid
- 직전 지원 version
- 현재 지원하는 가장 오래된 version
- unknown future version의 opaque sample
- migration 전·후 golden pair
- Managed Registry candidate→active, active→deprecated→removed, bounded alias와 영구 tombstone golden pair
- duplicate ID·namespace collision·ID reuse·consumer 미전환·stale derived Index invalid sample
- 6단계 explicit baseline/current와 API·CLI·Schema·file format·config·error code kind별 unchanged/compatible/additive/breaking/unknown golden pair
- baseline 부재·mutable ref, partial consumer, migration guide/companion change 누락과 의도치 않은 public expansion invalid sample
- docs command/link/anchor/snippet/config example/generated provenance, ConfigKeyTrace와 assumption drift fixture
- Windows drive·UNC·case collision·encoding·line-ending·path-length, redacted EnvironmentSnapshot과 clean-room readiness fixture
- doctor network/install/system mutation 요청 거부와 secret/environment value 비보존 fixture
- 중단된 migration과 rollback sample
- M8 ProjectMigrationManifest version source, chain gap·ambiguity·cycle, unknown preservation과 destructive approval sample
- M8 backup created/integrity verified/restore rehearsed/restore validated의 구분과 false restore claim invalid sample
- M8 checkpoint before/expected-after/diverged, partial/outcome unknown/resume/rollback state projection sample
- PerformanceWorkloadSpec old/future version, cohort revision 혼합·comparison protocol drift invalid sample
- LanguageMigrationPlan·EquivalenceReport compile-only/partial/platform-unverified와 compatibility window fixture
- M3 v2 historical evidence에서 M8 v3 current Gate 자동 승격 거부 sample
- M9 provider compatibility open → consumer transition → provider close DAG, relation unknown·cycle과 optional/required participant fixture
- M9 complete dirty manifest·user preexisting change 보존, worktree ownership mismatch와 file/symbol/contract/generated/lockfile overlap fixture
- M9 project별 apply/validation/merge의 partial·rollback required·held·outcome unknown·resume/compensation state projection sample
- MergePlan v1·RemoteStateSnapshot v1과 v2/v3 evidence를 current M9 queue·remote operation·Goal Gate로 자동 승격하지 않는 invalid sample
- push·PR·merge·publish approval 분리, stale/partial before snapshot, provider success response 뒤 missing/mismatched after snapshot fixture
- ChangeBundleReleaseHandoff의 project commit·artifact subject·Gate mismatch와 compatibility window remaining-risk fixture
- ReleaseManifest v1 ready/published를 v2 ready/approved/published로 자동 승격하지 않는 fixture
- artifact set digest·signed final byte·included file manifest와 source/config/Profile/environment mismatch fixture
- EvaluationRun v1의 missing context·adjudication·denominator·protected metric을 0이나 accept로 채우지 않는 fixture
- Rule·Check·Profile·Recipe active→deprecated→retired, rejected tombstone, replacement migration과 historical recovery fixture
- update 뒤 binary rollback과 store/config downgrade 거부·user data preservation fixture

검증은 schema generation drift, fixture round-trip, migration determinism, unknown 보존, event replay, checkpoint reconciliation, backup/restore claim 분리와 downgrade 거부를 포함한다.

관리 DB fixture는 추가로 clean open, double-writer 거부, unclean shutdown, scan generation crash, future version recovery-only, corrupt backup restore, missing/tampered/incompatible backup-set 거부, side-by-side rebuild, verified/rejected ArtifactRef reindex, local-only state loss report, redacted export/import binding·conflict, candidate 작성/activation 전후 crash, active-set all-old/all-new, apply receipt replay와 backend conformance를 포함한다. 모든 손상 fixture는 disposable temp management root와 project만 사용한다.

## Downgrade와 rollback

제품 binary rollback과 데이터 downgrade는 같은 일이 아니다. 이전 binary가 현재 store를 지원하지 않으면 자동으로 열지 않는다.

- update 전 호환 가능한 backup 또는 이전 format store를 유지한다.
- rollback 시 이전 binary가 마지막으로 쓸 수 있었던 store pointer로 돌아간다.
- 새 version에서 생긴 Goal은 export할 수 있지만 의미 손실이 있는 downgrade는 하지 않는다.
- rollback 뒤 양쪽 store에 쓰지 않으며 사용자가 어느 쪽을 계속 사용할지 명확히 선택한다.
