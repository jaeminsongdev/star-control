# 6단계 계약 호환성·문서·설정·개발 환경 관리

## 상태와 목적

이 문서는 6단계 **API·계약·문서·설정·개발 환경 관리**의 정본이다. 현재 상태는 **P-0047 첫 bounded 제품 Slice 구현**이다. `CompatibilityReport`·`CleanRoomDoctorReport` 공개 계약과 API·Schema·config·docs deterministic comparator, install/download/system mutation을 수행하지 않는 doctor engine을 구현했다. provider별 full public-surface extractor와 실제 clean VM 실행은 후속 adapter 범위이며 unverified를 pass로 승격하지 않는다. 구현 증거는 [M5~M9 제품 Slice](../testing/m5-m9-development-evidence-2026-07-20.md)에 고정한다.

6단계의 목적은 다음 질문에 결정적으로 답하는 것이다.

- 공개 API·CLI·Schema·파일 형식·config·error code가 승인된 baseline에서 어떻게 달라졌는가.
- 그 차이가 `breaking`, `additive`, `compatible`, `unknown` 중 무엇이며 어떤 소비자가 migration을 필요로 하는가.
- 계약 변경과 문서·Schema·생성 reference·consumer migration guide가 같은 변경 단위에서 함께 갱신됐는가.
- config key가 선언부터 문서화·읽기·override·deprecation·제거까지 추적되는가.
- 문서의 command·link·anchor·snippet·config example과 실제 등록 계약이 일치하는가.
- project doctor와 clean-room readiness가 대상 source나 system을 바꾸지 않고 환경 차이를 설명하는가.
- 후속 7단계 dependency·security 검사가 재발견 없이 사용할 manifest·lockfile·toolchain·environment 근거가 있는가.

## 선행 정본과 비중복 경계

6단계는 새 source-of-truth 체계를 만들지 않는다.

| 소유 정본 | 6단계가 재사용하는 내용 | 6단계가 소유하지 않는 내용 |
|---|---|---|
| [3단계 공통 검증·품질 Gate](../features/common-validation-gate.md) | `ValidationPlan`, exact subject binding, result 상태, Diagnostic, `GateDecision`, `EvidenceBundle`, `HUMAN_REVIEW` | 검사 재선택, 별도 pass 계산, 증거 신선도 완화 |
| [5단계 Managed Registry](managed-symbol-registry.md) | `ManagedDeclaration`, namespace, binding, consumer, lifecycle, tombstone, `RegistryConsistencyRecord` | symbol ID·값·owner·lifecycle의 중복 선언, DB를 정본으로 승격 |
| [변경 계획·영향 분석](change-planning-and-impact.md) | `ChangePlan`, `ImpactAnalysis`, consumer edge, affected check 선택 | 6단계에서 source 변경 범위를 즉석 생성 |
| [안전한 Patch·codemod](safe-patch-and-codemod.md) | immutable `PatchSet`, preview, 적용 전·후 revision | source·generated output 자동 수정 |
| [설정과 Catalog](config-and-catalog.md) | `EffectiveConfig`, descriptor와 registered tool/check | raw shell 실행, 별도 config precedence |
| [Version과 Migration](versioning-and-migrations.md) | version 축, manifest negotiation, persisted migration | 호환 기간을 DB migration과 혼합 |

관리 대상 config key, error code, Schema ID와 파일 형식 ID가 5단계 `managed_declaration`이면 6단계 record는 `declaration_ref`만 보관한다. 이름·owner·lifecycle·canonical value를 다시 선언하지 않는다. `candidate`와 `local_implementation_constant`는 비교 관찰에 포함할 수 있지만 관리형 계약으로 가장하지 않는다.

## 핵심 불변식

1. baseline은 승인된 immutable source/release에 명시적으로 결합한다. 현재 dirty checkout, DB의 최신 row 또는 가장 최근 시각을 baseline으로 자동 채택하지 않는다.
2. current는 검사 대상의 exact `ProjectRevision`, `WorkspaceSnapshot`, config, Catalog, Tool Registry와 environment fingerprint에 결합한다.
3. DB snapshot과 generated index는 derived state다. Git source manifest·source definition·Schema·등록 Catalog가 우선한다.
4. `unknown`은 compatible의 별칭이 아니다. required evidence가 없거나 의미 규칙이 결정적이지 않으면 성공으로 축약하지 않는다.
5. 확정된 breaking change가 하나라도 있으면 집계 결과는 `breaking`이다. breaking이 없더라도 required surface가 `unknown`이면 전체 결과는 `unknown`이다.
6. public surface 확대는 compatible일 수 있어도 의도된 변경이라는 증거가 없으면 별도 blocking Diagnostic이다.
7. 계약 변경은 public source, Schema·generated reference, 문서, compatibility metadata와 필요한 consumer migration guide를 한 `ChangePlan` lineage에서 함께 갱신한다.
8. doctor와 readiness 진단은 대상 checkout·사용자 config·OS·registry·PATH·package store를 수정하지 않는다.
9. CLI-only가 기본이다. 결정적 규칙으로 의미를 확정할 수 없는 항목은 AI 호출 없이 `HUMAN_REVIEW`로 남긴다.
10. network download, package 설치·update, 시스템 설정 변경은 필요한 조치로 진단할 수만 있고 자동 수행하지 않는다.

## 정본 입력과 derived 산출물

### source 입력

