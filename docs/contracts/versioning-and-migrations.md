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
| optional field 추가, enum 값 추가, validation 완화 | schema version 증가, compatibility table에 older reader 동작 명시 |
| 필수 field 추가, type·기본 의미 변경, enum 제거 | schema version 증가와 migration 필요 |
| ID 의미 변경, secret·permission 경계 변경 | 새 계약 ID 또는 명시적 breaking migration |
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
| 지원 과거 version | `migration_required` | status, migration plan, backup |
| 읽을 수 있는 미래 version | `read_only_recovery` | 비민감 metadata·export, backup 선택 |
| suspect | `read_only_recovery` | integrity·backup·side-by-side rebuild |
| corrupt | `quarantined` | 원본 보존, verified backup restore만 |

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

`migrate.star.project.v1-to-v2`는 [Project Catalog·Code Index 계약](project-catalog-and-code-index.md)의 `ProjectCheckout` 도입을 위한 **목표 migration 계약**이다. 현재 code·Schema·DB migration이 구현됐다는 뜻이 아니다. Project/root-binding cardinality와 global/project reference 의미를 바꾸므로 `lossless_local_state`이며 `management.auto_migrate_rebuildable` 대상이 아니다.

dry-run plan은 expected global/project `StoreVersionVector`, source `management_store_version`, 정렬된 ProjectId, 각 v1 `root_binding_id`, immutable `checkout_id_allocations`, 변환할 ProjectRef·event/projection count, invalidation 대상 scan generation, candidate binding envelope 목록, backup-set 위치, 예상 공간과 rollback active-set을 가진다. CheckoutId는 dry-run에서 한 번 발급해 plan fingerprint에 넣고 apply·retry가 같은 allocation을 재사용한다. apply 중 새 ID를 다시 뽑지 않는다.

변환은 다음 순서다.

1. Writer lease를 잡고 scan·project attachment mutation을 quiesce한 뒤 expected StoreVersionVector를 다시 확인한다.
2. global store, 영향받는 모든 project store와 active-set의 consistent backup-set을 만들고 byte·header hash를 검증한다.
3. v1 binding을 current-user context에서 열어 ProjectId와 final filesystem identity를 검증한다. plaintext path·그 hash는 plan, event, DB와 backup-set에 넣지 않는다.
4. attached v1 Project마다 plan에 고정한 CheckoutId로 `ProjectCheckout` 하나를 candidate global generation에 만든다. detached Project는 checkout을 합성하지 않는다.
5. protected binding store에는 같은 `root_binding_id`를 가리키되 ProjectId·CheckoutId를 가진 v2 envelope를 atomic candidate로 만든다. v1 envelope는 active-set 전환 전 덮어쓰거나 삭제하지 않는다.
6. Project v2에서 `root_binding_id`를 제거하고 정렬된 `attached_checkout_ids`와 derived registration state를 쓴다. 같은 binding이 여러 ProjectId에 연결되거나 manifest identity가 다르면 전체 migration을 block한다.
7. active Goal·Context의 ProjectRef v1은 exact ProjectId와 유일한 matching checkout을 증명할 수 있을 때만 v2 `checkout_id`로 바꾼다. 0개·2개 이상이면 해당 run을 임의 변환하지 않고 migration을 block한다.
8. 기존 P0 ScanRun·Symbol·Reference·Finding record는 역사 evidence로 보존할 수 있지만 CodeIndexSnapshot으로 승격하지 않는다. 새 index partition은 `unavailable`이며 checkout current probe와 첫 full scan 뒤에만 current가 된다.
9. candidate global/project relation, ProjectId partition, binding envelope, event/projection revision, redaction과 ArtifactRef integrity를 검증한다.
10. candidate generation과 binding set 전체가 통과한 뒤 하나의 새 active-set pointer를 atomic replace한다. startup smoke가 실패하면 이전 active-set과 v1 binding envelope로 rollback한다.

dry-run과 apply 결과는 migrated/attached/detached/blocked Project count, allocation fingerprint, preserved legacy scan count, required full-scan ProjectId와 loss report를 가진다. partial candidate는 active가 아니며 retention 전 보존한다. migration 성공만으로 ProjectCatalogSnapshot·CodeIndexSnapshot을 만들거나 freshness를 current로 표시하지 않는다.

