# Version과 Migration 계약

## 목적

제품 version, 저장 자료, 설정, Catalog와 통신 protocol은 바뀌는 속도와 호환 범위가 다르다. 하나의 version 숫자로 모두 묶지 않고 각 경계를 독립적으로 판정한다.

## Version 축

| 대상 | 형식 | 증가 기준 |
|---|---|---|
| Star-Control 제품 | SemVer | 사용자 기능·호환성·수정 release |
| 개별 데이터 계약 | positive integer `schema_version` | 직렬화 shape 또는 의미가 바뀔 때 |
| 설정 | `star.config`의 독립 schema version | key·type·병합 의미가 바뀔 때 |
| 상태 저장소 | store format integer | journal·index·snapshot 저장 방식이 바뀔 때 |
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

Event history를 in-place로 다시 쓰지 않는다. 과거 payload decoder를 유지하거나 검증된 변환 copy와 원본 hash를 함께 보관한다.

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

## Downgrade와 rollback

제품 binary rollback과 데이터 downgrade는 같은 일이 아니다. 이전 binary가 현재 store를 지원하지 않으면 자동으로 열지 않는다.

- update 전 호환 가능한 backup 또는 이전 format store를 유지한다.
- rollback 시 이전 binary가 마지막으로 쓸 수 있었던 store pointer로 돌아간다.
- 새 version에서 생긴 Goal은 export할 수 있지만 의미 손실이 있는 downgrade는 하지 않는다.
- rollback 뒤 양쪽 store에 쓰지 않으며 사용자가 어느 쪽을 계속 사용할지 명확히 선택한다.