| 입력 | 역할 | 신선도 기준 |
|---|---|---|
| `.star-control/contracts.toml` | 비교 대상 public surface, baseline policy, docs·assumption·environment constraint 선언 | exact Git blob 또는 workspace content hash |
| `.star-control/managed-registry/manifest.toml`과 fragment | 관리형 ID·namespace·binding·consumer·lifecycle 정본 | `ManagedRegistrySnapshot.source_fingerprint`와 일치 |
| 공개 source, CLI descriptor, Schema, 파일 형식 선언 | current surface 관찰 | M1 `CodeIndex` generation과 source revision 일치 |
| docs source와 generated manifest | 문서·reference 관찰 | source hash·generator ID/version·input hash 결합 |
| package manifest, package-manager lockfile, task/tool descriptor | toolchain·명령·dependency 환경 발견 | 파일 hash와 Catalog generation 일치 |
| `ChangePlan`, `ImpactAnalysis`, `ValidationPlan` | 의도·소비자·검사 범위 | M2 readiness가 `ready`이고 current subject와 일치 |

`.star-control/contracts.toml`은 project별 선택적 정본이다. 파일이 없는데 Profile이 contract comparison을 요구하면 implicit baseline을 만들지 않고 `CONTRACT_MANIFEST_NOT_FOUND` 또는 `star.validation.contract.baseline-missing`으로 종료한다.

### top-level contract

공통 `schema_version`, `id`, `project_ref`, `created_at`, `producer`, `source_refs`는 [데이터 계약 지도](README.md)의 Envelope를 따른다.

| contract | contract ID | source/derived | 책임 |
|---|---|---|---|
| `ProjectContractManifest` | `star.project-contract-manifest` | Git source | surface·baseline·docs·assumption·environment constraint의 project 선언 |
| `ContractSurfaceSnapshot` | `star.contract-surface-snapshot` | derived immutable | 한 revision의 공개 surface 정규화 |
| `CompatibilityReport` | `star.compatibility-report` | derived immutable | baseline/current change, consumer impact와 migration 판정 |
| `DocumentationSnapshot` | `star.documentation-snapshot` | derived immutable | 문서 link·command·snippet·example·reference 관찰 |
| `EnvironmentSnapshot` | `star.environment-snapshot` | derived immutable | read-only toolchain·filesystem·OS fingerprint |
| `ProjectDoctorReport` | `star.project-doctor-report` | derived immutable | 환경 constraint 대비 진단과 수동 remediation |
| `CleanRoomSpecification` | `star.clean-room-specification` | Git source 또는 승인된 plan artifact | 이미 준비된 disposable 환경에서 재현할 조건과 금지 행동 |
| `DependencySecurityInputManifest` | `star.dependency-security-input-manifest` | derived immutable | 후속 7단계에 전달할 manifest·lockfile·toolchain·환경 근거 |

derived 산출물은 재생성할 수 있어야 하며 source manifest를 역으로 수정하지 않는다. source가 바뀌면 기존 derived 산출물은 `stale`이다.

## `ProjectContractManifest`

### 필수 필드

| 필드 | 형식 | 규칙 |
|---|---|---|
| `manifest_id` | stable ID | project 안에서 고유하며 재사용하지 않음 |
| `manifest_version` | SemVer | manifest shape의 호환 version |
| `project_ref` | `ProjectRef` | exact project identity |
| `baseline_policy` | object | baseline 선택·승인·compatibility 기간 정책 |
| `surfaces[]` | `ContractSurfaceDescriptor` | 공개 비교 대상만 선언 |
| `documentation[]` | `DocumentationTarget` | docs source, generated reference, command/example policy |
| `assumptions[]` | `AssumptionSpec` | 결정적으로 관찰할 file·command·version·platform 주장 |
| `environment_constraints[]` | `EnvironmentConstraint` | doctor가 읽기 전용으로 비교할 지원 범위 |
| `clean_room_spec_ref` | optional ref | 재현 검사가 required인 경우 필수 |

### `ContractSurfaceDescriptor`

| 필드 | 설명 |
|---|---|
| `surface_id` | stable public surface ID. managed 대상이면 `declaration_ref`와 동일 lineage |
| `kind` | `api`, `cli`, `schema`, `file_format`, `config`, `error_code` |
| `owner` | source owner package/module/file |
| `source_selector` | M1이 해석할 typed selector. glob text만으로 공개 표면을 결정하지 않음 |
| `declaration_ref` | M5 `ManagedDeclarationRef`, 해당할 때 필수 |
| `schema_ref` | machine-readable shape와 version |
| `generated_refs[]` | 생성물과 generator manifest binding |
| `documentation_refs[]` | 사용자 문서·reference·migration guide |
| `consumer_contract_refs[]` | M5 또는 M2 consumer contract |
| `compatibility_policy_ref` | kind별 비교 규칙과 지원 기간 |
| `visibility_policy` | 허용 export/command/schema root/file/config/error namespace |

`source_selector`가 dynamic/reflection/macro expansion을 완전히 관찰하지 못하면 `coverage=partial`과 limitation을 남긴다. text search만으로 `coverage=complete`를 주장하지 않는다.

## baseline과 current snapshot

### baseline 선택

baseline은 다음 중 하나로 고정한다.

- 승인된 release artifact와 `ReleaseManifest` hash
- 승인된 Git commit/tree와 `ProjectContractManifest` blob hash
- migration window가 명시된 이전 public contract artifact

`baseline_policy`는 `baseline_ref`, `approval_ref`, `activated_at`, `supported_until`, `minimum_consumer_version`을 가진다. `supported_until`은 version boundary를 우선하며 시간이 필요한 경우 UTC instant를 함께 쓴다. branch 이름만, mutable tag만, 로컬 DB row ID만 있는 baseline은 유효하지 않다.

