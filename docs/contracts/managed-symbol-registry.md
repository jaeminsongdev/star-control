# 관리형 Symbol·상수·에러 코드 Registry 계약

## 상태와 문서 소유권

이 문서는 Star-Control 5단계인 **관리형 Symbol·상수·에러 코드 Registry**의 설계 정본이다. 현재 상태는 **설계 확정, 제품 구현 전**이다. 이 문서를 추가했다고 Registry manifest, generator, codemod, DB Schema, migration, CLI 또는 실제 상수·오류 코드 변경이 구현된 것은 아니다.

이 문서에서 **Managed Registry**는 여러 Project·언어·문서가 공유하는 계약 값을 관리하는 source 계약이다. 다음 세 Registry와 다른 domain이다.

| 이름 | 소유 대상 | 정본 |
|---|---|---|
| Managed Registry | error code, Schema ID, config key처럼 source와 consumer가 공유하는 계약 값 | 이 문서와 Git manifest |
| descriptor Catalog·Validator Registry | Task·Rule·Check·Profile·Gate 실행 metadata | [설정과 Catalog 계약](config-and-catalog.md) |
| live Tool Registry | 외부 EXE package·ToolDescriptor·실행 identity | [외부 Tool Registry](external-tool-registry.md) |

책임은 다음처럼 나눈다.

| 책임 | 정본 |
|---|---|
| 세 관리 분류, manifest·declaration·binding·consumer·lifecycle·alias 의미 | 이 문서 |
| Project·Symbol·Reference·candidate entity와 freshness | [Project Catalog·Code Index 계약](project-catalog-and-code-index.md) |
| TaskSpec·scope·consumer 영향·affected Check 선택 | [변경 계획·영향 분석 계약](change-planning-and-impact.md) |
| `managed_declaration` selector, dry-run·PatchSet·apply·복구 | [안전한 Patch·Refactor·codemod 엔진 계약](safe-patch-and-codemod.md) |
| pre/post Gate, Diagnostic·EvidenceBundle | [공통 검증·품질 Gate](../features/common-validation-gate.md), [검사·완료·증거](validation-and-evidence.md) |
| ErrorEnvelope·error code·display message·CLI exit code | [오류와 진단 계약](errors-and-diagnostics.md) |
| config key·default·Catalog 경계 | [설정과 Catalog 계약](config-and-catalog.md) |
| schema·manifest·consumer version과 migration | [Version과 Migration 계약](versioning-and-migrations.md) |
| source·generated output·Package·single writer 위치 | [Repository·Package 구조](../architecture/repository-layout.md) |

이 문서는 기존 error·config·Catalog 계약을 복제하지 않는다. Managed Registry는 그 계약의 **stable identity, ownership, binding과 lifecycle**만 소유한다.

## 목표와 제외 범위

### 목표

1. 중앙 관리할 계약 값과 검색만 할 지역 값을 기계적으로 구분한다.
2. Git manifest를 유일한 공유 정본으로 두고 DB를 재구축 가능한 derived Index로 유지한다.
3. stable ID, namespace, owner, type, value role과 source manifest를 한 선언에서 고정한다.
4. language symbol, schema, 문서와 generated output이 한 declaration에 어떻게 bind되는지 표현한다.
5. `active`, `deprecated`, `reserved`, `removed`, alias와 consumer migration 기간을 재현 가능하게 관리한다.
6. error code를 첫 수직 Slice로 구현할 수 있을 만큼 변경·deprecation·removal lifecycle을 고정한다.
7. Registry 변경이 M1 resolution, M2 impact, M4 dry-run PatchSet과 M3 pre/post Gate를 반드시 지나게 한다.
8. 6단계 contract·docs·environment drift 검사가 사용할 source→binding→consumer 관계와 판정 code를 제공한다.

### 제외 범위

- DB row 또는 검색 결과를 source-of-truth로 삼는 기능
- DB row를 기준으로 source 문자열을 무조건 치환하는 기능
- 모든 literal·숫자·문자열을 중앙 관리하는 기능
- local implementation constant를 강제로 config key로 바꾸는 기능
- 같은 값이라는 이유로 서로 다른 의미·owner의 상수를 합치는 기능
- removed ID·public value를 다른 의미로 재사용하는 기능
- generated source를 직접 편집하는 기능
- manifest 변경과 consumer rewrite를 dry-run 없이 즉시 적용하는 기능
- 한 PatchSet에서 둘 이상의 Project source를 바꾸는 기능
- cross-repo apply·merge·commit·push를 5단계 완료로 주장하는 기능
- browser UI, AI·LLM 판단, OpenAI API 또는 별도 rewrite engine