### backup·교체·rollback

- migration·repair 전 consistent backup과 byte hash manifest를 만든다.
- backend가 전체 migration transaction을 보장하지 않으면 새 store generation을 만들고 변환·검증한 뒤 active pointer를 atomic replace한다.
- 새 generation은 relation·partition·fingerprint·event/projection·ArtifactRef integrity를 통과해야 한다.
- 교체 뒤 startup 검사에 실패하면 이전 active generation으로 돌아가고 실패 evidence를 남긴다.
- backup, 손상 store와 이전 generation은 retention plan·permission 전 삭제하지 않는다.

### rebuild

관리 DB가 없거나 복구할 수 없으면 attached Project root, Git 선언·source, Catalog와 검증된 `.ai-runs` manifest에서 새 generation을 만든다.

- source-derived ProjectRevision, WorkspaceSnapshot, Symbol, Reference, Finding은 새 scan으로 계산한다.
- shared Suppression·Baseline은 Git 선언에서 import한다.
- `.ai-runs`의 canonical ValidationResult·GateDecision·ArtifactRef는 hash를 확인한 뒤 reindex할 수 있다.
- local-only Disposition, local Suppression, 과거 idempotency와 actor event는 backup·export가 없으면 복구하지 못했다고 명시한다.
- rebuild는 과거 ScanRunId·timestamp를 재생성하지 않고 `reconstructed_from`과 completeness를 기록한다.

### backend와 dependency gate

public 계약은 특정 DB를 선택하지 않는다. P0 private adapter의 concrete 선택은 [ADR-0008](../decisions/ADR-0008-P0-embedded-relational-backend.md)에 따라 `star-state` 내부의 `rusqlite 0.40.1` bundled backend로 확정됐다. 선택 전에 다음 항목을 비교했고, dependency를 추가한 뒤에도 같은 항목을 release gate로 검사한다.

- Windows x64·ARM64 지원과 process crash 내구성
- single-writer transaction, consistent backup와 integrity 검사
- side-by-side migration·read-only open 가능성
- license, 보안 update, binary 크기와 유지보수 상태
- Rust adapter의 오류·cancellation·threading 경계

선정 결과는 `star-state` private adapter에만 반영한다. StarConfig, CLI, MCP와 persisted domain contract에는 backend 이름, SQL, pragma, connection string이나 DB filename을 추가하지 않는다. backend 교체는 repository conformance를 다시 통과해야 하지만 public document migration을 요구해서는 안 된다.

## Migration 절차

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
- unknown key가 있는 파일은 오타인지 미래 version인지 확인되기 전 migration하지 않는다.
- `personal_auto` 선택과 secret reference는 migration으로 임의 생성하지 않는다.

## Fixture와 검증

각 contract version은 다음 fixture를 가진다.

- 최소 valid
- 모든 optional field를 포함한 valid
- type·range·reference가 잘못된 invalid
- 직전 지원 version
- 현재 지원하는 가장 오래된 version
- unknown future version의 opaque sample
- migration 전·후 golden pair
- 중단된 migration과 rollback sample

검증은 schema generation drift, fixture round-trip, migration determinism, unknown 보존, event replay와 downgrade 거부를 포함한다.

관리 DB fixture는 추가로 clean open, double-writer 거부, unclean shutdown, scan generation crash, future version read-only, corrupt backup restore, side-by-side rebuild, local-only state loss report, global/project partial migration, active-set atomic switch, incompatible backup-set 거부와 backend conformance를 포함한다.

## Downgrade와 rollback

제품 binary rollback과 데이터 downgrade는 같은 일이 아니다. 이전 binary가 현재 store를 지원하지 않으면 자동으로 열지 않는다.

- update 전 호환 가능한 backup 또는 이전 format store를 유지한다.
- rollback 시 이전 binary가 마지막으로 쓸 수 있었던 store pointer로 돌아간다.
- 새 version에서 생긴 Goal은 export할 수 있지만 의미 손실이 있는 downgrade는 하지 않는다.
- rollback 뒤 양쪽 store에 쓰지 않으며 사용자가 어느 쪽을 계속 사용할지 명확히 선택한다.