baseline 부재·mutable ref·approval 불명·artifact hash 불일치·지원 기간 해석 불능은 `unknown`이다. current를 baseline으로 복사해 zero diff를 만드는 fallback은 금지한다.

### `ContractSurfaceSnapshot`

| 필드 | 설명 |
|---|---|
| `snapshot_role` | `baseline` 또는 `current` |
| `subject_binding` | project revision, workspace, manifest, config, Catalog, Tool, environment fingerprint |
| `surfaces[]` | 정렬·정규화된 `SurfaceObservation` |
| `coverage` | `complete`, `partial`, `unverified` |
| `limitations[]` | 언어 adapter, dynamic dispatch, generated source 등 누락 가능성 |
| `registry_snapshot_ref` | exact M5 snapshot |
| `content_fingerprint` | 시간과 로컬 절대 경로를 제외한 canonical content hash |

`SurfaceObservation`은 `surface_id`, `kind`, normalized signature/shape, visibility, source location, schema/generated/docs bindings와 evidence ref를 포함한다. 정규화기는 의미 있는 case, order, default, nullability, encoding과 unknown-field 정책을 삭제하지 않는다.

## 호환성 분류

### 결과 enum과 집계

| 값 | 뜻 | 기본 처리 |
|---|---|---|
| `unchanged` | canonical surface fingerprint가 동일 | pass 후보 |
| `compatible` | 변경은 있으나 기존 소비자 동작을 깨지 않음 | 의도·동시 변경 확인 뒤 pass 후보 |
| `additive` | 새 공개 기능이 추가됐고 기존 소비자 계약을 보존 | 의도된 public 확대와 consumer 정책 확인 |
| `breaking` | 기존 소비자가 변경 없이 계속 사용할 수 없음 | migration과 compatibility window required |
| `unknown` | evidence 부족 또는 결정적 의미 판정 불가 | 누락 evidence면 block, 의미 판단이면 `HUMAN_REVIEW` |

집계 순서는 `breaking > unknown > additive > compatible > unchanged`다. 단, 이 순서는 위험 우선 표시용이며 `additive`를 자동 허용한다는 뜻이 아니다. required surface 하나가 stale·partial·unverified이면 해당 비교와 전체 report를 `unknown`보다 강한 pass로 계산하지 않는다.

### kind별 최소 판정표

| kind | additive 후보 | compatible 후보 | breaking | unknown 예 |
|---|---|---|---|---|
| public API | 충돌 없는 새 export·optional member | 문서·metadata만 변경, 의미 보존 alias | export 제거, visibility 축소, required parameter·return/error 의미 변경 | macro/reflection으로 call shape 불명 |
| CLI | 새 command, 새 optional option | help·description 변경, 동일 parse/exit/output contract | command/option 제거·rename, required arg 추가, default·exit code·machine output 의미 변경 | runtime-only dispatch, plugin command coverage 불명 |
| Schema | unknown field를 보존하는 optional field | annotation·description 변경 | required field 추가, type/nullability 축소, field 제거·rename | enum 추가를 old consumer가 처리하는지 불명 |
| file format | reader가 보존하는 optional field와 default | canonical ordering처럼 의미 없는 serialization 차이 | magic/version/encoding 변경, required field·meaning 변경, old reader가 data loss | unknown field·round-trip 정책 불명 |
| config | default가 있고 optional인 새 key | description·example 보정 | key 제거·rename, type·merge precedence·default 의미 변경 | 실제 reader/override path coverage 불명 |
| error code | unknown-code fallback이 보장된 새 code | message wording만 변경 | code 제거, 의미 재사용, retry/severity/exit mapping 변경 | 소비자가 exhaustive switch인지 불명 |

Schema enum 추가, API overload 추가, CLI option 단축명 추가처럼 모호성·exhaustiveness·resolution에 영향을 줄 수 있는 변경은 단순 추가만 보고 `additive`로 확정하지 않는다. 등록된 kind별 rule이 해당 소비자 정책을 증명하지 못하면 `unknown`이다.

### `ContractChangeRecord`

각 차이는 다음을 보존한다.

- `contract_change_group_id`, `change_id`, `surface_ref`, `kind`, `baseline_observation_ref`, `current_observation_ref`
- `classification`, `rule_id`, `confidence`, `evidence_refs[]`, `limitations[]`
- `intent_status`: `declared`, `undeclared`, `conflicting`, `unknown`
- `public_surface_delta`: `none`, `expanded`, `narrowed`, `replaced`, `unknown`
- `consumer_impacts[]`, `migration_requirement`, `compatibility_window_ref`
- `required_companion_changes[]`와 충족 상태
- blocking Diagnostic refs

분류 rule과 evidence가 없는 free-form 결론은 report에 넣지 않는다.

### `CompatibilityReport`

| 필드 | 설명 |
|---|---|
| `manifest_ref` | exact `ProjectContractManifest` ID/version/hash |
| `baseline_snapshot_ref`, `current_snapshot_ref` | 비교한 두 immutable snapshot과 content fingerprint |
| `baseline_approval_ref` | baseline activation actor·scope·artifact hash |
| `change_records[]` | `(surface_id, change_id)` byte-order로 정렬한 `ContractChangeRecord` |
| `aggregate_classification` | 위 위험 우선 집계 결과 |
| `consumer_coverage` | declared·observed·unresolved 집합별 completeness·limitation |
| `migration_summary` | requirement별 count와 required owner/guide/deadline 누락 |
| `companion_change_evaluation` | required/not-applicable/fulfilled/missing 항목과 rule evidence |
| `diagnostic_refs[]` | compatibility·public expansion·migration blocking issue |
| `completeness`, `missing_reasons[]` | required surface/consumer/evidence 축의 상태 |
| `report_fingerprint` | timestamp·render·raw path를 제외한 canonical content hash |