cross-project 영향 계산은 read-only로 허용한다. 여러 Project를 하나의 operation으로 실제 적용·병합하는 기능은 사용자 단계 번호 **9단계**인 [P7 여러 프로젝트와 원격 저장소](../roadmap/final-implementation.md#p7-여러-프로젝트와-원격-저장소)의 coordination 계약 전에는 지원하지 않는다.

## 세 관리 분류

Registry 분류 enum은 정확히 다음 세 값이다.

| 분류 | 의미 | 공유 정본 | 허용 동작 |
|---|---|---|---|
| `managed_declaration` | 여러 Project·언어·문서에서 공유하며 사용자가 중앙 관리 대상으로 승인한 계약 값 | 유효한 Git Managed Registry manifest entry | lifecycle·binding·consumer 관리, M2→M4→M3 변경 |
| `candidate` | scanner가 error code·Schema ID·config key·constant 후보로 발견했지만 아직 중앙 관리 대상으로 승인되지 않은 값 | current source와 CodeIndexSnapshot의 관찰 evidence | 검색·비교·review·promotion 제안만 허용 |
| `local_implementation_constant` | 한 구현 경계 안에서만 의미가 있고 Registry가 소유하지 않는 지역 상수 | current source와 명시적 source ownership/classification | 검색·영향 분석은 허용, Registry change target·codegen은 금지 |

이 분류는 M1의 evidence 상태 `declared|inferred|candidate`와 다른 축이다. 예를 들어 syntax로 명확히 선언된 private constant는 index evidence가 `declared`이면서 Registry 분류는 `local_implementation_constant`일 수 있다. 반대로 text-only error code 후보는 Registry 분류 `candidate`, evidence 상태 `candidate`다.

### 분류 결정 순서

1. current·valid manifest의 exact declaration mapping이 있으면 `managed_declaration`이다.
2. manifest mapping은 없지만 reviewed source ownership rule 또는 current visibility·scope evidence가 한 구현 경계로 제한하면 `local_implementation_constant`다.
3. 위 둘을 증명하지 못하고 지원 대상 pattern과 일치하면 `candidate`다.
4. evidence가 충돌하거나 stale·partial이면 local로 낮추지 않고 `candidate + unverified`로 남긴다.
5. 같은 literal occurrence를 하나의 분류로 묶지 않고 ProjectId·owning Symbol·Contract·source anchor별로 판정한다.

### 분류 전이

- `candidate -> managed_declaration`: 사용자가 owner·namespace·type·lifecycle을 승인한 manifest ChangePlan과 PatchSet이 post Gate를 통과한 뒤에만 가능하다.
- `candidate -> local_implementation_constant`: reviewed source ownership/classification 변경 또는 exact private/local evidence가 필요하다. local Disposition만 있으면 공유 정본이 아니라 local 판단이다.
- `local_implementation_constant -> managed_declaration`: 새 manifest entry와 consumer impact review가 필요하다. 기존 DB 분류를 승격하지 않는다.
- `managed_declaration`은 manifest entry를 삭제해 local 또는 candidate로 강등하지 않는다. 더 이상 사용하지 않으면 `deprecated -> removed` lifecycle과 tombstone을 유지한다.

## 관리 대상 지원 순서

구현·onboarding 순서는 다음과 같이 고정한다. 앞 Slice의 lifecycle·fixture·Gate가 통과하기 전 뒤 대상을 자동 관리 대상으로 넓히지 않는다.

| 순서 | kind | 첫 지원 범위 |
|---:|---|---|
| 1 | `error_code`, `diagnostic_id` | ErrorEnvelope code와 stable Diagnostic Rule ID. 5단계 첫 수직 Slice |
| 2 | `schema_id`, `schema_version` | 직렬화·IDL·Schema identity와 지원 version |
| 3 | `config_key`, `config_default` | fully qualified key와 typed 제품 기본값 |
| 4 | `cli_command`, `cli_exit_code` | command path·stable 종료 의미 |
| 5 | `event_id`, `capability_id`, `permission_id` | event·capability·Action identity |
| 6 | `feature_flag_id` | flag identity와 lifecycle. 임의 runtime value 저장은 제외 |
| 7 | `format_id`, `resource_id` | 여러 Project가 공유하는 format·resource identity |
| 8 | `global_constant` | 사용자가 owner·consumer·호환성 필요를 명시적으로 승인한 값만 |

지원 순서는 중요도를 나타내지만 candidate 탐지를 금지하지 않는다. 뒤 순서 대상도 M1 검색 결과에는 나올 수 있으나 해당 kind의 conformance가 구현되기 전에는 `managed_declaration`으로 자동 승격하거나 codegen하지 않는다.

## Git 정본과 저장 경계

### source manifest 위치

한 owner Project의 기본 source set은 다음 위치를 사용한다.

```text
<project>/.star-control/managed-registry/
├─ manifest.toml
└─ declarations/
   ├─ errors.toml
   ├─ schemas.toml
   ├─ config.toml
   └─ ... explicit fragment
```

- `manifest.toml`은 root manifest이며 하나의 `registry_id`, owner Project와 namespace claim set을 소유한다.
- fragment는 root의 `declaration_files`에 project-relative path로 명시된 파일만 읽는다. glob, directory 자동 열거, remote include, absolute·UNC·device path와 root escape를 허용하지 않는다.
- fragment 순서는 의미가 없다. declaration과 namespace는 stable ID byte-order로 canonicalize한다.
- source path 이동은 declaration identity를 바꾸지 않지만 root manifest와 source provenance fingerprint를 갱신한다.
- owner Project 밖 파일을 include하지 않는다. consumer Project는 source manifest를 복사해 정본처럼 사용하지 않고 registry·declaration ref를 가진다.

### 세 저장 계층

| 계층 | 저장 내용 | 정본 성격 |
|---|---|---|
| Git manifest·source | root/fragment, language binding source, Schema·docs·generator input | 공유 정본 |
| local management DB | ManagedRegistrySnapshot, declaration/binding/consumer index, candidate/local classification projection | source에서 재구축 가능한 derived Index |
| `.ai-runs` | manifest diff, codegen/codemod preview, compatibility report, Gate·consumer evidence | hash가 있는 실행 evidence |

DB에는 source fragment 전체 byte, generated source byte, raw literal과 다른 Project의 private symbol detail을 복제하지 않는다. CanonicalSourceId, ProjectPathRef, content hash, declaration/binding key와 ArtifactRef만 저장한다.

### source와 DB가 다를 때

1. current source manifest hash와 snapshot input이 같고 required mapping이 complete할 때만 DB snapshot은 `current`다.
2. source가 달라지면 DB snapshot은 즉시 `stale_source`다. 최근 scan 시각, last-known-good 또는 DB row edit로 current를 유지하지 않는다.
3. manifest가 invalid여도 Git byte가 현재 정본이다. 이전 valid snapshot은 historical 조회용일 뿐 current generation·codegen·Gate positive evidence가 아니다.
4. rescan·resolution이 실패하면 이전 snapshot을 `current`로 되돌리지 않고 `invalid|partial|unverified`와 stable reason을 반환한다.
5. DB 손실 시 current Git manifest와 source/index를 다시 읽어 snapshot을 구축한다. 과거 approval·event·Patch evidence는 backup 또는 `.ai-runs`가 없으면 복구하지 못했다고 보고한다.

## ManifestSet wire 계약

### root `ManagedRegistryManifest`

root file의 schema ID는 `star.managed-registry-manifest`, `schema_version=1`이다.

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `registry_id` | 예 | namespace를 포함한 stable ID. path·표시 이름과 분리 |
| `registry_version` | 예 | 전체 declaration set의 SemVer |
| `owner_project_id` | 예 | manifest를 소유한 ProjectId |
| `namespace_claims` | 예 | 정렬된 NamespaceClaim. 빈 set 금지 |
| `declaration_files` | 예 | owner Project 안의 normalized explicit fragment path. 중복 금지 |
| `compatibility_policy_ref` | 예 | alias·consumer·removal floor를 정의한 versioned policy ref |
| `required_check_families` | 예 | 최소 `managed_registry_contract`, `consumer_compatibility`, `generated_consistency`, `docs_contract_drift` |
| `extensions` | 아니요 | 등록된 namespace만 허용. core lifecycle을 덮을 수 없음 |

`registry_id`는 같은 owner·namespace set의 장기 identity다. 다른 의미의 Registry에 재사용하지 않는다. owner Project 이전은 일반 field edit가 아니라 명시적인 ownership transfer와 양쪽 Project evidence가 필요한 breaking migration이며 5단계 cross-repo apply 대상이 아니다.

#### NamespaceClaim

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `namespace` | 예 | lowercase dot-separated exact prefix. wildcard 금지 |
| `owner_project_id` | 예 | 선언·승인 책임 Project. root owner와 다르면 explicit delegation 필요 |
| `allowed_kinds` | 예 | 이 namespace에서 허용한 ManagedDeclaration kind의 non-empty set |
| `delegated_child_namespaces` | 예 | 없으면 empty. child namespace·delegate Project·allowed kind를 명시 |
| `status` | 예 | `active`, `reserved`. 제거된 namespace도 `reserved` tombstone으로 유지 |
| `introduced_in_registry_version` | 예 | claim이 생긴 registry SemVer |
| `transfer_ref` | ownership 이전 시 | 양쪽 owner 승인·compatibility 계획의 immutable ref |

namespace claim은 longest-prefix 우선순위로 owner를 추측하지 않는다. exact claim 또는 parent의 explicit child delegation 하나만 declaration을 소유할 수 있다. 겹치는 claim, delegation cycle, child보다 넓은 kind 위임과 source owner 밖 암묵적 이전은 `REGISTRY_NAMESPACE_COLLISION`이다.

#### RegistryCompatibilityPolicyRef

`compatibility_policy_ref`는 같은 CatalogSnapshot에서 해석되는 versioned policy ID·version·definition fingerprint다. policy는 최소 다음 값을 제공한다.

- kind별 허용 lifecycle edge와 breaking-change version floor
- alias 최대 registry-version 기간과 UTC expiry 사용 여부
- required/optional/observed-only consumer별 freshness·coverage floor
- deprecation·removal에 허용하는 compatibility status set
- unknown·stale·below-minimum consumer의 default Gate 결과
- kind별 public value uniqueness scope와 tombstone retention `permanent`
- pre/post required Check family와 human approval floor

policy가 없거나 CatalogSnapshot과 fingerprint가 다르면 manifest resolution은 `invalid|stale_catalog`이며 built-in default를 추측 적용하지 않는다. policy는 ID 재사용, unbounded alias, generated direct edit, DB source write와 9단계 전 cross-project apply를 허용할 수 없다.

### `ManagedRegistryFragment`

fragment의 schema ID는 `star.managed-registry-fragment`, `schema_version=1`이다.

| 필드 | 필수 | 규칙 |
|---|---:|---|
| `registry_id` | 예 | root와 exact 일치 |
| `namespace` | 예 | root가 claim하거나 명시적으로 delegate한 namespace |
| `declarations` | 예 | 0개 이상 ManagedDeclaration. stable ID 중복 금지 |
| `source_description` | 아니요 | 표시·review용. identity에서 제외 |

fragment는 다른 fragment를 include하지 않는다. 같은 declaration을 여러 fragment에 나누거나 뒤 fragment로 덮어쓰지 않는다. empty fragment는 허용하지만 root가 참조한 file 누락은 manifest invalid다.

## ManagedDeclaration 계약

### 필수 field

| 필드 | 필수 | 의미 |
|---|---:|---|
| `managed_declaration_id` | 예 | 재사용하지 않는 stable ID |
| `item_version` | 예 | 이 declaration 의미의 SemVer |
| `namespace`, `semantic_key` | 예 | owner 경계와 namespace 내부 의미 key |
| `kind` | 예 | 지원 순서 표의 stable enum |
| `owner` | 예 | OwnerRef |
| `value_type` | 예 | scalar kind 또는 local SchemaRef |
| `value_role` | 예 | `stable_identifier`, `config_default`, `compile_time_contract` 중 하나 |
| `primary_value` | active/deprecated/removed, reserved value 할당 시 | type을 통과한 public/contract 값. removed에서는 runtime 값이 아니라 영구 tombstone |
| `description` | 예 | 의미와 사용 경계. 표시 문구 자체가 contract value는 아님 |
| `status` | 예 | `active`, `deprecated`, `reserved`, `removed` |
| `lifecycle` | 예 | introduced/deprecated/removed version, replacement와 migration record |
| `aliases` | 예 | 없으면 empty. AliasRecord array |
| `binding_specs` | 예 | 없으면 empty. 언어·Schema·docs·generated binding 요구 |
| `consumer_contracts` | 예 | 없으면 empty. consumer compatibility 요구 |
| `uniqueness_scope` | 예 | kind별 충돌 판정 범위 |
| `definition_fingerprint` | 생성 시 | source writer가 쓰지 않는 resolved semantic hash |

사람이 편집하는 TOML에는 `definition_fingerprint`를 직접 쓰지 않는다. loader가 선언 의미를 canonicalize해 계산하고 snapshot에 넣는다. source byte SHA-256은 provenance로 별도 보존한다.

`definition_fingerprint` payload는 managed declaration ID·item version, namespace·semantic key·kind, OwnerRef의 stable field, value type/role/primary value, status·LifecycleRecord, AliasRecord, BindingSpec, ConsumerContract와 uniqueness scope를 stable field name·ID byte-order로 canonicalize한 SHA-256이다. `description`, display owner, source path, timestamp, comment와 TOML key order는 payload에서 제외한다. 따라서 display-only 설명 변경은 source hash만 바뀌고 semantic fingerprint·item version은 유지할 수 있다. 반대로 fingerprint payload가 바뀌었는데 item version·registry version 규칙을 만족하지 않으면 manifest invalid다.

### stable ID와 namespace

- `managed_declaration_id`는 lowercase ASCII dot/kebab grammar를 사용한다. 예: `star.managed.error.management-store-unavailable`.
- `managed_declaration_id`는 실제 public value와 다르다. `MANAGEMENT_STORE_UNAVAILABLE`은 `primary_value`다.
- `namespace`는 lowercase dot-separated owner 경계다. 예: `star.error`, `star.schema`, `star.config`.
- exact namespace claim은 한 current registry owner만 가질 수 있다. 상위 namespace와 하위 namespace가 다른 owner라면 root manifest의 explicit delegation이 필요하다.
- 같은 `managed_declaration_id + item_version`에 다른 definition fingerprint가 있으면 source 우선순위로 덮지 않고 conflict다.
- ID, semantic key와 tombstoned public value는 path rename·owner 표시 변경과 무관하게 유지한다.

### OwnerRef

OwnerRef는 다음 field를 가진다.

| 필드 | 필수 | 의미 |
|---|---:|---|
| `project_id` | 예 | source와 승인 책임 Project |
| `contract_id` | contract 값이면 | error/config/Schema/CLI 등 owning contract identity |
| `module_key` | 구현 owner가 있으면 | M1 package/module stable key |
| `approval_policy_ref` | 예 | lifecycle·breaking change 최소 승인 floor |
| `display_owner` | 아니요 | 사람 표시용. identity에서 제외 |

owner는 raw path, 사용자 이름, 자유 형식 담당자 이름만으로 표현하지 않는다. owner 변경은 consumer 영향과 namespace claim을 재평가하는 semantic change다.

### type과 value role

v1 built-in scalar는 `string`, `integer`, `boolean`, `decimal`, `semver`, `identifier`다. object·array·union처럼 구조가 필요한 값은 local `SchemaRef`를 사용하고 Registry entry 하나를 임의 JSON blob 저장소로 만들지 않는다.

| value role | 의미 | 변경 경계 |
|---|---|---|
| `stable_identifier` | error code, Schema ID, config key, event/permission ID처럼 소비자가 identity로 비교 | in-place 의미 변경 금지. 새 declaration + migration |
| `config_default` | user/project override가 없을 때 EffectiveConfig가 선택하는 typed 기본값 | runtime override 가능. key identity와 default value 변경을 별도 impact로 계산 |
| `compile_time_contract` | wire tag, protocol limit, ABI/format marker처럼 build된 producer·consumer가 함께 지켜야 하는 값 | runtime config override 금지. source binding·consumer 호환과 rebuild 검증 필요 |
| `compile_time_contract` | protocol limit·wire discriminant처럼 build된 consumer가 같은 값을 알아야 하는 계약 상수 | 일반 config화 금지. 변경 시 version·consumer compatibility 필수 |

`config_default`를 compile-time constant로 복제하거나 `compile_time_contract`를 사용자 설정으로 노출하지 않는다. 같은 numeric/string value여도 role·owner·semantic key가 다르면 별도 declaration이다.

### kind별 uniqueness

| kind | 기본 uniqueness scope |
|---|---|
| error code·diagnostic ID | 등록된 registry set 전체의 normalized public value |
| Schema·format·resource·event·capability·permission·feature flag ID | fully qualified public ID |
| config key | owning config contract + fully qualified dotted key |
| config default | 해당 config key declaration |
| CLI command | owning executable + normalized command path |
| CLI exit code | owning executable + numeric code + stable meaning |
| global constant | owner namespace + semantic key. raw value equality는 conflict가 아님 |

manifest는 `uniqueness_scope`를 더 좁혀 collision을 숨길 수 없다. kind policy보다 넓은 scope는 허용할 수 있지만 기존 declaration과 compatibility 검사를 다시 수행한다.

## binding·definition·reference·generated output

### 관계 원칙

```text
Git ManagedDeclaration
  -> implementation definition binding
       -> source reference observations
  -> Schema/document binding
  -> generator input binding
       -> generated output manifest
  -> consumer contract
       -> observed consumer version·reference set
```

Git declaration이 contract 정본이다. 언어별 constant·enum·static, Schema member와 문서 표는 **binding**이며 별도의 competing truth가 아니다.

### BindingSpec

| 필드 | 필수 | 의미 |
|---|---:|---|
| `binding_id` | 예 | declaration 안의 stable binding ID |
| `role` | 예 | `definition`, `schema`, `documentation`, `generated_output` |
| `consumer_project_id` | 예 | binding source를 소유한 Project |
| `language_id` | 언어 binding이면 | stable language ID |
| `symbol_name` | symbol binding이면 | expected qualified name. display label과 분리 |
| `target_selector` | 예 | contract/symbol/generator/path owner selector. raw literal 금지 |
| `value_projection` | 예 | primary value·type·name 중 이 binding에 투영할 field |
| `update_mode` | 예 | `codegen`, `codemod`, `manual_review` |
| `generator_ref` | codegen이면 | generator ID·version·input/output contract |
| `required_index_tier` | 예 | text/syntax/semantic/declared minimum |
| `required_check_families` | 예 | binding drift·build·contract·docs 검사 floor |

모든 source reference를 manifest에 하나씩 적지 않는다. `binding_specs`는 expected definition/output owner를 선언하고 actual reference set은 M1 CodeIndexSnapshot에서 파생한다. 각 `ManagedBindingObservation`은 BindingSpec ref, Symbol/CanonicalSource, actual value·type projection, definition/reference role, tier·resolution·freshness와 drift 상태를 가진다.

#### ManagedBindingObservation

| 필드 | 필수 | 의미 |
|---|---:|---|
| `binding_spec_ref` | 예 | declaration ID·item version·binding ID |
| `observation_role` | 예 | `definition`, `reference`, `schema`, `documentation`, `generated_output` |
| `project_id`, `checkout_id` | 예 | 실제 관찰 Project·working copy |
| `symbol_ref`, `canonical_source_ref` | 가능한 만큼 | qualified symbol 또는 ProjectPathRef 기반 source anchor |
| `observed_name`, `observed_type`, `observed_value_projection` | relation별 | expected field와 비교한 typed 관찰. 민감 원문 금지 |
| `generator_execution_ref`, `output_manifest_ref` | generated일 때 | pinned generator와 declared/actual output identity |
| `tier`, `resolution`, `freshness`, `coverage`, `limitations` | 예 | M1 evidence quality |
| `drift_status` | 예 | `RegistryConsistencyRecord.status`와 호환되는 상태 |
| `content_fingerprint` | 예 | timestamp·raw path를 제외한 canonical observation hash |

reference가 여러 개면 observation record를 source anchor별로 정렬하거나 큰 set은 content-addressed EvidenceRefSet으로 외부화한다. count만 저장해 reference 0건을 증명하지 않는다. required scope가 complete해야 0건 판정이 유효하다.

definition binding은 declaration마다 0개 이상일 수 있다. 여러 언어 binding이 허용되지만 같은 language/surface의 authoritative definition이 둘이면 explicit facade/delegation 없이 conflict다. `reference`는 manifest role이 아니라 derived observation이다.

### generated output

- generated binding은 authoritative input으로 ManagedDeclaration ref와 manifest fingerprint를 가진다.
- generator ID·version·executable hash, config/template hash, declared output ProjectPathRef set과 output manifest hash를 evidence에 고정한다.
- generated output은 source classification `generated`와 `generated_by` provenance를 가져야 한다.
- generated file 직접 편집은 declaration 변경이 아니며 M3 `generated.direct-edit`로 차단한다.
- generator output set 밖 변화, 같은 input의 다른 output, missing provenance와 stale output은 success가 아니다.
- generator가 unavailable하면 source를 수동으로 맞춰 통과시키지 않고 `human_review` 또는 required binding `block`으로 둔다.

### codegen과 codemod 선택

| 조건 | 선택 |
|---|---|
| manifest만으로 language·Schema·docs output을 완전히 결정하고 output 전체가 generated owner임 | `codegen` |
| hand-authored definition·reference를 semantic rename·API transition해야 함 | `codemod` |
| parser/semantic coverage가 없고 exact bounded path/range만 안전함 | 별도 `text_replace` Recipe + human review. global replacement 금지 |
| 의미 판단이나 consumer 설계 선택이 필요함 | `manual_review` 후 typed ChangePlan |

codegen도 M4 `rewrite_kind=codegen`, isolated preview, idempotence와 PatchSet을 사용한다. codemod는 syntax/symbol-aware Recipe를 사용한다. 둘 다 live source를 직접 바꾸거나 M2/M3를 우회하지 않는다.

## consumer project와 최소 지원 version

### ConsumerContract

| 필드 | 필수 | 의미 |
|---|---:|---|
| `consumer_project_id` | 예 | consumer ProjectId |
| `consumer_surface_id` | 예 | package·API·Schema·CLI 등 consuming surface key |
| `minimum_supported_version` | 예 | provider/Registry가 계속 호환을 약속하는 가장 낮은 consumer release SemVer |
| `first_version_with_primary_binding` | 전환 시 | 새 primary declaration을 이해하는 최초 consumer version |
| `accepted_declaration_versions` | 예 | consumer가 이해하는 declaration item version range |
| `required_binding_ids` | 예 | 이 consumer에 필요한 definition/schema/generated binding |
| `compatibility_mode` | 예 | `required`, `optional`, `observed_only` |
| `transition_deadline` | alias/deprecation 시 | registry version 상한과 optional UTC expiry |
| `required_check_families` | 예 | contract/build/test/docs check floor |

`minimum_supported_version`은 현재 설치 version을 DB에 복사한 값이 아니라 owner가 선언한 지원 약속이다. M1이 manifest·package metadata에서 관찰한 actual consumer version은 `ConsumerObservation`에 별도로 둔다.

#### ConsumerObservation

| 필드 | 필수 | 의미 |
|---|---:|---|
| `consumer_contract_ref` | 예 | declaration·consumer surface의 exact contract |
| `consumer_project_id`, `checkout_id` | 예 | 관찰한 consumer working copy |
| `observed_version` | 확인 시 | manifest/package metadata에서 읽은 SemVer와 provenance |
| `observed_binding_ids` | 예 | current Index에서 확인한 required/alias binding set |
| `reference_summary` | 예 | primary/deprecated/removed reference count와 exact evidence ref |
| `compatibility_status` | 예 | 아래 상태 enum |
| `freshness`, `coverage`, `tier`, `limitations` | 예 | 관찰 품질. partial/unverified를 complete로 합성하지 않음 |
| `content_fingerprint` | 예 | timestamp·private path를 제외한 canonical observation hash |

같은 consumer Project에 여러 checkout이 있으면 current target checkout별 observation을 분리한다. global projection에는 ProjectId·surface·status·snapshot ref만 두고 private symbol·source path를 복제하지 않는다.

consumer 상태는 다음과 같다.

| 상태 | 의미 |
|---|---|
| `compatible` | current consumer가 primary declaration·item version을 지원하고 required binding이 current |
| `compatible_via_alias` | finite migration window 안에서 deprecated/alias binding만 사용 |
| `transition_required` | 지원 약속 범위에 old binding consumer가 남아 있음 |
| `below_minimum` | observed consumer version이 선언한 minimum보다 낮음 |
| `stale` | consumer source/index가 current가 아님 |
| `unverified` | version·binding·reference를 확인하지 못함 |
| `not_applicable` | complete applicability evidence로 consumer가 아님 |

consumer index가 missing·stale·partial이면 “전환 완료”가 아니다. addition은 policy에 따라 possible impact와 review로 진행할 수 있지만 deprecated declaration의 removal은 모든 required consumer가 `compatible`이고 reference coverage가 current·complete할 때만 허용한다.

## lifecycle·alias·migration

### LifecycleRecord

| 필드 | 필수 | 의미 |
|---|---:|---|
| `introduced_in_registry_version` | reserved/active/deprecated/removed | 최초 allocation·public 도입 version |
| `deprecated_in_registry_version` | deprecated/removed | 새 reference를 금지하기 시작한 version |
| `removed_in_registry_version` | removed | runtime 사용을 종료한 version |
| `replacement_declaration_id` | replacement가 있으면 | active successor. 자기 자신·removed target 금지 |
| `no_replacement_reason` | replacement가 없으면 | terminal retirement의 reviewed reason |
| `migration_window` | deprecated/removed | 시작·exclusive 종료 registry version과 optional UTC 상한 |
| `transition_evidence_refs` | deprecated/removed | consumer plan·Gate·EvidenceBundle immutable ref. 없으면 empty |
| `status_reason` | reserved/deprecated/removed | 표시가 아닌 reviewed lifecycle 이유 |

status와 맞지 않는 미래 transition field는 쓰지 않는다. `active`에는 removed version이 있을 수 없고 `removed`는 introduced·deprecated·removed version이 단조 증가해야 한다. replacement와 `no_replacement_reason`을 동시에 쓰지 않는다. removed entry는 manifest에서 삭제하지 않고 ID·primary/alias value를 tombstone으로 계속 보존한다.

### 상태 의미

| 상태 | 새 definition/reference | runtime 사용 | 필수 정보 |
|---|---|---|---|
| `reserved` | 금지 | 금지 | owner, kind, type, reserved reason·version |
| `active` | 허용 | primary value 사용 | introduced version, current binding·consumer contract |
| `deprecated` | 새 사용 금지, 기존 reference는 migration 대상 | 호환 기간 안에서만 허용 | replacement 또는 deprecation reason, finite migration window |
| `removed` | 금지 | 금지. alias accept만 별도 replacement에서 가능 | removed version, terminal tombstone, migration evidence |

`reserved`는 아직 공개하지 않은 ID/value allocation이고 `removed`는 과거 공개된 의미의 terminal tombstone이다. 두 상태 모두 다른 의미에 재사용할 수 없다.

### 허용 전이

```text
reserved -> active
reserved -> removed
active -> deprecated
deprecated -> active      # 같은 의미로 deprecation 철회, ID·value 변경 없음
deprecated -> removed
removed -> terminal
```

`active -> removed`를 건너뛰지 않는다. 긴급 보안 제거가 필요해도 별도 breaking ChangePlan, 새 replacement/consumer 조치와 human approval을 만들며 정상 자동 lifecycle 전이로 합성하지 않는다.

### AliasRecord

| 필드 | 필수 | 의미 |
|---|---:|---|
| `alias_id` | 예 | declaration 안의 stable alias record ID |
| `alias_kind` | 예 | `previous_managed_id`, `previous_public_value`, `previous_symbol_name` |
| `alias_value` | 예 | kind에 맞는 typed old identity |
| `introduced_in` | 예 | alias를 받기 시작한 registry/item version |
| `accepted_until_registry_version` | 예 | finite exclusive upper bound |
| `expires_at` | 아니요 | 운영상 더 이른 UTC boundary. Gate `valid_until` 입력 |
| `replacement_declaration_id` | 예 | current primary target |
| `consumer_scope` | 예 | alias를 허용한 consumer set |
| `read_policy` | 예 | v1은 `accept_only` 고정 |
| `write_policy` | 예 | v1은 `primary_only` 고정 |

alias는 ID 재사용 허가가 아니다. generator와 새 source binding은 primary만 emit한다. old value를 계속 emit해야 한다면 old declaration을 `deprecated`로 유지하고 별도 compatibility binding을 명시한다.

alias graph는 acyclic이어야 하고 terminal target은 `active` declaration 하나여야 한다. alias value는 같은 uniqueness scope의 다른 active/reserved/removed primary·alias와 충돌할 수 없다.

### 제거 gate

deprecated declaration을 removed로 바꾸기 전에 다음을 모두 증명한다.

1. replacement 또는 명시적인 no-replacement removal reason과 alias/migration range가 manifest에 있다.
2. required consumer의 current source·version·reference coverage가 complete하다.
3. old primary definition·generated output과 compatibility shim 밖 old reference가 0건이다.
4. 남은 old reference는 허용된 compatibility shim 안에만 있고 removal PatchSet과 함께 제거된다.
5. `minimum_supported_version >= first_version_with_primary_binding` 관계가 모든 required consumer에서 성립한다.
6. docs·Schema·examples가 primary를 가리키고 deprecated ID를 새 사용으로 안내하지 않는다.
7. removal 뒤 old ID와 public value tombstone이 manifest에 남는다.

하나라도 stale·partial·unverified이면 removal PatchSet은 `human_review`로 낮추지 않고 기본 `block`이다. 사용자가 waiver로 ID 재사용을 허용할 수 없다.

## error code·diagnostic ID 첫 수직 Slice

### 범위

첫 Slice는 다음 두 kind만 `managed_declaration`으로 publish한다.

- `ErrorEnvelope.code`의 stable error code
- `Diagnostic.rule_ref.rule_id`의 stable Diagnostic ID

CLI exit code는 지원 순서 4번으로 남기며 첫 Slice에 포함하지 않는다. error display message, title, remediation text와 localization string은 첫 Slice의 stable ID 값이 아니다.

### display message와 stable code

| 변경 | Registry 처리 |
|---|---|
| 오탈자·표현 개선, 같은 기계 의미 | `message`/docs source만 변경. error code declaration ID·primary value 유지 |
| context parameter·redaction 개선, code 의미 동일 | error code 유지, message Schema/producer fixture만 검증 |
| 기계가 분기해야 하는 실패 의미 변경 | 새 managed declaration·새 public code 발급 |
| 기존 code 이름을 보기 좋게 rename | in-place rename 금지. 새 code + old deprecated + finite migration |
| 더 이상 발생하지 않는 code | deprecated 후 consumer 0건을 증명하고 removed tombstone |

error code value는 uppercase `<CATEGORY>_<SPECIFIC_REASON>` grammar와 [오류 namespace](errors-and-diagnostics.md#오류-namespace)를 만족한다. Rule ID는 Catalog ID grammar와 Rule definition fingerprint contract를 만족한다.

### first-slice lifecycle 예

1. scanner가 source·문서·Schema에서 `REGISTRY_MANIFEST_INVALID`를 발견하면 먼저 candidate다.
2. 사용자가 의미·owner·error category·consumer를 검토하고 manifest PatchSet을 승인하면 active managed declaration이 된다.
3. message text만 바뀌면 code는 유지한다.
4. code 의미를 분리해야 하면 새 code를 active로 추가하고 old code를 deprecated로 바꾼다.
5. consumer는 migration window 안에 새 code로 전환한다. old code는 alias 또는 deprecated compatibility binding으로만 읽는다.
6. current consumer 0건과 minimum version을 증명한 뒤 old code를 removed로 바꾸고 tombstone을 보존한다.

기존 오류 표의 문자열을 DB에서 추출해 곧바로 manifest로 쓰지 않는다. source 후보 inventory, duplicate/conflict report, 사용자 승인, manifest-only PatchSet과 post Gate를 거친다.

## ManagedRegistrySnapshot과 derived Index

`star.managed-registry-snapshot` v1은 Controller가 current Project Catalog·Code Index와 manifest source를 결합해 만든 immutable derived document다.

| 필드 | 필수 | 의미 |
|---|---:|---|
| `managed_registry_snapshot_id` | 예 | `mrs_` + full SHA-256 base32 derived ID |
| `registry_id`, `registry_version` | 예 | source manifest identity |
| `owner_project_id`, `checkout_id` | 예 | source owner와 관찰 working copy |
| `project_revision_id`, `workspace_snapshot_id` | 예 | 실제 읽은 source byte |
| `manifest_source_refs` | 예 | root·fragment CanonicalSourceId·content hash·ProjectPathRef |
| `namespace_claims` | 예 | resolved owner·delegation과 collision 상태 |
| `declaration_entries` | 예 | ID/version/fingerprint/status/type/value/owner summary |
| `binding_observations` | 예 | expected/actual definition·reference·Schema·docs·generated 관계 |
| `consumer_observations` | 예 | version·binding·compatibility·freshness 상태 |
| `candidate_index_refs` | 예 | M1 candidate detail을 소유한 CodeIndexSnapshot/ref |
| `local_constant_index_refs` | 예 | 검색 가능한 local constant detail ref |
| `tombstone_set_fingerprint` | 예 | reserved·removed ID/value set |
| `resolution_state` | 예 | `valid`, `conflicted`, `invalid`, `partial`, `unverified` |
| `freshness` | 예 | `current`, `stale_source`, `stale_catalog`, `partial`, `unverified`, `unavailable` |
| `coverage`, `limitations`, `diagnostic_refs` | 예 | manifest·language·consumer별 관찰 범위와 문제 |
| `content_fingerprint` | 예 | 모든 semantic resolved content의 canonical hash |

snapshot identity payload는 registry ID/version, owner Project/Checkout, source revision/workspace, manifest semantic fingerprint, namespace/declaration/tombstone set, binding·consumer observation content fingerprint, CodeIndexSnapshot ref와 completeness를 포함한다. timestamp, raw absolute path, display text, DB key와 cache hit는 제외한다.

candidate와 local constant의 source detail을 snapshot에 복제하지 않는다. snapshot은 owning CodeIndexSnapshot, query scope·classification reason과 summary fingerprint만 참조한다.

### publish와 freshness

1. current source root·fragment 전체를 읽고 schema·reference·namespace·lifecycle을 검증한다.
2. current M1 Index에서 owner Symbol·binding·reference·consumer를 resolve한다.
3. duplicate·collision·reuse·alias cycle과 generated provenance를 검사한다.
4. resolution 결과를 invisible generation에 쓰고 fingerprint·count·reference integrity를 검증한다.
5. `valid`이고 required coverage가 complete한 generation만 usable current pointer가 될 수 있다.
6. invalid source에서는 이전 current pointer를 current로 유지하지 않는다. 이전 generation은 `stale_source` historical view다.

`resolution_state=valid`는 consumer가 모두 전환됐다는 뜻이 아니다. consumer 상태가 `transition_required`여도 manifest 자체는 valid할 수 있지만 removal·breaking change Gate는 block한다.

source가 indexed input보다 바뀐 snapshot 자체의 freshness 값은 `stale_source`다. 6단계가 “DB Index가 Git 정본과 다름”이라는 관계를 보고할 때는 같은 사실을 `RegistryConsistencyRecord.status=stale_registry_index`로 정규화한다. 두 값을 서로 다른 성공/실패로 해석하지 않는다.

## conflict·duplicate·collision 탐지

### fail-closed conflict

다음은 manifest/snapshot publish를 거부한다.

- 같은 `managed_declaration_id`의 중복 entry
- 같은 ID·item version의 다른 definition fingerprint
- 같은 uniqueness scope의 primary/alias/tombstone value 충돌
- explicit delegation 없는 namespace owner 또는 prefix collision
- namespace claim과 declaration OwnerRef Project 불일치
- 같은 binding target을 서로 다른 declaration이 authoritative definition으로 소유
- `value_type`과 primary/default/binding projection type 불일치
- alias cycle, 자기 alias, 무한 range 또는 removed target alias
- lifecycle 전이 역행, removed entry 삭제, ID·value tombstone 재사용
- generated output의 owner·generator provenance 불일치
- root에 없는 fragment, 다른 registry ID fragment, root escape·remote include

### valid snapshot 안의 compatibility Diagnostic

다음은 source를 무효화하지 않을 수 있지만 change Gate와 6단계 drift에 영향을 준다.

- active declaration binding missing·value mismatch
- deprecated ID의 새 reference
- removed ID/reference·generated output 잔존
- alias window 만료 또는 만료 임박
- consumer `transition_required|below_minimum|stale|unverified`
- docs·Schema가 old ID 또는 다른 default를 설명
- generated output stale·direct edit·nondeterministic replay
- manifest와 language symbol name mapping drift

같은 literal count가 많다는 사실은 duplicate declaration이나 collision 증거가 아니다. stable owner·namespace·type·binding relation을 먼저 확인한다.

## 변경 workflow

### 입력 `ManagedDeclarationChangeIntent`

DB index를 보여주는 향후 **DB 편집 UI**라는 표현은 browser UI나 DB source editor가 아니라 CLI·terminal 기반 management view를 뜻한다. 어떤 관리 surface도 DB row를 source 값으로 직접 저장하거나 Git file을 즉시 수정하지 않는다. 사용자가 선택한 변경을 다음 typed intent로 만든다.

| 필드 | 필수 | 의미 |
|---|---:|---|
| `registry_snapshot_ref` | 예 | 사용자가 본 current snapshot ID·hash |
| `declaration_ref` | 기존 변경이면 | ID·item version·definition fingerprint |
| `change_kind` | 예 | `create`, `update_description`, `change_default`, `deprecate`, `add_alias`, `remove`, `add_binding`, `change_consumer_floor`, `classify_candidate` |
| `desired_fields` | 예 | kind별 Schema를 통과한 typed 값 |
| `reason` | 예 | 사용자 의도와 호환성 근거 |
| `requested_consumer_scope` | 예 | read-only 영향 분석 범위 |
| `expected_manifest_fingerprint` | 예 | stale source 방지 |

intent는 local planning input이며 source 정본이 아니다. raw replacement, SQL, shell command와 전체-project literal selector를 포함하지 않는다.

### 필수 순서

```text
current Git manifest + M1 Registry/Code Index
  -> typed ManagedDeclarationChangeIntent
  -> TaskSpec + accepted ScopeRevision
  -> M2 ImpactAnalysis + ChangePlan + ValidationPlan
  -> M4 managed_declaration ChangeRecipe dry-run
  -> RecipeExecution + recipe_preview ChangeSet
  -> M2 impact·Profile·Check reconciliation
  -> immutable single-Project PatchSet
  -> M3 patch_pre_apply Gate + exact approval
  -> M4 PatchApplication
  -> new source snapshot + ManagedRegistrySnapshot rebuild
  -> M3 patch_post_apply Gate
  -> EvidenceBundle + ReviewPack
```

어느 진입점도 `intent -> DB update -> source sync` 경로를 만들지 않는다.

### M1 resolution

- manifest root·fragment, declaration owner와 expected fingerprint를 current source에서 확인한다.
- language symbol·Schema·docs·generated owner와 actual reference set을 current Index에서 resolve한다.
- candidate/local classification, tier·coverage·limitation을 별도 보존한다.
- source나 Index가 stale·partial이면 write readiness를 만들지 않는다.

### M2 impact·consumer planning

- declaration, owner contract, binding source와 actual reference를 seed로 사용한다.
- provider Project와 consumer Project를 separate partition으로 유지한다.
- display-only change, compatible addition, default change, stable ID change, deprecation, removal을 다른 risk class로 판정한다.
- contract/build/test/docs/generated consistency Check와 consumer fallback scope를 materialize한다.
- cross-project impact는 read-only다. 다른 Project의 planned change unit을 owner PatchSet에 넣지 않는다.
- consumer 전환이 필요한데 5단계에서 적용할 수 없으면 `related_project_impacts`와 `transition_required`를 남기고 호환 window 없이 breaking owner change를 ready로 만들지 않는다.

#### ManagedRegistryExpectation

M2가 ValidationPlan `managed_registry_expectations`에 넣는 항목은 Project별로 다음 field를 가진다.

| 필드 | 필수 | 의미 |
|---|---:|---|
| `project_id`, `checkout_id` | 예 | 검증·적용 subject. 한 expectation은 한 Project만 소유 |
| `before_snapshot_ref`, `before_manifest_fingerprint` | 예 | planning input이 된 current Registry source·derived snapshot |
| `target_declaration_ref` | 예 | ID·item version·before definition fingerprint |
| `expected_manifest_fingerprint` | prepare 뒤 | PatchSet operation에서 계산한 expected-after source identity |
| `expected_declaration_fingerprint` | prepare 뒤 | desired semantic state의 after fingerprint |
| `expected_namespace_tombstone_fingerprint` | 예 | collision·reuse 판정 대상 set |
| `expected_binding_fingerprint`, `expected_consumer_fingerprint` | 예 | required after binding·compatibility set |
| `required_compatibility_status` | 예 | `compatible`, `compatible_via_alias`, `transition_required` 중 policy가 허용한 floor |
| `required_freshness`, `required_coverage`, `required_check_families` | 예 | 자동 Gate 최소 evidence floor |
| `expectation_fingerprint` | 예 | 위 semantic field의 canonical SHA-256 |

initial M2 plan에서 PatchSet after byte가 아직 없으면 expected-after 두 field는 `pending_prepare` typed state이고 ValidationPlan은 Registry apply용 `ready`가 아니다. M4 preview와 M2 reconciliation 뒤 exact fingerprint를 채운 새 plan revision만 pre Gate 입력이 된다. runner나 DB UI가 pending 값을 현재 source로 추측하지 않는다.

### M4 dry-run·PatchSet

- selector는 `kind=managed_declaration`, declaration ID와 expected fingerprint를 사용한다.
- manifest fragment가 source target이며 DB row·generated output은 primary target이 아니다.
- 같은 Project 안의 binding·generator output 변경은 declared ownership과 Recipe assurance가 있을 때 하나의 PatchSet preview에 포함할 수 있다.
- generated output은 generator input으로부터 isolated preview에서 만든다.
- hand-authored reference 전환은 syntax/symbol-aware codemod가 current coverage를 증명할 때만 automatic apply 대상이다.
- preview가 새 consumer/risk/check를 발견하면 PatchSet을 만들지 않고 M2 재계획한다.

### M3 pre/post Gate

pre-apply는 최소 다음을 검사한다.

1. manifest·snapshot·declaration expected fingerprint가 current다.
2. duplicate ID, namespace collision, tombstone reuse와 alias/lifecycle conflict가 없다.
3. requested transition이 허용 state edge이며 item/registry version 변화가 맞다.
4. M2 consumer impact·minimum version·migration window와 required Check가 ready다.
5. PatchSet이 한 Project·한 Checkout만 수정하고 generated output 직접 편집이 없다.
6. codegen/codemod assurance, idempotence, reverse artifact와 approval scope가 complete하다.

post-apply는 최소 다음을 검사한다.

1. actual manifest byte가 expected after fingerprint와 일치한다.
2. 새 ManagedRegistrySnapshot이 source와 `current`, required scope에서 `valid + complete`다.
3. binding definition·type·primary/default value와 symbol name mapping이 declaration과 일치한다.
4. generated output manifest가 exact하고 direct edit·undeclared output이 없다.
5. deprecated/removed ID reference와 consumer transition 상태가 policy를 만족한다.
6. docs·Schema·config example·error/Diagnostic table과 language binding drift가 없다.
7. M2가 선택한 contract/build/test/docs/generated/consumer Check가 current·complete·stable하다.

post Gate가 block이어도 실제 Git source를 DB LKG로 덮어쓰지 않는다. PatchApplication은 actual source와 recovery 상태를 보존하며 reverse는 별도 precondition·permission·Gate를 요구한다.

## validation·evidence 계약

Registry change의 EvidenceSubjectBinding에는 일반 M3 field에 더해 다음 M5 target을 포함한다.

- before/expected-after/actual-after ManagedRegistrySnapshot ref와 fingerprint
- declaration ID·item version·before/after definition fingerprint
- manifest root·fragment source ref와 content hash
- namespace claim·tombstone set fingerprint
- BindingSpec/ManagedBindingObservation set fingerprint
- ConsumerContract/ConsumerObservation set fingerprint
- alias·lifecycle·minimum version evaluation ref
- generator/Recipe/Tool identity와 output manifest
- RegistryConsistencyRecord set fingerprint

EvidenceBundle은 manifest diff, candidate promotion decision, M2 cross-project read-only impact, PatchSet, pre/post Gate, consumer transition table와 6단계 drift input을 연결한다. 큰 manifest/rendered report·generated diff는 ArtifactRef로 분리한다.

CompletionClaim `registry_current`는 다음을 모두 만족할 때만 verified다.

- actual Git manifest hash가 authoritative snapshot input과 같음
- snapshot `freshness=current`, `resolution_state=valid`
- required binding·consumer coverage complete
- blocking drift 0건
- post Gate `auto_pass`와 complete EvidenceBundle·ReviewPack

## stable error·Diagnostic·event

command failure는 다음 stable error code를 사용한다.

- `REGISTRY_MANIFEST_INVALID`
- `REGISTRY_SNAPSHOT_STALE`
- `REGISTRY_DECLARATION_CONFLICT`
- `REGISTRY_DUPLICATE_ID`
- `REGISTRY_NAMESPACE_COLLISION`
- `REGISTRY_ID_REUSE_FORBIDDEN`
- `REGISTRY_ALIAS_INVALID`
- `REGISTRY_BINDING_UNRESOLVED`
- `REGISTRY_CHANGE_STALE`
- `REGISTRY_CROSS_PROJECT_APPLY_UNSUPPORTED`

project/source 문제는 ErrorEnvelope로 숨기지 않고 다음 Rule family의 Diagnostic으로 남긴다.

- `star.validation.registry.binding-drift`
- `star.validation.registry.consumer-not-migrated`
- `star.validation.registry.deprecated-reference`
- `star.validation.registry.removed-reference`
- `star.validation.registry.alias-window-expired`
- `star.validation.registry.generated-output-stale`
- `star.validation.registry.generated-direct-edit`
- `star.validation.registry.docs-schema-drift`

최소 event 흐름은 다음이다.

```text
registry.snapshot_started
registry.snapshot_published | registry.snapshot_failed
registry.change_planned
registry.candidate_classified
registry.patch_prepared
registry.patch_applied | registry.patch_blocked
registry.consumer_transition_observed
registry.post_gate_completed
```

event에는 raw source, manifest byte, private symbol name와 consumer path를 inline으로 넣지 않는다.

## version·compatibility 원칙

Managed Registry는 네 version 축을 분리한다.

| 축 | 증가 기준 |
|---|---|
| manifest `schema_version` | root/fragment wire shape 또는 해석 의미 변경 |
| `registry_version` SemVer | declaration set·namespace·호환 정책 변화 |
| declaration `item_version` SemVer | 한 declaration의 type·value·lifecycle·binding·consumer 의미 변화 |
| `ManagedRegistrySnapshot.schema_version` | derived snapshot wire shape·freshness 의미 변화 |

- 설명·표시 text만 바뀌고 machine 의미가 같으면 item version을 유지할 수 있지만 source byte provenance는 바뀐다.
- 새 active declaration과 backward-compatible binding 추가는 registry minor다.
- compatible alias 추가는 old/new declaration item minor와 registry minor다.
- type 변경, stable value in-place 변경, namespace ownership 이전, compatibility floor 축소와 removal은 breaking이다. stable ID/value in-place 변경 대신 새 declaration을 사용한다.
- status·alias·consumer floor·binding·uniqueness scope는 definition fingerprint에 포함한다.
- future manifest version은 metadata inspection만 허용하고 resolution·codegen·Patch target으로 사용하지 않는다.

기존 source에서 candidate를 onboarding하는 것은 DB migration이 아니다. reviewed manifest ChangePlan으로 새 Git source를 추가하는 source migration이다. DB projection은 manifest를 읽어 rebuild한다. 과거 DB candidate row를 manifest entry로 자동 변환하지 않는다.

## 6단계 drift·compatibility 인계

6단계는 M1 text search를 새 truth로 만들지 않고 `ManagedRegistrySnapshot`의 expected relation과 current source observation을 비교한다.

### RegistryConsistencyRecord

| 필드 | 의미 |
|---|---|
| `declaration_ref` | ID·item version·definition fingerprint |
| `relation_kind` | `definition`, `reference`, `schema`, `documentation`, `generated_output`, `consumer` |
| `expected_ref`, `observed_ref` | BindingSpec/ConsumerContract와 current observation |
| `freshness`, `coverage`, `tier`, `resolution` | 증거 품질 |
| `status` | 아래 drift code |
| `compatibility` | `compatible`, `compatible_via_alias`, `breaking`, `invalid`, `unverified` |
| `required_action` | `rescan`, `replan`, `regenerate`, `codemod`, `update_docs`, `migrate_consumer`, `review` |
| `content_fingerprint` | 의미 input/output canonical hash |

6단계 stable drift status는 다음을 최소 집합으로 사용한다.

- `in_sync`
- `stale_registry_index`
- `missing_binding`
- `unexpected_binding`
- `value_mismatch`
- `type_mismatch`
- `symbol_name_mismatch`
- `deprecated_reference`
- `removed_reference`
- `alias_window_expired`
- `consumer_below_minimum`
- `consumer_transition_incomplete`
- `generated_output_stale`
- `generated_output_unowned`
- `docs_schema_drift`
- `namespace_collision`
- `duplicate_id`
- `id_reuse_attempt`

`in_sync`는 expected relation의 current·complete observation일 때만 가능하다. stale·partial·unsupported·unverified를 0건 drift로 렌더링하지 않는다. same-value/different-owner는 통합 제안이 아니라 separate semantic declaration으로 유지한다.

6단계에 넘기는 authoritative input은 다음이다.

1. current manifest/source·ManagedRegistrySnapshot ref
2. declaration·namespace·tombstone definition fingerprint
3. expected BindingSpec·ConsumerContract
4. current CodeIndexSnapshot과 actual definition/reference/generated/docs observation
5. alias·lifecycle·minimum consumer version evaluation
6. exact RegistryConsistencyRecord와 limitation set

## application·Package 경계

```text
star-application
  -> star-project: manifest·candidate·binding·consumer read-only 관찰
  -> star-planning: Registry TaskSpec·impact·consumer scope·ChangePlan
  -> star-execution: existing M4 Recipe/PatchSet/apply·recovery
  -> star-validation: Registry consistency Check + pre/post Gate
  -> star-state/star-evidence: derived snapshot·event·artifact transaction
```

- `star-contracts`가 manifest/snapshot/ref·enum의 유일한 wire type을 소유한다.
- `star-project`가 current manifest loader, classification·binding observation과 snapshot 후보를 만든다. source write port를 받지 않는다.
- `star-planning`은 immutable snapshot·Index·intent로 영향과 consumer scope를 계산한다.
- `star-execution`은 Registry 전용 rewrite engine을 만들지 않고 M4 `managed_declaration` Recipe를 사용한다.
- `star-validation` B04/B07이 conflict·lifecycle·consumer·generated·docs drift를 공통 Diagnostic으로 만든다.
- `star-state`는 derived snapshot과 candidate/local projection을 저장하지만 source 값을 결정하지 않는다.
- Controller만 DB snapshot을 publish하고 PatchApplyPermit을 소비한다. CLI·MCP·관리 view는 DB나 source를 직접 쓰지 않는다.

새 Package를 만들지 않는다. Registry 범위가 독립 dependency·배포 경계를 실제로 갖기 전에는 기존 module 책임으로 유지한다.

## 첫 수직 Slice 구현 순서

제품 구현은 별도 승인 뒤 다음 순서를 따른다.

1. `ManagedDeclarationId`, root/fragment, declaration·alias·binding·consumer·snapshot contract type과 Schema
2. minimal/full/invalid/future fixture, ID·definition·tombstone fingerprint golden
3. source-only manifest loader와 duplicate·namespace·lifecycle·alias resolver
4. M1 error code·Diagnostic ID candidate discovery와 managed/local classification query
5. error code/Rule ID owner Symbol·Schema·docs binding observation
6. ManagedRegistrySnapshot invisible generation·freshness·source-wins rebuild
7. pure lifecycle·ID reuse·consumer compatibility engine
8. Registry-specific B04/B07 Diagnostic과 6단계 RegistryConsistencyRecord
9. M2 fake impact·consumer planning과 single-project ChangePlan/ValidationPlan
10. M4 fake `managed_declaration` Recipe·manifest-only PatchSet, pre/post M3 Gate
11. CLI-only query/plan/prepare/apply E2E와 DB direct-write 부재 검사
12. Windows x64·ARM64 contract·path·crash·redaction conformance

첫 Slice에서는 generator, language codemod, config key, CLI exit code와 cross-repo apply를 구현하지 않는다. manifest-only error/Diagnostic ID onboarding과 lifecycle·drift 판정이 먼저 통과해야 한다.

## 최소 Corpus

- 같은 error code가 두 source에서 발견됐지만 manifest owner가 하나인 정상 case
- 같은 public code를 다른 의미로 claim한 duplicate conflict
- 같은 string value이지만 namespace·semantic owner가 다른 정상 상수
- active→deprecated→removed와 removed ID/value 재사용 거부
- alias cycle, 무한 migration window와 expired alias
- display message만 바뀌어 stable code가 유지되는 case
- stable error 의미 변경이 새 declaration 없이 in-place edit된 실패 case
- candidate promotion과 local implementation classification
- config default와 compile-time contract constant 혼동 거부
- current provider·current consumer, stale consumer, below-minimum consumer
- generated output direct edit·stale output·undeclared output
- manifest source 변경 뒤 DB snapshot `stale_source`
- invalid source에서 last-known-good를 current로 쓰지 않는 case
- single-project PatchSet과 cross-project apply 거부
- pre Gate 뒤 manifest·Index·consumer drift로 permit 무효화
- post Gate에서 removed reference·docs/Schema drift 탐지
- secret·사용자 이름·절대 경로·민감 literal 미저장

## 설계 수용 기준

- 관리 대상, candidate와 local constant가 같은 상태로 섞이지 않는다.
- Git manifest가 공유 정본이고 DB는 derived Index라는 점이 모든 변경·복구 경로에서 유지된다.
- stable ID·namespace·owner·type·source·binding·consumer·lifecycle을 구현 가능한 field로 표현한다.
- error code의 message-only 변경, 새 code, deprecation, alias, removal과 tombstone 경계가 명확하다.
- ID·public value 재사용과 same-value 자동 통합이 금지된다.
- config default와 compile-time contract constant가 다른 role·Gate를 사용한다.
- definition·reference·generated output·docs·Schema 관계와 codegen/codemod 선택 기준이 명확하다.
- DB management surface는 manifest ChangePlan·PatchSet만 만들며 source를 직접 쓰지 않는다.
- Registry 변경이 M2 impact, M4 dry-run/PatchSet과 M3 pre/post Gate를 우회하지 않는다.
- conflict, duplicate, namespace collision, stale Index와 consumer 미전환이 fail-closed로 탐지된다.
- cross-project 영향은 표현하지만 실제 cross-repo apply는 9단계 전까지 거부된다.
- 6단계가 current source relation과 exact drift·compatibility code를 입력으로 받을 수 있다.
- 현재 상태가 문서 설계이며 Registry file·generator·codemod·실제 상수 구현 완료가 아님을 일관되게 표시한다.