report는 같은 surface의 여러 change를 숨기지 않는다. `aggregate_classification`만 저장하고 record를 버리거나, consumer별 `blocked_unknown`을 전체 count로 축약해 owner를 잃지 않는다.

## 소비자·migration·deprecated lifecycle

### 소비자 coverage

소비자 목록은 M5 `ConsumerContract`, M2 dependency/impact edge와 current M1 observation을 합성한다. 세 집합을 구분한다.

| 집합 | 의미 |
|---|---|
| declared consumers | Registry 또는 manifest에 명시된 소비자 |
| observed consumers | current Code Index·Schema import·CLI invocation·config reader에서 관찰된 소비자 |
| unresolved candidates | text/dynamic/reflection 등 확정하지 못한 후보 |

declared와 observed가 다르면 Registry를 자동 보정하지 않고 `RegistryConsistencyRecord`와 Diagnostic을 만든다. required consumer coverage가 `partial|unverified`이면 제거·rename·type change를 안전하다고 판정하지 않는다.

### `ConsumerImpactRecord`

필드는 `consumer_ref`, `relationship`, `declared_version_range`, `observed_version`, `impact=none|compatible_update|migration_required|blocked_unknown`, `migration_guide_ref`, `owner`, `evidence_refs`, `coverage`다.

`migration_requirement`는 다음 값만 사용한다.

- `none`: current consumer가 변경 없이 호환됨이 증명됨
- `recommended`: additive/compatible이지만 새 기능 채택이나 deprecation 해소를 권장
- `required`: breaking change 또는 지원 window 종료 전에 consumer 변경 필요
- `blocked_unknown`: 소비자 또는 의미 coverage가 불충분해 결론 불가

### compatibility 기간과 제거 조건

deprecated lifecycle의 ID와 상태는 M5가 소유하고, 6단계는 compatibility window와 완료 evidence를 평가한다.

`active -> deprecated -> removed` 전이는 다음을 요구한다.

1. deprecation 시작 version과 마지막 지원 version을 명시한다.
2. replacement 또는 의도적 무대체 사유와 migration guide를 연결한다.
3. declared·observed consumer별 minimum supported version과 전이 상태를 기록한다.
4. alias/shim이 있으면 finite expiry와 old/new behavior equivalence check를 둔다.
5. 제거 직전 current complete consumer scan에서 required old reference가 0임을 증명한다.
6. tombstone과 ID 재사용 금지는 M5 규칙을 따른다.
7. M3 pre/post Gate의 exact subject에서 schema·docs·generated·consumer evidence가 함께 통과한다.

기간이 지났다는 이유만으로 자동 제거하거나 소비자가 없다고 추정하지 않는다.

## 의도치 않은 public surface 확대

current export/command/schema root/file entry/config key/error code가 baseline에 없을 때 다음 조건을 모두 만족해야 의도된 `additive` 후보다.

- `ChangePlan.expected_public_surface_delta`에 surface ID와 owner가 명시됨
- visibility policy와 namespace allowlist에 포함됨
- Schema·docs·consumer policy가 같은 PatchSet lineage에 포함됨
- generated output이면 source generator input에서 유래하고 provenance가 일치함
- 새 public symbol이 accidental re-export, wildcard export, debug command, test fixture 또는 private config 노출이 아님

하나라도 불충족하면 classification과 별개로 `star.validation.contract.public-surface-expanded`를 낸다. 즉, binary compatible 가능성이 의도된 공개 약속을 뜻하지 않는다.

## generated source와 원본 drift

generated artifact는 다음 binding을 가져야 한다.

- `generator_id`, registered `ToolDescriptorRef`, generator version/fingerprint
- source input refs와 canonical input hash
- generation options의 secret-redacted fingerprint
- output logical path와 content hash
- generated marker와 owner

source input hash가 달라졌는데 output이 이전 hash이면 M5 `generated_output_stale`, output은 달라졌는데 source/generator provenance가 없으면 `generated_output_unowned` consistency status를 사용한다. generated file 직접 편집은 새 status를 만들지 않고 `star.validation.registry.generated-direct-edit` Diagnostic으로 진단한다. generated output을 정본으로 source에 역반영하거나 doctor가 generator를 실행해 자동 수리하지 않는다.

## 문서 실행 가능성과 drift

### `DocumentationSnapshot`

각 `DocumentationEntry`는 `document_ref`, `logical_path`, `entry_kind`, source range, normalized target, policy/ref, observation, evidence와 fingerprint를 가진다. `entry_kind`는 다음과 같다.

- `link`, `anchor`
- `command`, `command_output`
- `code_snippet`
- `config_example`
- `schema_reference`, `generated_reference`
- `assumption`

snapshot은 `subject_binding`, `document_sources[]`, 정렬된 `entries[]`, `catalog_snapshot_ref`, `schema_manifest_ref`, `generated_manifest_refs[]`, entry-kind별 `coverage`, `limitations[]`, `diagnostic_refs[]`, `content_fingerprint`를 가진다. parsed entry 0건은 문서가 실제로 비어 있음이 source inventory로 확인된 경우에만 complete다.

각 entry의 `DocumentationCheckRecord`는 `rule_ref`, `expected`, `observed`, `outcome=pass|fail|not_run|unknown`, `execution_ref`, `evidence_refs[]`, `limitation`을 가진다. 실행 정책이 없는 snippet, unsafe command와 network가 필요한 link는 존재 자체만으로 pass가 아니며 해당 required 수준에 따라 `not_run|unknown`으로 남는다.

### 결정적 검사 기준

| 대상 | pass 기준 | 실행·보안 경계 |
|---|---|---|
| local link | repository logical path로 해석되고 target이 current snapshot에 존재 | URL fetch는 별도 network policy 없이는 하지 않음 |
| anchor | target 문서의 canonical GitHub-style heading anchor와 case-sensitive 일치 | 렌더러 차이가 있으면 `unknown` |
| command | 문서 text가 typed candidate로 parse되고 exact registered `TaskDescriptor`/`ToolDescriptor` signature와 일치 | raw shell text 실행 금지 |
| command behavior | safe introspection 또는 disposable fixture의 exit/output/schema가 문서 기대와 일치 | mutation·network·secret 요구 시 `not_run`/review |
| code snippet | language, wrapper/context, compile/parse/check policy와 expected result가 선언됨 | 선언 없는 snippet을 임의 실행하지 않음 |
| config example | target `schema_ref`·version·precedence가 있고 parse/schema/unknown-key 검사를 통과 | 실제 사용자 config에 쓰지 않음 |
| Schema reference | ID/version/hash와 current generated Schema가 일치 | generated Schema 직접 수정 금지 |
| generated reference | source/generator/input/output provenance와 content hash가 일치 | doctor 자동 재생성 금지 |

문서의 CLI 실제 동작은 command 등록 signature, parser schema, safe probe의 exit code·machine output과 비교한다. `--help`만 맞는다고 behavior를 증명하지 않는다. 반대로 실행이 unsafe하면 문서를 실패로 꾸미지 않고 `not_run`과 이유를 보존한다.

`command_output`은 volatile timestamp, temp path, tool patch version처럼 허용된 normalization만 적용한다. error code, field name, exit code, required option과 의미 있는 순서는 삭제하지 않는다.

### contract 변경의 동시 변경 조건

다음 companion set은 하나의 accepted `contract_change_group_id`와 before/after subject lineage에서 함께 평가한다. 같은 Project의 항목은 한 `ChangePlan`·한 M4 `PatchSet`에 있어야 한다.

1. public source 또는 canonical declaration
2. machine-readable Schema/file-format descriptor
3. generated reference와 provenance manifest
4. 사용자 문서·CLI reference·config example
5. compatibility classification·version/window metadata
6. `migration_requirement=required`인 모든 consumer의 migration guide
7. M5 declaration·consumer/lifecycle 변경이 있는 경우 Registry manifest

required 항목이 적용 대상이 아니면 `not_applicable` 사유와 rule evidence가 있어야 한다. 누락은 `star.validation.contract.companion-change-missing`이며 post Gate를 차단한다. 다른 Project의 companion은 같은 group 아래 별도 project `ChangePlan`·migration guide requirement로 연결하지만 9단계 전에는 read-only `planned|blocked_external|not_required` 상태만 기록한다. cross-project 실제 적용이 필요한 breaking removal은 9단계 전 완료할 수 없으며 자동 적용하지 않는다.

## config key lifecycle과 사용 추적

### 두 축을 분리한다

요구된 여섯 용어를 하나의 상태 machine으로 잘못 모델링하지 않는다.

- declaration/lifecycle 축: `declared=true`이면 M5 declaration이 존재하고 그 실제 status는 `active|deprecated|removed`다. 전이 자체는 `active -> deprecated -> removed`를 따른다.
- coverage/usage 관찰: `documented`, `read`, `overridden`

한 key는 `declared+active+documented+read+overridden`일 수 있다. `declared`는 declaration 존재, `active|deprecated|removed`는 M5 lifecycle이고 나머지는 current snapshot의 관찰이다. `declared`라는 새 M5 status를 만들지 않는다.

### `ConfigKeyTrace`

| 필드 | 설명 |
|---|---|
| `declaration_ref` | M5 config-key declaration과 lifecycle |
| `schema_ref` | type, default, required, unknown-key policy |
| `documentation_refs[]` | reference와 example coverage |
| `reader_bindings[]` | typed source reader·package·runtime scope |
| `effective_config_paths[]` | `EffectiveConfig.provenance`의 source kind와 precedence path |
| `override_observations[]` | override source kind·location ref·presence만, 값은 저장하지 않음 |
| `consumer_refs[]` | 직접·간접 소비자와 version 범위 |
| `lifecycle_status` | `active`, `deprecated`, `removed` 등 M5 값의 참조 |
| `coverage` | declaration/docs/reader/override/consumer 각각의 complete 상태 |

`overridden`은 CLI·project·user·default 등 provenance를 뜻하며 secret 또는 실제 config value를 evidence에 복사하지 않는다.

### 판정

- 선언됐지만 current complete semantic reader coverage에 reader가 없고 generated/test-only 예외도 없으면 `star.validation.config.key-unused`다.
- text search 또는 partial adapter만 있으면 unused를 확정하지 않고 suspected finding 또는 `HUMAN_REVIEW`로 남긴다.
- runtime이 읽지만 declaration/Schema가 없으면 unmanaged key Diagnostic과 M5 `candidate`를 만든다. 자동 등록하지 않는다.
- active/deprecated key가 문서에 없으면 `key-undocumented`, removed key를 reader/docs/example이 참조하면 `removed-key-used`다.
- deprecated key 사용은 replacement·window·migration guide와 함께 평가한다.
- environment variable은 이름, owner, required/optional, scope, redaction policy와 presence만 관찰한다. 문서 없는 변수는 `environment-variable-undocumented`이며 값은 절대 출력·저장하지 않는다.

## assumption drift

자연어 전체를 사실 DB로 추측 변환하지 않는다. 결정적으로 검사할 주장은 `AssumptionSpec`으로 명시한다.

| kind | 선언 예 | 관찰 |
|---|---|---|
| `file_exists` | logical path, file kind, optional hash | current source inventory |
| `command_available` | registered task/tool ID와 signature | Catalog·read-only probe |
| `version_constraint` | subject ID와 accepted range | manifest/tool/schema observation |
| `platform_support` | OS family, arch, filesystem capability | `EnvironmentSnapshot` |
| `environment_capability` | case sensitivity, encoding, line ending, path length 등 | read-only doctor probe |

각 spec은 `assumption_id`, owner, docs refs, severity, expected value/constraint, observation method, freshness와 evidence requirement를 가진다. 실제가 다르면 `assumption-drift`; probe가 등록되지 않았거나 의미 판단이 필요하면 `unknown`/`HUMAN_REVIEW`다.

## project doctor의 read-only 경계

### 허용 진단

doctor는 target project와 system에 대해 다음을 읽을 수 있다.

- OS family/build/architecture와 filesystem capability
- repository logical root, Git revision·status의 요약 fingerprint
- package manifest, package-manager lockfile, toolchain 선언, 주요 task/command descriptor
- 등록된 read-only probe를 통한 tool/package-manager identity·version
- environment variable의 선언 여부와 presence. 값은 읽거나 기록하지 않음
- path separator/drive/UNC/device/reserved name/trailing dot-space/junction·symlink 특성
- case-sensitive lookup과 case collision
- BOM/encoding, CRLF/LF/mixed line ending, `.gitattributes`와 관찰된 `core.autocrlf`
- logical/relative/absolute path length와 long-path capability
- manifest·lockfile·Catalog·Schema·generated provenance hash

읽기 위해 실행하는 프로그램은 exact registered `ToolDescriptor`의 read-only invocation이어야 한다. PATH에서 임의 executable을 찾아 실행하거나 raw shell/script host로 우회하지 않는다.

### 금지 동작

doctor와 `docs_config_environment` Profile은 다음을 수행하지 않는다.

- network access, download, advisory DB refresh
- package install/update/restore 또는 lockfile rewrite
- SDK/toolchain 설치·switch
- Windows registry, PATH, execution policy, code page, long-path policy 변경
- service·scheduler·daemon 생성 또는 실행 상태 변경
- `git clean`, reset, checkout, stash, source/generated/config write
- formatter, generator, migration, build처럼 target tree에 output을 만드는 command 실행
- secret·environment variable·credential·private path의 실제 값 출력·저장

필요한 조치는 Diagnostic의 `manual_remediation`으로만 제시하고, 별도 사용자 승인 없는 action token이나 자동 fix를 만들지 않는다. 설치가 필요하면 `mutation_required`와 `not_run`을 기록한다.

### `ProjectDoctorReport`

필수 필드는 `subject_binding`, `environment_snapshot_ref`, `constraint_evaluations[]`, `toolchain_observations[]`, `manifest_observations[]`, `command_availability[]`, `windows_compatibility[]`, `clean_room_readiness`, `diagnostics[]`, `forbidden_actions_observed[]`, `completeness`, `limitations`다.

doctor 자체가 성공해도 project build/test/contract compatibility가 성공한 것은 아니다. report는 M3 Check evidence의 입력이며 독립 pass authority가 아니다.

## environment fingerprint와 Windows 차이

### `EnvironmentSnapshot`

snapshot은 `subject_binding`, `os_observation`, `filesystem_observations[]`, `path_observation`, `text_environment`, `toolchain_observations[]`, `package_manager_observations[]`, `manifest_observations[]`, `task_descriptor_refs[]`, `environment_contract_presence[]`, `probe_records[]`, `completeness`, `limitations[]`, `environment_fingerprint`를 가진다. 각 probe record는 registered descriptor·actual executable identity·typed arguments·exit/output Schema·started/finished status를 보존하되 raw environment 값은 보존하지 않는다.

| nested record | 필수 내용 |
|---|---|
| `ToolchainObservation` | stable toolchain/runtime ID, 발견 source, declared range, observed version·executable fingerprint, `present\|missing\|version_mismatch\|unknown`, probe/evidence ref |
| `PackageManagerObservation` | ecosystem·manager ID, 발견 source, declared/observed version, executable fingerprint, read-only capability, 상태와 evidence ref |
| `ManifestObservation` | ecosystem, `dependency_manifest\|lockfile\|toolchain\|task` kind, logical path, content hash, owner, manifest-lock relation, completeness |
| `CommandDiscoveryRecord` | project script/name, owning `TaskDescriptor`/`CheckDescriptor` ref, typed invocation availability, `registered\|candidate\|conflicting\|unknown` |
| `EnvironmentContractPresence` | environment variable declaration ref, required/optional, scope와 `present\|absent\|unknown`; 실제 값 없음 |

project script나 package-manager task를 발견해도 자동으로 executable command로 승격하지 않는다. exact registered descriptor가 없는 항목은 `candidate`이며 doctor가 실행하지 않는다.

fingerprint 입력은 다음 canonical 필드다.

- OS family, release/build, architecture
- filesystem/volume capability: case behavior, symlink/junction 지원, long-path capability
- project logical path shape: drive/UNC 여부, depth, normalized segment count와 길이 bucket
- default/observed text encoding, BOM policy, line-ending policy와 mixed-file 관찰
- toolchain·runtime·package-manager ID, version, executable content/signature fingerprint
- package manifest·lockfile·task descriptor hash
- environment variable contract ID와 presence state
- clean-room network/cache policy

fingerprint에서 username, home path, raw absolute target path, temp path, secret value, environment variable value, wall-clock timestamp를 제외한다. 필요한 path 차이는 logical token과 길이·case·경로 종류로 정규화한다.

Windows probe는 다음 차이를 별도 Diagnostic으로 보존한다.

- `C:\\` drive path, UNC, device path, `/`와 `\\` 혼용
- reserved device name, trailing dot/space, colon과 invalid character
- logical case mismatch와 같은 폴더의 case-collision 가능성
- UTF-8/UTF-8 BOM/UTF-16/legacy code page, undecodable byte
- CRLF/LF/mixed와 `.gitattributes` 기대 차이
- 상대·절대 path 길이, segment 길이, long-path opt-in capability
- symlink/junction resolution과 repository root 탈출 가능성

doctor는 `core.autocrlf`, code page, long-path policy를 관찰만 하며 바꾸지 않는다.

## clean-room 재현 계약

### readiness와 실제 실행 분리

`clean-room readiness`는 현재 환경에서 명세 완전성과 prerequisite 존재 여부를 읽기 전용으로 진단한다. 실제 `clean-room validation`은 사용자가 이미 준비한 disposable workspace/environment에서 M3가 선택한 Check로 별도 실행한다.

6단계 doctor는 disposable 환경을 생성하거나 package를 내려받지 않는다. 실제 clean-room runner도 v1 고정 정책 `dependency_download=deny`, `package_install=deny`, `system_mutation=deny`를 완화하지 않는다. missing dependency는 실패 원인이지 자동 설치 신호가 아니다. 대상 프로그램의 network 동작을 검사해야 하면 별도 `test_network_policy`·PermissionPlan을 사용하며 dependency/package download와 섞지 않는다.

### `CleanRoomSpecification`

필수 필드는 다음과 같다.

- exact source/release artifact revision과 integrity hash
- target OS family/build range, architecture, filesystem capability
- toolchain/runtime/package-manager ID와 accepted version/hash
- package manifest·lockfile refs와 integrity hash
- registered task/check ID, 순서, timeout, resource bound와 expected artifact/result
- required environment variable name·scope·presence·secret provider reference. 값 없음
- `test_network_policy`, 고정 `dependency_download=deny`, `package_install=deny`, `system_mutation=deny`, cache state(`empty`, `preprovisioned`, `forbidden`)와 관찰용 package source identity
- path kind/depth/length, case, encoding, line-ending constraint
- writable disposable output roots와 보존/redaction policy
- 금지 행동: download/install/system change/source rewrite/credential export

`CleanRoomReadiness`는 `ready`, `not_ready`, `unknown`, `not_required`다. required 필드 누락, unregistered command, mutable source, lockfile 불일치, 필요한 tool 부재는 `not_ready`; 의미 판단이나 probe 부재는 `unknown`/`HUMAN_REVIEW`다.

## CLI-only 목표 surface

다음은 구현 시 Catalog에 등록해야 할 목표 command다. 현재 실행 가능한 명령 예시로 해석하지 않는다.

| command ID | 목적 | source/system effect |
|---|---|---|
| `contract.snapshot` | explicit baseline/current surface snapshot | 없음, derived evidence write만 |
| `contract.compare` | compatibility·consumer·migration report | 없음 |
| `docs.check` | docs/config/generated/assumption drift 검사 | 없음 |
| `config.trace` | key lifecycle·reader·override provenance 조회 | 없음 |
| `project.doctor` | environment/toolchain/manifest read-only 진단 | 없음 |
| `environment.fingerprint` | redacted canonical environment snapshot | 없음 |
| `clean-room.readiness` | 명세와 현재 prerequisite 비교 | 없음 |

각 command는 `--project`, exact subject ref, `--format json|text`를 받고 versioned contract를 출력해야 한다. `--fix`, `--install`, `--download`, `--configure-system` 옵션은 제공하지 않는다. derived evidence 저장은 `state.writer`를 거치며 target checkout에는 쓰지 않는다.

## M3 검사와 판정 결합

### Profile·Rule 소유권

- `api_contract_change`는 B04 compatibility·consumer·migration·Registry·public expansion 검사를 required로 선택한다.
- `docs_config_environment`는 B07 docs/config/assumption/doctor/clean-room readiness 검사를 required로 선택한다.
- Profile은 M2에서 ValidationPlan을 만들 때만 검사를 선택한다. M3 runner나 doctor가 검사를 추가·삭제하지 않는다.

### exact evidence binding

6단계 required evidence는 기존 M3 subject binding에 다음 fingerprint/ref를 더한다.

- `ProjectContractManifest`와 baseline approval
- baseline/current `ContractSurfaceSnapshot`
- current `ManagedRegistrySnapshot`과 `RegistryConsistencyRecord` set
- `DocumentationSnapshot`
- `EnvironmentSnapshot`과 `ProjectDoctorReport`
- applicable `CleanRoomSpecification`과 readiness/result
- package manifest·lockfile·toolchain observation
- companion change set과 consumer migration coverage

하나가 current source/config/Catalog/Tool/environment와 다르면 기존 result는 stale이다.

### pass·block·review

- missing/stale/partial/unverified required evidence, confirmed breaking인데 migration/window 누락, companion change 누락, doctor side effect 시도는 `block`이다.
- 결정적 검사가 complete하고 정책을 모두 만족하면 `auto_pass` 후보가 된다.
- public 의미, undocumented support promise, dynamic consumer, ambiguous Schema/CLI compatibility처럼 사람이 해석해야 하는 경우 `HUMAN_REVIEW`다.
- `HUMAN_REVIEW`는 pass로 정규화하지 않고 exact report fingerprint·질문·선택지·영향을 ReviewPack에 넣는다.

## Diagnostic 최소 namespace

정확한 severity·confidence·fingerprint·suppression은 [오류와 진단 계약](errors-and-diagnostics.md)을 따른다.

| 영역 | required rule ID |
|---|---|
| compatibility | `star.validation.contract.baseline-missing`, `star.validation.contract.breaking-change`, `star.validation.contract.unknown-change`, `star.validation.contract.consumer-unverified`, `star.validation.contract.migration-guide-missing`, `star.validation.contract.companion-change-missing`, `star.validation.contract.deprecation-window-invalid`, `star.validation.contract.public-surface-expanded` |
| docs | `star.validation.docs.broken-link`, `star.validation.docs.broken-anchor`, `star.validation.docs.command-unregistered`, `star.validation.docs.command-signature-drift`, `star.validation.docs.command-unsafe`, `star.validation.docs.snippet-invalid`, `star.validation.docs.snippet-unverified`, `star.validation.docs.config-example-invalid`, `star.validation.docs.schema-drift`, `star.validation.docs.generated-reference-drift`, `star.validation.docs.assumption-drift` |
| config | `star.validation.config.key-undocumented`, `star.validation.config.key-unused`, `star.validation.config.override-untracked`, `star.validation.config.deprecated-key-used`, `star.validation.config.removed-key-used`, `star.validation.config.environment-variable-undocumented` |
| environment | `star.validation.environment.toolchain-missing`, `star.validation.environment.tool-version-mismatch`, `star.validation.environment.package-manager-mismatch`, `star.validation.environment.lockfile-drift`, `star.validation.environment.command-unavailable`, `star.validation.environment.path-case-collision`, `star.validation.environment.encoding-mismatch`, `star.validation.environment.line-ending-mismatch`, `star.validation.environment.path-length-risk`, `star.validation.environment.fingerprint-drift`, `star.validation.environment.clean-room-unverified`, `star.validation.environment.mutation-required` |

같은 subject·location·rule·canonical message parameters는 같은 fingerprint를 가져야 한다. local absolute path, username, secret과 volatile version output은 fingerprint parameter에 넣지 않는다.

## 7단계 dependency·security 검사 인계

`DependencySecurityInputManifest`는 결과나 vulnerability 판정이 아니라 발견 근거만 전달한다.

| 필드 | 내용 |
|---|---|
| `subject_binding` | project/workspace/source exact revision |
| `dependency_manifests[]` | ecosystem, logical path, hash, declared manager |
| `lockfiles[]` | kind, logical path, hash, completeness, manifest relation |
| `toolchain_observations[]` | compiler/runtime/SDK ID·version·fingerprint·provenance |
| `package_manager_observations[]` | manager ID·version·read-only capability·provenance |
| `task_and_check_refs[]` | dependency build/test/audit에 사용할 registered descriptors |
| `environment_snapshot_ref` | OS/arch/filesystem/network/cache fingerprint |
| `source_classification` | source, generated, vendored, cache, unknown |
| `coverage` | complete/partial/unverified와 limitation |
| `freshness` | source/config/Catalog/Tool/environment binding |

6단계는 advisory DB를 내려받거나 package graph를 online resolve하거나 vulnerability·license 결과를 발명하지 않는다. [7단계 정본](failure-security-and-dependency-maintenance.md)은 이 manifest의 completeness를 preflight하고, 부족하면 자체 결과를 pass로 만들지 않는다.

## 구현 순서

제품 구현 승인이 생기면 다음 순서를 지킨다.

1. 여덟 top-level contract의 Schema v1, invalid/future fixture와 canonical hash 규칙
2. `.star-control/contracts.toml` loader와 M5 ref resolver를 read-only port로 구현
3. baseline/current snapshot pure normalizer와 kind별 comparator corpus
4. consumer impact·migration/window·public expansion pure evaluator
5. DocumentationSnapshot parser와 registered command/config/generated/assumption evaluator
6. ConfigKeyTrace와 secret-redacted provenance evaluator
7. EnvironmentSnapshot read-only probe port와 Windows fixture
8. ProjectDoctorReport·clean-room readiness pure evaluator
9. M2 Profile selection, M3 Check/evidence/Gate와 CLI-only vertical slice
10. 후속 7단계 input manifest handoff conformance

각 단계는 fake port와 fixture부터 시작한다. 실제 executable probe, build/test 또는 disposable clean-room 실행은 마지막 adapter 단계이며 install/download/system mutation을 추가하지 않는다.

## 완료 조건

- 모든 public surface가 explicit baseline/current와 evidence로 비교된다.
- change classification이 kind별 규칙, 소비자와 migration requirement에 연결된다.
- deprecated window와 제거 조건이 M5 lifecycle·consumer coverage·migration guide·M3 Gate에 연결된다.
- public surface 의도치 않은 확대와 generated source drift를 별도 판정한다.
- 문서 command·link·anchor·snippet·config example·Schema·generated reference의 실행 가능 기준이 있다.
- config key의 lifecycle과 documented/read/overridden 관찰을 혼합하지 않고 끝까지 추적한다.
- doctor와 clean-room readiness가 read-only이며 금지 동작을 자동 수행하지 않는다.
- Windows path·case·encoding·line ending·path length가 redacted environment fingerprint에 반영된다.
- 의미 판단을 AI 없이 `HUMAN_REVIEW`로 보존한다.
- 후속 7단계가 사용할 manifest·lockfile·toolchain·environment 입력의 provenance·coverage·freshness가 있다.
