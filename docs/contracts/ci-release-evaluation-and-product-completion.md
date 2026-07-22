# 10단계 CI·Release·평가·최종 제품 완성 계약

## 상태와 목적

이 문서는 Star-Control 10단계인 **CI·Release·배포 준비, 규칙 평가와 최종 제품 완성**의 의미·상태·Gate·구현 순서 정본이다. M10 release/evaluation engine의 현재 상태는 **문서 설계 확정, 제품 구현 전**이다. 다만 P-0026이 M10과 분리된 설치 transport 수직 Slice로 Inno Setup package·설치 기록·Codex Plugin 렌더링을 구현했다. 이 bounded 구현은 M10 상태기계·CI·공개 배포·평가가 구현됐거나 release `ready`라는 근거가 아니다.

10단계는 0~9단계를 다시 구현하거나 별도 release engine으로 복제하지 않는다. 앞 단계가 만든 current source·계획·검사·artifact·호환성·복구·원격 관찰을 같은 신원으로 묶어 다음 두 질문에 답한다.

1. 지금 검증한 source와 실제 배포 후보 byte가 같은가.
2. Star-Control의 Rule·Check·Profile·Recipe 자동화가 실제 결함을 더 잘 찾고 재작업 시간을 줄이는가.

정확한 `ReleaseManifest`·`EvaluationRun` wire field와 evidence version은 [검사·완료·증거 계약](validation-and-evidence.md), version 전이는 [Version과 Migration 계약](versioning-and-migrations.md), Windows 설치 운영은 [설치와 공개 배포](../operations/installation.md), technical manifest·경로·Codex 등록은 [Windows 설치와 Codex 연동 계약](windows-installation-and-codex-integration.md), Package 위치는 [Repository·Package 구조](../architecture/repository-layout.md)가 소유한다. 이 문서는 release·evaluation application 흐름, 상태기계, Gate, 평가 판정과 최종 제품 감사 기준을 소유한다.

## 0~9단계 선행 정본 gap matrix

10단계 설계 전에 다음 연결을 live 문서 기준으로 확인했다. `설계 연결`은 정본이 존재하고 다음 단계 입력이 정의됐다는 뜻이며 제품 구현 완료를 뜻하지 않는다.

| 사용자 단계 | 정본 | 10단계가 소비하는 것 | 설계 연결 | 현재 제품 상태 |
|---:|---|---|---|---|
| 0 | [공통 개발 관리와 로컬 관리 DB](development-management.md) | stable ID·fingerprint, Controller single writer, source/DB/evidence 분리, backup·rebuild | 연결됨 | P0 첫 수직 Slice만 구현, 전체 lifecycle은 남음 |
| 1 | [Project Catalog와 Code Index](project-catalog-and-code-index.md) | Project·Checkout·revision, source inventory, toolchain·dependency, freshness | 연결됨 | 설계 확정·구현 전 |
| 2 | [변경 계획·영향·affected 선택](change-planning-and-impact.md) | TaskSpec·ScopeRevision·ChangePlan·ValidationPlan과 release risk path | 연결됨 | 설계 확정·구현 전 |
| 3 | [공통 검증·품질 Gate](../features/common-validation-gate.md) | current evidence binding, ratchet, validator guard, GateDecision | 연결됨 | 설계 확정·구현 전 |
| 4 | [Patch·Refactor·codemod](safe-patch-and-codemod.md) | immutable PatchSet·actual ChangeSet·post Gate·복구 | 연결됨 | 설계 확정·구현 전 |
| 5 | [Managed Registry](managed-symbol-registry.md) | stable ID·lifecycle·consumer·generated ownership | 연결됨 | 설계 확정·구현 전 |
| 6 | [계약 호환성·문서·설정·환경](contract-compatibility-and-environment.md) | public compatibility, docs/config metadata, clean-room readiness | 연결됨 | 설계 확정·구현 전 |
| 7 | [실패·보안·의존성 유지보수](failure-security-and-dependency-maintenance.md) | recovery, supply-chain·license·external-data freshness, Maintenance Radar | 연결됨 | 설계 확정·구현 전 |
| 8 | [Migration·성능·언어·플랫폼](migration-performance-and-platform.md) | install/state migration, restore, supported platform와 runtime evidence | 연결됨 | 설계 확정·구현 전 |
| 9 | [CrossRepo ChangeBundle](cross-repo-change-bundle.md) | project별 immutable source revision·artifact·Gate·remote ref를 가진 `ChangeBundleReleaseHandoff` | 연결됨 | 설계 확정·구현 전 |

0~9단계 정본 파일·읽는 순서·로드맵·원장의 **단계 누락은 없다**. 다만 10단계 시작 시점의 gap은 다음과 같으며, 이 문서와 관련 정본 변경이 이를 설계 수준에서 닫는다.

| gap | 기존 상태 | 10단계 설계 결론 |
|---|---|---|
| 전용 10단계 의미 정본 | B09·D02·D03와 P8·P9에 요약만 존재 | 이 문서를 release/evaluation application 정본으로 추가 |
| 검사 계층 | quick·target·full·release의 exact 경계 없음 | 네 계층의 입력·환경·Gate·evidence를 고정 |
| release 상태 | `ready`와 `published`만 구분, `approved` 부재 | `ready`, `approved`, `published`를 분리한 상태기계 고정 |
| artifact 승격 | source revision·digest 원칙은 있으나 promotion protocol 부족 | build-once, immutable artifact set, byte 재사용과 재build 금지 고정 |
| `ci_release_deploy` Profile | 최종 16개 표의 요약 행만 존재 | required Check·Gate·approval·completion metadata를 상세화 |
| M11 Rust style conformance | `rust_style_auto_fix` 의미 정본만 있고 제품 type·adapter·Corpus 없음 | P9 전에 stable toolchain·coverage·Patch/Gate·CLI-only x64와 ARM64 cross-target evidence v6 요구 |
| EvaluationRun | 큰 단위 비교 field만 존재 | Rule·Check·Profile·Recipe별 실행 시간·finding·실결함·FP·flaky·suppression과 context 분리 |
| lifecycle | ManagedDeclaration lifecycle은 있으나 Rule·Check·Profile·Recipe 노후화 경로 부족 | Catalog item deprecation·replacement·migration·tombstone 규칙 고정 |
| 최종 소유권 감사 | 여러 정본에 분산 | A01~D03, 공통 관리 자산, 정본·Writer·제외 기능을 한 matrix로 연결 |

## 범위와 제외 범위

### 포함

- `local_quick`, `target`, `full`, `release` 검사 계층과 promotion 조건
- 같은 Task ID·source revision·tool version·config fingerprint·resolved Profile identity 유지
- clean Windows x64 Stable build·test·package·install lifecycle과 ARM64 Preview cross-build·simulation evidence
- source revision과 immutable artifact digest 연결, build-once·verify·promote
- release file list, package dry-run, version·changelog·metadata·license 검증
- 적용 필요성을 먼저 판정하는 SBOM·provenance·signing evidence
- install·safe_default first run·update·rollback·uninstall과 사용자 자료 보존
- `ready`, `approved`, `published`가 다른 release 상태인 상태기계
- publish·deploy·원격 effect의 action별 승인과 after-state 확인
- Rule·Check·Profile·Recipe 평가, baseline/candidate 비교와 deprecation·migration
- Maintenance Radar와 EvaluationRun 연결
- CLI-only 효용과 Codex 연동 효용의 분리 측정
- A01~D03·최종 16 Profile·Package·정본·Writer·제외 기능의 최종 감사

### 제외

- 자체 CI runner, build system, package manager, installer engine, signing service, artifact registry 또는 deploy platform 구현
- compiler, scanner, debugger, profiler, vulnerability DB, license DB와 hosting provider API의 core 재구현
- 승인 없는 build farm·remote CI·registry·release·deploy·account effect
- 같은 artifact라고 주장하면서 release 단계에서 source를 다시 build하는 경로
- 실제 원격 after-state를 확인하지 않은 `published`·`deployed` 표시
- 추정 가격, 추정 token, 누락된 시간 또는 미측정 finding을 0으로 채우는 행위
- pass율을 높이기 위한 Rule severity 하향, Check 제거, fixture 기대값 자동 갱신, suppression 확대
- CLI-only core path에서 Codex·다른 AI·OpenAI API를 필수 dependency로 만드는 행위
- browser UI, 자체 예약 release·평가 실행과 background deployment scheduler

## 핵심 불변식

1. **동일 subject**: 모든 계층은 같은 Task ID, project별 source revision, dirty state, config, Catalog, logical Tool ID/version/descriptor와 resolved Profile fingerprint를 사용한다. architecture별 executable hash는 달라도 되지만 declared platform artifact여야 하고 그 외 차이는 새 ValidationPlan·candidate다.
2. **build once**: 배포 후보 byte는 한 번 생성하고 hash로 봉인한다. 검증·승격·publish는 그 byte를 재사용하며 release용 재build를 하지 않는다.
3. **source와 artifact 연결**: artifact마다 exact source revision set, build invocation, toolchain·environment와 SHA-256이 있어야 한다.
4. **상태 분리**: `ready`는 검증 완료, `approved`는 외부 effect 허가, `published`는 실제 원격 결과 확인이다. 어떤 상태도 다른 상태를 암시하지 않는다.
5. **프로젝트별 사실 보존**: 여러 Project release는 각 source revision·Gate·artifact를 유지한다. 하나의 synthetic revision으로 합치지 않는다.
6. **Gate 단일화**: release adapter, CI provider, installer와 signer는 결과 observation·receipt만 반환한다. Controller가 M3 Gate를 재사용해 유일한 상태 projection을 쓴다.
7. **새 악화 우선**: 기존 부채를 한 번에 없애는 것보다 candidate가 새 finding을 만들거나 severity·coverage를 악화시키지 않는 ratchet을 우선한다.
8. **검증기 보호**: candidate가 더 높은 통과율을 보이더라도 Rule·Check·Profile·Corpus·severity·suppression을 약화해 얻은 결과면 accept할 수 없다.
9. **측정 정직성**: ground truth, duration, usage와 monetary cost가 없으면 `unknown|unavailable|not_comparable`로 남기며 0으로 만들지 않는다.
10. **자료 보존**: update·deploy·rollback·uninstall 실패 중에도 user config, management store, `.ai-runs`와 복구 artifact를 삭제하지 않는다.
11. **CLI-first**: release planning, deterministic Check, evaluation과 상태 조회는 Codex 없이 가능하다. Codex는 선택적인 작업 소비자·검토 context다.
12. **정본 우선**: source·manifest·Catalog·policy가 canonical이고 DB/index는 derived state다. ReleaseManifest·EvaluationRun 결과를 source 설정으로 자동 역쓰기하지 않는다.

## 핵심 용어와 상태 축

| 용어 | 의미 |
|---|---|
| release subject | Task ID와 project별 immutable source revision set |
| release candidate | 한 subject에서 한 번 build·package되어 artifact set digest로 봉인된 후보 |
| artifact set digest | 정렬된 artifact entry의 logical name·role·architecture·size·SHA-256을 canonical hash한 값 |
| promotion | artifact byte를 바꾸지 않고 같은 digest의 승인된 channel·visibility metadata만 전진시키는 동작 |
| verification layer | `local_quick`, `target`, `full`, `release` 중 하나 |
| release readiness | required release Gate와 evidence가 current·complete한 상태 |
| publish approval | exact manifest revision·artifact set digest·channel·remote target에 결합한 single-use 사용자 승인 |
| published proof | provider after snapshot이 exact version·source·artifact digest를 현재 상태로 확인한 근거 |
| evaluation subject | stable Rule·Check·Profile·Recipe 또는 routing/policy ID와 version·definition fingerprint |
| evaluation context | `cli_only` 또는 `codex_integrated`; 서로 다른 context를 같은 cohort로 합치지 않음 |
| adjudicated defect | 사람이 승인했거나 재현 가능한 deterministic evidence가 실제 결함으로 확정한 finding |
| false positive | current rule 의미·subject에서 결함이 아니라는 review 근거가 있는 finding; 단순 suppression과 다름 |

release 상태는 최소 다음 값을 사용한다.

| 상태 | 의미 | 외부 effect |
|---|---|---|
| `draft` | version·입력·정책을 조립 중 | 없음 |
| `candidate` | artifact set이 봉인됐지만 release Gate 미완료 | 없음 |
| `blocked` | required evidence·호환성·검사가 불충족 | 없음 |
| `ready` | 모든 release readiness Gate가 current·complete | 없음 |
| `approved` | exact publish/deploy action 승인이 current | 아직 없음 |
| `publishing` | 승인된 remote operation이 시작됨 | 진행 중 |
| `publish_outcome_unknown` | remote effect 여부를 after snapshot으로 확정할 수 없음 | 새 effect 금지 |
| `published` | after snapshot이 exact remote result를 확인 | 확인됨 |
| `rollback_required` | install/update/deploy 실패 뒤 복구가 필요 | 새 publish 금지 |
| `withdrawn` | 승인된 withdrawal·superseding release가 확인됨 | historical evidence 유지 |

`ready -> approved -> publishing -> published`를 건너뛸 수 없다. approval이 만료·취소·stale이면 새 manifest revision의 current projection은 `ready` 또는 `blocked`로 돌아가며 과거 approval event를 삭제하지 않는다. `published`도 설치 성공이나 deploy 성공을 자동 의미하지 않으며 deploy target별 verification을 별도로 가진다.

## 입력 document graph

```text
TaskSpec·ScopeRevision
  + ProjectCatalogSnapshot·CodeIndexSnapshot
  + project별 ChangePlan·ValidationPlan
  + project별 GateDecision·EvidenceBundle
  + CompatibilityReport·Migration/Restore evidence
  + SupplyChainSnapshot·ExternalDataSnapshot
  + ChangeBundleReleaseHandoff 또는 single-project release input
  + EffectiveConfig·CatalogSnapshot·environment별 ToolRegistrySnapshot
  -> release preflight
  -> clean build/package candidate
  -> immutable artifact set + ReleaseManifest v2 candidate
  -> target/full/release verification
  -> ready
  -> exact ApprovalRequest
  -> approved
  -> RemoteOperationRecord
  -> after RemoteStateSnapshot
  -> published | publish_outcome_unknown | rollback_required

Rule·Check·Profile·Recipe baseline
  + candidate definition
  + versioned Corpus/eval case set
  + case별 ValidationRun·Diagnostic·Finding·adjudication
  + rework·failure·duration·verified CostRecord
  + MaintenanceRadarSnapshot
  -> EvaluationRun v2
  -> keep | trial | accept | reject | needs_review
  -> review된 Catalog/policy change 또는 deprecation migration
```

single-project release도 9단계 handoff를 거짓으로 만들지 않는다. application service가 같은 `ProjectReleaseInput` shape를 current project source·artifact·Gate에서 직접 만들되 `source_kind=single_project`를 기록한다. multi-project이면 `ChangeBundleReleaseHandoff`의 각 ref를 current 상태에 다시 확인하며 handoff의 approval·freshness·success를 복사하지 않는다.

## 동일 identity envelope

모든 `ValidationRun`, CI job observation, artifact와 release Gate는 다음 identity를 가져야 한다.

| 필드 | 요구사항 |
|---|---|
| `task_spec_ref`, `task_id` | 모든 계층에서 동일 |
| `scope_revision_ref` | accepted current revision |
| `project_source_revisions` | ProjectId별 commit/content revision과 dirty=clean proof |
| `change_bundle_handoff_ref` | multi-project일 때 current handoff revision |
| `validation_plan_ref` | 계층·phase가 materialize된 current plan |
| `config_fingerprint` | EffectiveConfig exact hash |
| `catalog_snapshot_ref` | Task·Tool·Check·Rule·Profile version/hash 집합 |
| `logical_toolset_fingerprint` | 모든 계층·architecture가 공유하는 Tool ID·version·descriptor/protocol hash 집합 |
| `tool_registry_snapshot_refs` | environment별 실제 executable identity; architecture 외 undeclared delta 금지 |
| `profile_refs` | `ci_release_deploy`와 change class Profile closure |
| `profile_resolution_fingerprint` | parent·Rule·Check·Gate·approval metadata를 합친 hash |
| `environment_ref` | OS·architecture·toolchain·filesystem·network/cache policy |
| `invocation_ref` | typed TaskInvocation, arguments, cwd binding, timeout·resource limit |

CI provider의 run ID, branch 이름, tag와 workflow name은 보조 provenance일 뿐 이 identity를 대체하지 않는다. local과 CI의 Task ID가 같아도 source revision, config fingerprint 또는 logical Tool version이 다르면 같은 candidate evidence로 합치지 않는다. x64·ARM64 Tool executable SHA-256 차이는 같은 descriptor가 선언한 platform artifact relation과 version 일치를 증명한 경우에만 허용한다.

## 검사 계층

### 계층 matrix

| 계층 | 실행 시점 | 최소 환경 | 필수 목적 | release readiness 사용 |
|---|---|---|---|---|
| `local_quick` | 편집 중·commit 전 | current checkout | format·문서 link·Schema drift·가벼운 affected Check로 빠른 feedback | 직접 사용 불가 |
| `target` | stage/PR candidate | current 또는 격리 checkout | M2 selected affected Check, change class Profile, regression과 contract Gate | full 승격의 입력 |
| `full` | main 후보·release 후보 전 | clean disposable Windows | 전체 workspace build·test·lint·docs·contract·security floor, 이전 계층 재현 | release 전 필수 |
| `release` | 봉인된 artifact set | clean x64 Stable 환경·설치 sandbox와 ARM64 Preview cross-build·simulation sandbox | package, file list, metadata, x64 install/update/rollback/uninstall, ARM64 model lifecycle, safe_default, artifact digest·supply-chain·publish preflight | `ready`의 직접 Gate |

낮은 계층 통과는 높은 계층을 대체하지 않는다. 같은 Check가 높은 계층에서 다시 실행되면 낮은 결과를 덮지 않고 별도 `ValidationRun`으로 연결한다. previous success reuse는 exact source/config/Catalog/Tool/environment와 Check invalidation closure가 증명될 때만 가능하며 `release`의 artifact byte 검사·install lifecycle·publish preflight는 항상 새 candidate digest에 대해 실행한다.

### `local_quick`

- source effect가 없는 fast check 또는 현재 변경에 직접 연결된 bounded Check만 실행한다.
- dependency download, remote CI, signing, installer와 publish를 시작하지 않는다.
- failure는 조기 feedback이고 release candidate 상태를 직접 바꾸지 않는다.
- 실행하지 않은 full/release Check를 성공으로 렌더링하지 않는다.

### `target`

- M2가 선택한 exact Check family·scope·fallback을 재사용한다.
- `test_correctness`, `api_contract_change`, `security_supply_chain`, `docs_config_environment` 등 실제 change class Profile closure를 합친다.
- affected scope soundness를 증명하지 못하면 package→workspace→project full로 승격한다.
- candidate source가 바뀌면 기존 target evidence는 stale이다.

### `full`

- user state와 이전 build output이 없는 clean disposable Windows environment를 사용한다.
- locked dependency와 pinned toolchain·TaskDescriptor·ToolDescriptor를 사용한다.
- network/cache 사용 여부와 dependency provisioning source를 environment evidence에 명시한다.
- clean, incremental과 cache hit 결과를 섞지 않는다. release readiness에는 clean mode가 필수다.
- validator guard, Corpus, full contract/Schema/catalog drift와 secret/supply-chain floor를 포함한다.

### `release`

- `ci_release_deploy` Profile의 exact release phase를 사용한다.
- 이미 봉인된 artifact set을 입력으로 package contents·installer·lifecycle을 검증한다.
- artifact를 다시 compile하거나 package byte를 다시 생성해 이전 digest와 같은 것으로 간주하지 않는다.
- 지원 target, version/changelog/license, conditional supply-chain 자료와 rollback plan을 모두 판정한다.
- remote publish 없이도 `ready`까지 갈 수 있다. publish가 없다는 이유로 release readiness 검사를 생략하지 않는다.

## release preflight

candidate build 전 application service는 다음 순서로 fail-closed preflight한다.

1. TaskSpec·ScopeRevision·ReleaseManifest draft와 project set을 확인한다.
2. 모든 Project source가 clean immutable revision인지 확인한다. dirty·uncommitted·untracked source는 release input이 아니다.
3. multi-project이면 ChangeBundleReleaseHandoff의 participant·commit·Gate·artifact ref를 current Project/remote snapshot에 다시 bind한다.
4. required M1~M9·M11 contract version과 evidence version을 확인한다. historical v1/v2/v3/v4/v5 evidence를 v6 final release Gate로 자동 승격하지 않는다.
5. version source, changelog entry, package metadata, license·notice와 compatibility policy를 읽는다.
6. resolved `ci_release_deploy`와 관련 최종 16 Profile closure, Check·Rule·Tool version/hash를 materialize한다.
7. clean Windows x64 Stable environment와 ARM64 Preview cross-build·simulation matrix, network/cache·dependency provisioning과 artifact retention을 고정한다.
8. conditional SBOM·provenance·signing applicability decision과 근거를 고정한다.
9. install·update source version·rollback target·uninstall data policy를 고정한다.
10. publish target이 있으면 remote identity·channel·capability를 read-only로 관찰하되 승인으로 사용하지 않는다.

필수 입력 하나라도 missing·stale·partial이면 build를 시작하지 않거나 `blocked` candidate로 종료한다. architecture policy가 요구하는 `native_unverified`는 ARM64 Preview의 명시적 limitation일 뿐 pass가 아니며 x64 Stable evidence로 사용하지 않는다. 외부 certificate·timestamp provider처럼 required external prerequisite가 없으면 `blocked_external`로 종료한다. preflight 중 source·version·changelog·config를 자동 수정하지 않는다.

## clean Windows 지원 matrix

Star-Control 공개 runtime의 OS baseline은 **Windows 11 24H2 build 26100 이상**이다. `v0.1.0`은 architecture별 support tier를 분리하며 publication destination은 GitHub Releases다.

| target | tier·상태 | build·package evidence | runtime·lifecycle evidence |
|---|---|---|---|
| `x86_64-pc-windows-msvc` | Stable | clean native 또는 approved clean builder의 exact binary·toolchain, signed Runtime·installer와 final manifest | native x64 process·IPC·Controller·CLI·MCP, clean install·safe_default·update·failure rollback·repair·uninstall |
| `aarch64-pc-windows-msvc` | Preview, `native_unverified` | cross-build provenance, PE architecture, signed Runtime·installer와 exact file manifest | installer model과 fake lifecycle. native process·IPC·Controller·CLI·MCP·install 성공은 주장하지 않음 |

각 environment는 OS edition/build, architecture, filesystem capability, line ending/encoding, toolchain/runtime/package manager, dependency source/cache, environment variable presence contract와 runner image identity를 기록한다. username, home/temp 절대 경로, secret value와 mutable wall-clock 값은 fingerprint에 넣지 않는다.

ARM64 실기 부재는 Preview의 `native_unverified` limitation으로 보존하며 x64 Stable 또는 native ARM64 성공으로 바꾸지 않는다. Preview에 요구된 cross-build·architecture·manifest·signature·installer model·fake lifecycle 중 하나라도 없으면 Preview asset과 전체 required package set을 차단한다. ARM64 Stable 승격은 release 한 번의 우회가 아니라 native evidence를 요구하는 versioned policy·문서·compatibility change다.

## build-once와 artifact 승격

### candidate 생성

architecture별 candidate builder는 pinned source와 environment에서 한 번 build·package한다. 결과는 다음 순서로 봉인한다.

1. typed build TaskInvocation과 dependency/toolchain manifest를 고정한다.
2. output root를 새 bounded staging generation으로 만든다.
3. expected artifact role마다 actual byte·size·media type·architecture·SHA-256을 계산한다.
4. package 내부 file manifest와 artifact provenance를 finalize한다.
5. 정렬된 entry로 `artifact_set_digest`를 계산한다.
6. staging generation을 read-only candidate store에 publish한다.
7. ReleaseManifest `candidate` revision과 ArtifactRef를 Controller transaction으로 commit한다.

candidate finalization 전 crash·cancel·output limit은 이전 candidate를 유지하고 새 staging을 incomplete로 남긴다. 같은 candidate ID에 다른 byte를 덮어쓰지 않는다.

### 검증과 promotion

- target/full/release verifier는 artifact ID가 아니라 SHA-256을 다시 계산해 exact byte를 확인한다.
- verification용 download/copy 뒤에도 source store와 local byte의 digest를 모두 비교한다.
- promotion은 artifact byte를 copy-on-write 또는 provider metadata로 다른 channel에 노출할 수 있지만 digest를 바꾸지 않는다.
- timestamp injection, 재압축, re-signing, manifest rewrite처럼 byte가 바뀌면 새 artifact·새 candidate다.
- signing이 package byte를 바꾸는 방식이면 **서명된 byte가 최종 candidate**이며 서명 뒤 모든 package·install 검사를 다시 수행한다. unsigned artifact의 통과를 signed artifact에 상속하지 않는다.
- ready 뒤 source, config, tool, Profile, policy, version, changelog, package file list 또는 artifact byte가 바뀌면 새 candidate를 만든다.

## release file list와 package dry-run

package file manifest entry는 최소 다음 field를 가진다.

| 필드 | 의미 |
|---|---|
| `logical_path` | package root 기준 정규화 경로 |
| `artifact_ref`, `sha256`, `size_bytes` | 실제 포함 byte identity |
| `role` | executable, plugin, catalog, config template, schema, license, notice, metadata, installer resource |
| `architecture` | `neutral`, `x64`, `arm64` |
| `source_owner_ref` | source/generated owner와 generator provenance |
| `install_scope` | program, plugin, user-template, documentation |
| `required` | package 종류별 필수 여부 |
| `license_ref` | 필요한 경우의 license·third-party notice 연결 |

package dry-run은 package를 원격에 publish하거나 system에 install하지 않고 staging root와 archive/installer planned contents를 펼쳐 다음을 검사한다.

- expected file 누락과 undeclared extra file 0건
- x64 package에 ARM64 binary 또는 반대 architecture 혼입 0건
- Plugin package에 runtime binary·secret·user state 포함 0건
- runtime package에 `legacy/`, source `target/`, `.git/`, `.ai-runs/`, local management DB, user config·credential 포함 0건
- executable·DLL·schema·catalog·license·notice의 digest와 manifest 일치
- path escape, duplicate normalized path, Windows case collision, reserved name, long-path policy 위반 0건
- generated Schema·reference의 source/generator/input hash 일치
- installer ownership manifest와 uninstall preserve/delete policy 일치

목표 `dist/` 산출물은 다음 role을 가진다. Windows installer extension은 P-0026에서 Inno Setup `.exe`로 확정했다. Plugin은 installer에 포함된 로컬 Marketplace template을 실제 경로로 렌더링하며, 독립 ZIP 공개 여부는 후속 channel 정책이다.

```text
dist/
├─ star-control-plugin-<version>.zip
├─ star-control-windows-x64-<version>-setup.exe
├─ star-control-windows-x64-<version>.zip        # portable 정책이 승인된 경우만
├─ star-control-windows-arm64-<version>-setup.exe
├─ star-control-windows-arm64-<version>.zip      # portable 정책이 승인된 경우만
├─ checksums.sha256
├─ release-manifest.json
├─ sbom.spdx.json                                # applicability=required일 때
├─ provenance.json                               # applicability=required일 때
└─ signatures/                                   # signing policy가 required일 때
```

조건부 파일을 만들지 않았으면 empty placeholder를 넣지 않고 ReleaseManifest applicability decision과 생략 이유를 기록한다.

## version·changelog·metadata·license

Star-Control 자체 release의 source 정본 목표는 다음처럼 한 곳씩 둔다.

| 정보 | 정본 목표 | release 검사 |
|---|---|---|
| product version | root `Cargo.toml`의 `[workspace.package].version` | runtime crate·binary metadata·Plugin·installer·ReleaseManifest projection이 동일한지 검사 |
| release policy·package set | `packaging/release.toml` 목표 source | channel, version source, package roles, support matrix, supply-chain applicability |
| changelog | root `CHANGELOG.md` | exact version section, user-visible change·migration·known risk 존재 |
| license | root `LICENSE`와 workspace license declaration | package 포함, metadata 값 일치 |
| third-party notice | generated dependency/license input에서 만드는 review 대상 notice | source·version·license provenance와 누락/unknown 상태 |
| installer ownership | `release-manifest.json`, [Windows 설치 계약](windows-installation-and-codex-integration.md), `packaging/windows/star-control.iss` | install/update/uninstall file·state ownership과 preserve policy |

version 값을 여러 manifest에서 독립 정본으로 관리하지 않는다. projection이 필요한 형식은 typed release tool이 canonical version source에서 생성하고 drift를 Gate가 차단한다. changelog의 `Unreleased`만 있는 상태는 public version release ready가 아니다.

license `unknown`, conflicting, stale external data 또는 package entry와 notice coverage 불일치는 clean으로 만들지 않는다. 법률 판단이 필요한 항목은 `HUMAN_REVIEW`이며 scanner의 추측으로 license를 확정하지 않는다.

## SBOM·provenance·signing applicability

SBOM, provenance와 signing은 항상 존재하는 척하지 않고 release target·channel·installer 기술·외부 요구에 따라 각각 판정한다.

| 상태 | 의미 |
|---|---|
| `required` | release policy·배포 채널·계약이 요구 |
| `not_required` | versioned policy와 review 근거로 해당 release에 불필요 |
| `unavailable` | 요구되지만 tool·credential·service·environment가 없음 |
| `incomplete` | 일부 package·dependency·artifact만 coverage |
| `complete` | exact final artifact set에 대한 current 자료 |

규칙은 다음과 같다.

- `v0.1.0` GitHub Releases의 x64 Stable과 ARM64 Preview Runtime·installer는 Authenticode가 `required`다. certificate·timestamp provider 선택과 비용은 실행 전 별도 승인 대상으로 유지한다.
- `required`인데 unavailable/incomplete이면 release Gate는 block한다.
- `not_required`는 누락 field가 아니라 policy version, 이유, reviewer/decision ref를 가진 판정이다.
- SBOM은 final package에 실제 포함된 component와 version을 기준으로 하며 manifest만 보고 포함을 추측하지 않는다.
- provenance는 source revision, builder identity, invocation, dependency/toolchain input과 final artifact digest를 연결한다.
- signing key·token·certificate private material은 config, DB, log, artifact와 ReleaseManifest에 저장하지 않는다. 외부 signer adapter는 signature·certificate chain observation과 receipt만 반환한다.
- signature verification은 final byte에 수행한다. signer 성공 response만으로 signed 상태를 확정하지 않는다.

## release Gate phase

10단계는 M3 validation engine에 다음 phase를 추가한다. phase 이름은 `ValidationPlan`·`ValidationRun`·`GateDecision` v5에서 처음 materialize하며 M11 Rust binding을 포함한 P9 final writer는 compatible v6를 사용한다.

| phase | 핵심 입력 | 성공 조건 |
|---|---|---|
| `release_preflight` | Task/source/version/Profile/policy/support matrix | current·complete, build 시작 가능 |
| `release_build` | clean builder·typed invocation | expected output 생성, artifact set 봉인 |
| `release_verify` | final artifact digest·full checks | x64 Stable native run과 ARM64 Preview cross-build·simulation run이 각 support tier·artifact subject와 일치 |
| `release_package` | package file manifest·metadata·supply-chain applicability | dry-run·digest·license·notice 완전 |
| `release_install_lifecycle` | installer·previous version·user-state fixture | install·safe_default·update·rollback·uninstall 규칙 충족 |
| `release_ready` | 앞 phase Gate·compatibility·remaining risk | deterministic readiness complete |
| `release_publish_preflight` | exact manifest·approval·remote before snapshot | current single-use approval와 target 일치 |
| `release_publish_verify` | adapter receipt·remote after snapshot | exact version/source/artifact/channel 확인 |

`release_ready`는 `release_publish_preflight`를 요구하지 않는다. 공개하지 않고 내부 candidate로 보관할 수 있기 때문이다. 반대로 publish approval이 있어도 `release_ready`를 우회할 수 없다.

Gate aggregation은 project별 Gate를 대체하지 않는다. 한 required Project의 source·artifact·compatibility·migration·rollback evidence가 incomplete이면 전체 release는 block한다. optional package를 빼려면 ReleaseManifest draft부터 package set과 reason을 새 revision으로 만들어야 하며 실패 뒤 임의로 제외하지 않는다.

## ready·approved·published 상태기계

```text
draft
  -> candidate
  -> blocked -----------------------------┐
  -> blocked_external --------------------|
  -> ready                                |
       -> approved                        |
            -> publishing                 |
                 -> published             |
                 -> publish_outcome_unknown
                 -> rollback_required     |
published -> withdrawn                    |
blocked/blocked_external/publish_outcome_unknown/rollback_required -> 새 preflight·approval·operation revision
```

상태 전이 조건은 다음과 같다.

- `candidate`: final artifact set digest와 candidate manifest가 존재한다.
- `blocked_external`: certificate·timestamp provider처럼 required 외부 prerequisite가 없어 local 검증만으로 해소할 수 없다. pass나 approval 상태가 아니며 unsigned Stable로 우회하지 않는다.
- `ready`: 모든 required release Gate가 `AUTO_PASS`, evidence packaging이 complete하고 unresolved external gate가 없다.
- `approved`: `release_publish` 또는 deploy action별 ApprovalRequest가 exact manifest revision, digest, channel, provider, destination, expiry에 결합돼 있다.
- `publishing`: Controller가 approval을 single-use permit으로 소비하고 `RemoteOperationRecord`를 시작했다.
- `published`: provider after snapshot이 exact version, source revision/tag, artifact digest와 visible channel을 확인했다.
- `publish_outcome_unknown`: timeout·connection loss·partial provider response 뒤 after snapshot으로 결과를 확정하지 못했다.
- `rollback_required`: install/update/deploy smoke 또는 observation window에서 rollback trigger가 충족됐다.
- `withdrawn`: 승인된 withdrawal·superseding operation과 after snapshot이 공개 제거·대체 상태를 확인했다.

`publish_outcome_unknown`에서 같은 upload를 자동 retry하지 않는다. 먼저 read-only reconcile을 수행하고, 결과가 없다는 current proof와 새 idempotency/precondition이 있을 때만 새 approval을 받는다.

ReleaseManifest top-level `status`는 candidate와 **주 publication channel**의 lifecycle을 표현한다. publish 이후 deploy 승인 때문에 top-level status를 `published -> approved`로 되감지 않는다. 각 원격 action은 `remote_actions[]`에서 별도로 다음 field를 가진다.

| 필드 | 의미 |
|---|---|
| `remote_action_id`, `action_kind` | stable ID와 `publish`, `deploy`, `withdraw`, `rollback` 중 하나 |
| `provider`, `destination`, `channel` | exact remote target identity |
| `manifest_revision`, `artifact_set_digest` | action이 소비하는 immutable release subject |
| `approval_request_ref`, `before_snapshot_ref` | action별 single-use 승인과 current precondition |
| `remote_operation_ref`, `after_snapshot_ref` | effect attempt·receipt와 read-only 결과 확인 |
| `state` | `planned`, `approved`, `running`, `verified`, `outcome_unknown`, `rollback_required`, `rolled_back`, `withdrawn` |

`verified`의 의미는 action kind에 따라 publication이면 top-level `published`, deploy이면 target별 `deployed_verified`, withdrawal이면 `withdrawn`, rollback이면 `rolled_back`이다. public publication target이 없는 내부 artifact는 top-level `ready`에 머물며 deploy receipt를 이용해 가짜 `published`를 만들지 않는다. 한 target의 실패·unknown을 다른 target의 성공이 덮지 않고 required target 하나가 `rollback_required`이면 전체 release의 current risk projection도 이를 보존한다.

## install·update·rollback·uninstall

### clean install과 safe_default 첫 실행

Stable clean install evidence는 다음 순서를 실제 disposable Windows x64 environment에서 확인해야 한다. ARM64 Preview의 fake lifecycle은 같은 state transition·file ownership·failure injection을 검사하되 이 native 절차를 통과한 것으로 표시하지 않는다.

1. 기존 Star-Control program file·Controller startup entry·user config가 없는지 확인한다.
2. installer가 expected x64 file set만 설치한다.
3. 관리자 executor/service를 만들지 않고 current-user Controller startup entry를 명시적으로 보여주고 opt-out을 제공한다.
4. Plugin·MCP·Hook의 version·신뢰·활성 상태를 read-only로 진단한다.
5. 제품 기본 PolicyProfile이 `safe_default`인지 확인한다.
6. network·remote write·paid action·source mutation 없이 deterministic first-run smoke를 수행한다.
7. 상태·로그에 secret·username·raw absolute path가 없는지 확인한다.

first-run smoke는 아직 구현되지 않은 기능을 fake output으로 통과시키지 않는다. required core command가 `unavailable`이면 release ready가 아니다.

### update

- current installed version, config/store/catalog/plugin/runtime compatibility를 먼저 읽는다.
- new binary·Plugin·catalog를 side-by-side staging하고 final digest를 확인한다.
- state format 변경이면 compatible backup과 migration dry-run·restore verification을 먼저 수행한다.
- source/user config의 unknown field를 보존하고 silent reset하지 않는다.
- activation 전 이전 executable set·store pointer·startup entry를 rollback target으로 고정한다.
- update 후 binary·Plugin·MCP·Controller·safe_default smoke와 state open/migration Gate를 실행한다.
- 실패하면 새 version을 성공으로 표시하지 않고 rollback 또는 hold한다.

### rollback

- rollback은 이전에 검증한 artifact digest와 compatible state/store generation으로만 수행한다.
- binary rollback과 data downgrade를 같은 것으로 보지 않는다.
- 이전 binary가 current store를 읽지 못하면 user data를 삭제하거나 억지 downgrade하지 않고 compatible backup pointer·export·manual recovery를 제안한다.
- rollback action도 exact target·artifact·state generation·startup entry와 검증 plan에 결합한다.
- rollback 뒤 install smoke와 state integrity를 다시 검증한다.

### uninstall

기본 uninstall은 program file, installer ownership에 포함된 runtime·Plugin 등록과 Controller startup entry만 제거한다. 다음 자료는 기본 보존한다.

- `%APPDATA%\Star-Control` user config와 trusted manifest
- `%LOCALAPPDATA%\Star-Control` management state·backup·quarantined recovery material
- 대상 Project의 `.star-control` source와 `.ai-runs` evidence
- 사용자 source, Git repository, worktree와 remote state

`--purge-user-data`와 같은 목표 기능은 uninstall과 별도 destructive action이다. exact path class·예상 byte·backup/export·approval을 보여주고, 다른 Project/user data와 ownership을 증명할 수 없으면 실행하지 않는다.

## publish·deploy와 원격 확인

publish·deploy·remote release 생성은 [9단계 remote operation](cross-repo-change-bundle.md#remotestatesnapshot-v2)과 [승인·권한·안전](../architecture/security-and-permissions.md)을 재사용한다.

1. current complete before `RemoteStateSnapshot`을 만든다.
2. exact ReleaseManifest revision·artifact digest·channel·destination·expected remote state를 보여준다.
3. action별 ApprovalRequest를 받는다. push, PR, release publish와 deploy 승인은 서로 재사용하지 않는다.
4. adapter에 typed request와 single-use permit을 전달한다.
5. adapter receipt를 보존하되 성공 상태로 확정하지 않는다.
6. after snapshot에서 exact version·source/tag·artifact digest·visibility·deploy revision을 다시 조회한다.
7. 일치하면 publication은 top-level `published`, deploy는 target remote action `verified`와 `deployed_verified` projection이다. 확인하지 못하면 publication은 `publish_outcome_unknown`, deploy는 해당 action의 `outcome_unknown`이며 기존 top-level `published`를 바꾸지 않는다.

remote account/permission, secret scope, billing plan, protected branch, registry retention과 channel policy 변경은 release publish 승인의 범위가 아니다. 별도 사용자 권한이 없으면 실행하지 않는다.

deploy smoke와 observation window는 target별 registered CheckDescriptor로 표현한다. 단순 HTTP 200이나 process start만으로 data/API compatibility·실제 version·artifact digest를 확정하지 않는다. rollback trigger와 observation duration은 publish 전에 versioned policy로 고정하고 실패 뒤 낮추지 않는다.

## 배포 실패와 사용자 자료 보존

다음 조건은 최소 rollback trigger다.

- 설치·update 뒤 binary/Plugin/MCP/Controller identity 불일치
- supported architecture에서 process start·IPC·first-run smoke 실패
- state migration invariant·restore verification·backward compatibility 실패
- deployed revision 또는 artifact digest 불일치
- required health/contract Check fail, partial, stale 또는 unverified
- secret·license·supply-chain blocking finding

실패 시 순서는 다음과 같다.

1. 새 publish·deploy·cleanup effect를 중단한다.
2. actual local/remote state를 read-only로 reconcile한다.
3. user config·management state·source·evidence에 retention hold를 건다.
4. immutable 이전 artifact·state backup과 rollback precondition을 확인한다.
5. 필요한 exact approval 뒤 rollback 또는 withdrawal을 수행한다.
6. after-state와 user data preservation을 다시 검증한다.
7. 실패·rollback·remaining risk를 ReleaseManifest 새 revision과 EvidenceBundle에 남긴다.

rollback 실패를 원래 release success로 숨기지 않는다. user data 삭제, primary checkout reset, remote history rewrite와 force push는 rollback 기본 경로가 아니다.

## EvaluationRun v2 평가 단위

평가 subject는 다음 단위 중 정확히 하나다.

- Rule ID·version·definition fingerprint
- CheckDescriptor ID·item version·definition fingerprint
- ProfileDescriptor ID·item version·resolved closure fingerprint
- ChangeRecipe ID·version·definition fingerprint
- routing/policy candidate ID·version

여러 subject를 동시에 바꾸면 어떤 변화가 결과를 만들었는지 판정할 수 없으므로 기본 recommendation은 `needs_review`다. 조합 candidate를 평가하려면 constituent version set과 interaction hypothesis를 별도 subject로 고정한다.

case result는 최소 다음 identity를 갖는다.

| 필드 | 의미 |
|---|---|
| `case_id`, `case_version`, `corpus_ref` | 재현 가능한 사례 identity |
| `evaluation_context` | `cli_only` 또는 `codex_integrated` |
| `baseline_subject_ref`, `candidate_subject_ref` | 비교 정의와 fingerprint |
| `task_source_binding` | Task ID·source revision·config·Catalog·Tool·environment |
| `run_refs` | baseline/candidate ValidationRun·Diagnostic·Finding·Gate |
| `adjudication` | confirmed defect, false positive, unresolved, not applicable과 evidence |
| `rework` | 재계획·재실행·수동 수정 횟수와 실제 duration |
| `outcome` | success, failure, rollback, user accept/reject/revert |
| `cost_refs` | provider가 검증한 CostRecord만 |
| `limitations` | missing ground truth·sample·environment·provider data |

## 평가 metric

Rule·Check·Profile·Recipe별로 raw case result에서 다음 metric을 계산한다.

| metric | 계산·해석 규칙 |
|---|---|
| execution time | attempt별 wall duration과 가능할 때 CPU duration; p50/p95는 sample 수와 함께 |
| finding count | raw Diagnostic/Finding 수, rule family·severity·baseline relation별 분리 |
| actual defect count | adjudication이 confirmed인 finding만; unresolved를 defect나 FP로 넣지 않음 |
| false positive count/rate | adjudicated non-defect만; suppression 수로 대체하지 않음 |
| false negative | versioned positive Corpus 또는 후속 confirmed defect가 candidate에 누락된 경우 |
| flaky count/rate | 같은 subject·input·environment에서 결과가 변한 case |
| suppression | active·expired·newly_added·broadened·removed를 각각 집계 |
| new/worsened debt | baseline relation `new\|worsened`; release·change Gate의 우선 보호 대상 |
| existing debt | `existing_unchanged`; 보고·Radar 대상이며 candidate accept를 자동 block하지 않음 |
| rework | replan, rerun, manual edit, review round와 실제 wall duration |
| failures | tool launch, timeout, invalid output, Gate block, rollback과 outcome unknown 분리 |
| acceptance | 사용자 accept, override, reject, post-completion revert; 무응답은 unknown |
| usage | provider/tool이 실제 제공한 token·invocation·byte·duration 단위 |
| monetary cost | provider가 검증 가능한 금액과 price source를 제공했을 때만 |

precision·recall·rate를 계산할 때 denominator와 unadjudicated case 수를 함께 표시한다. denominator 0은 100%가 아니라 `not_computable`이다. suppression된 finding도 raw finding·Gate·평가에서 사라지지 않는다.

## baseline·candidate 비교

비교 가능하려면 다음이 같아야 한다.

- case/corpus version과 case selection policy
- Task·source revision 또는 동일한 recorded input
- change class·Profile context
- config 중 평가 subject 외의 leaf와 policy floor
- Catalog·Tool Registry와 external tool identity
- OS·architecture·toolchain·network/cache·resource limit
- warmup·attempt·timeout·retry·measurement protocol
- ground-truth/adjudication policy

다르면 metric을 억지로 보정하지 않고 dimension별 `not_comparable`을 기록한다. 시간 순서가 다른 실제 작업 비교는 외부 변화와 learning effect를 limitation으로 남기며 deterministic replay·shadow 결과와 분리한다.

변경 전후 효용은 최소 다음 delta를 함께 본다.

- confirmed defect 발견 수와 false negative
- false positive·flaky·suppression 변화
- new/worsened finding과 protected Gate 결과
- first result까지 시간, 총 실행 시간, 수동 review·재작업 시간
- 실패·retry·rollback·revert 횟수
- verified usage와 비용

한 지표의 개선으로 protected safety·correctness metric 악화를 상쇄하지 않는다. 예를 들어 시간이 줄어도 confirmed false negative, validator weakening, new critical finding 또는 rollback 증가가 있으면 자동 accept가 아니다.

## recommendation과 trial

EvaluationRun recommendation은 정확히 다음 의미를 사용한다.

| recommendation | 의미 |
|---|---|
| `keep` | current baseline 유지 |
| `trial` | candidate를 versioned shadow/bounded opt-in으로 더 관찰 |
| `accept` | review된 Catalog/policy source change 후보로 승인 가능 |
| `reject` | 효용 부족·오탐·회귀·안전 악화로 사용하지 않음 |
| `needs_review` | ground truth·sample·comparability·의미 판단 부족 |

threshold와 protected metric은 evaluation 시작 전에 `evals/policies/`의 versioned policy로 고정한다. 결과를 본 뒤 threshold, case set, retry, severity 또는 suppression policy를 candidate에 유리하게 바꾸면 새 EvaluationRun이다.

다음 경우 기본 recommendation은 `trial`, `reject` 또는 `needs_review`이며 자동 accept가 아니다.

- sample·adjudication·provider data가 부족함
- 실행 시간은 줄었지만 재작업·실패·rollback이 줄지 않음
- false positive·flaky·suppression broadened가 policy 상한을 넘음
- confirmed defect 추가 발견이 없고 사용자 수락·시간 개선도 없음
- CLI-only에서는 효용이 없고 Codex context에서만 개선되거나 그 반대
- candidate가 protected Rule·Check·Profile closure를 약화함
- evaluation environment·case set이 baseline과 비교 불가능함

`trial`은 기본 `shadow`다. 실제 route·Check·permission·source·release를 바꾸지 않는다. bounded opt-in을 허용하려면 exact users/projects, 기간, fallback baseline, stop trigger와 data retention을 먼저 승인한다.

## 검증기 약화 금지

candidate accept 전 B03 validator guard는 최소 다음을 baseline과 비교한다.

- required Rule·Check 제거 또는 optional 하향
- severity·confidence·Gate floor 하향
- `new|worsened` ratchet을 report-only로 변경
- suppression selector 확대·expiry 제거·승인 완화
- flaky를 마지막 retry pass로 숨기는 변경
- positive/negative/adversarial Corpus 삭제·expected 자동 갱신
- evidence freshness·completeness·artifact digest 요구 완화
- `ready`, `approved`, `published` 상태를 합치는 변경
- CLI-only에서 AI review를 required pass로 추가하거나 deterministic Check를 제거

protected 변화 하나라도 있으면 candidate pass율·속도와 무관하게 `reject` 또는 별도 ADR이 필요한 `needs_review`다. 검사 자체가 잘못됐다는 증거가 있으면 Rule 의미·Corpus·migration을 명시적으로 바꾸며 이전 evidence를 current로 재해석하지 않는다.

## Maintenance Radar와 deprecation

Maintenance Radar item은 오래된 Rule·Check·Profile·Recipe에 대해 다음을 참조한다.

- current item ID·version·definition fingerprint와 lifecycle
- last EvaluationRun, recommendation, sample·limitation과 evaluated_at
- false positive·flaky·suppression·failure·duration trend
- replacement candidate·migration plan·compatibility window
- active Catalog/Profile/Recipe/Rule/Check reference와 historical evidence count
- deprecation deadline·owner·next review trigger

Catalog item lifecycle은 다음처럼 고정한다.

| 상태 | 새 plan 선택 | historical evidence | 요구사항 |
|---|---:|---:|---|
| `active` | 허용 | 유지 | current Corpus·owner·version |
| `deprecated` | 명시적 compatibility window 안에서만 | 유지 | replacement·migration guide·deadline·Diagnostic |
| `retired` | 금지 | archived CatalogSnapshot으로 유지 | consumer/profile/suppression/baseline migration 완료 |
| `rejected` | 금지 | EvaluationRun과 tombstone 유지 | trial candidate ID/version 재사용 금지 |

전이는 `active -> deprecated -> retired` 또는 trial candidate의 `rejected`다. stable ID와 version 의미를 재사용하지 않는다.

- Rule deprecation은 baseline·suppression·Finding history를 replacement fingerprint로 임의 변환하지 않는다.
- Check deprecation은 대체 Check의 scope·tool·output·coverage conformance가 있어야 한다.
- Profile deprecation은 최종 16개 built-in coverage를 줄이지 않고 replacement Profile closure·permission·Gate migration을 제공한다.
- Recipe deprecation은 새 PatchSet prepare를 막되 기존 partial/recovery attempt의 exact historical Recipe byte는 보존한다.
- removed source entry가 있어도 archived CatalogSnapshot·tombstone은 historical evidence 해석에 남긴다.
- deprecation 자체가 validator coverage를 낮추면 B03와 release Gate가 block한다.

## CLI-only와 Codex 연동 효용 분리

`evaluation_context`는 반드시 다음 둘 중 하나다.

| context | 포함 | 제외 |
|---|---|---|
| `cli_only` | deterministic discovery·planning·Check·Gate·release/eval command와 human review | Codex task 생성·model usage·AI review |
| `codex_integrated` | 같은 core command에 Codex planning/execution/review가 선택 소비자로 붙은 run | 다른 AI provider·OpenAI API 직접 호출 |

같은 case라도 context별 EvaluationRun을 만들고 metric을 합산하지 않는다. CLI-only의 제품 효용은 Codex 비용·시간 없이 계산하고, Codex 연동 효용은 추가된 model usage·review/rework 변화와 함께 계산한다. Codex 연동이 유리해도 core Rule·Gate·release 상태기계를 Codex dependency로 바꾸지 않는다.

## CLI application 계약

아래는 목표 command surface이며 현재 구현을 뜻하지 않는다.

### read-only·local effect 없음

```text
star release plan
star release status
star release verify --layer local_quick|target|full|release
star release package dry-run
star release manifest show
star eval plan
star eval run --mode offline|replay|shadow
star eval compare
star eval status
star catalog lifecycle show
```

`verify`가 실제 Check process를 실행하면 project source에는 effect가 없지만 process·artifact write·optional network/cost permission은 ValidationPlan에 표시한다. `plan|status|show`는 external effect가 없다.

### local install lifecycle effect

```text
star release install-test
star release update-test
star release rollback-test
star release uninstall-test
```

이 명령은 Star-Control-owned disposable environment에서만 실행하며 실제 사용자 설치를 암묵 변경하지 않는다. target root·artifact digest·state fixture·cleanup ownership을 permit에 결합한다.

### remote effect

```text
star release approve
star release publish
star release publish verify
star release withdraw
star release deploy
star release deploy rollback
```

`approve`는 effect를 실행하지 않고 exact ApprovalRequest decision을 기록한다. `publish|withdraw|deploy|rollback`은 각각 별도 approval과 before/after snapshot을 요구한다. `--yes`, `personal_auto`, standing remote scope와 이전 승인으로 이 경계를 우회하지 않는다.

## application·Package 경계

```text
star-application/release
  -> star-vcs/release_handoff·remote_state: source/remote current probe
  -> star-validation/release_gate: tier·phase·readiness pure decision
  -> star-checks/release_deploy: package·metadata·install lifecycle Diagnostic
  -> star-execution/release: typed build/package/promotion/remote operation 조정
  -> star-evidence/release: artifact set·manifest·report·redaction
  -> packaging/windows: installer source·ownership·lifecycle fixture

star-application/evaluation
  -> star-evaluation: cohort·metric·comparison·recommendation pure engine
  -> star-validation/validator_guard: 보호 metric·Corpus·weakening Gate
  -> evals/: source corpus·baseline·policy·candidate metadata
  -> star-state/star-evidence: EvaluationRun·case result·report
```

- `star-execution/release`는 compiler·CI·installer·signer·registry SDK를 직접 구현하지 않고 port를 조정한다.
- build/package/CI/signer/remote adapter는 state writer가 아니며 observation·artifact·receipt만 반환한다.
- `star-evaluation`은 Catalog source를 쓰지 않는 pure comparator다.
- 실제 Catalog/Rule/Profile/Recipe 변경은 review된 source change와 M3 Gate를 거친다.
- CLI·MCP·Codex adapter는 ReleaseManifest status나 EvaluationRun recommendation을 재해석하지 않는다.
- Controller가 ReleaseManifest·EvaluationRun·event·current projection의 단일 Writer다.

새 Package를 만들지 않는다. 기존 `star-validation`, `star-checks`, `star-execution`, `star-evaluation`, `star-vcs`, `star-evidence`, adapter와 `packaging/` 책임 안에서 구현한다.

## 최종 기능 소유권 감사

| ID | 의미 정본 | 기본 물리 owner |
|---|---|---|
| A01 | [목표·단계 계약](goal-and-stage.md) | `star-planning/task_contract` |
| A02 | [목표·단계 계약](goal-and-stage.md) | `star-planning/stage_graph·replan` |
| A03 | [Project Catalog·Code Index](project-catalog-and-code-index.md) | `star-project` |
| A04 | [변경 계획·영향 분석](change-planning-and-impact.md) | `star-planning/impact`, `star-validation/selector` |
| A05 | [배정 계약](routing.md) | `star-routing` |
| A06 | [목표·상태](goal-and-stage.md), [Codex 통합](../architecture/codex-integration.md) | `star-application`, `star-execution` |
| A07 | [이벤트·상태](events-and-state.md), [상태·산출물](../architecture/state-and-artifacts.md) | `star-state`, `star-execution/recovery` |
| A08 | [승인·권한·안전](../architecture/security-and-permissions.md) | `star-policy` |
| A09 | [CrossRepo ChangeBundle](cross-repo-change-bundle.md), [worktree·merge](../architecture/worktrees-and-merge.md) | `star-vcs`, `star-execution/change_bundle` |
| A10 | [설정·Catalog](config-and-catalog.md), [Managed Registry](managed-symbol-registry.md), [Tool Registry](external-tool-registry.md) | `star-config/registry`, `star-project/managed_registry` |
| B01 | [검사·완료·증거](validation-and-evidence.md), [공통 Gate](../features/common-validation-gate.md) | `star-validation`, `star-checks/change_scope` |
| B02 | [검증 기능 B02](../features/validation.md#b02-테스트-신뢰성-검증) | `star-checks/test_trust` |
| B03 | [공통 Gate validator guard](../features/common-validation-gate.md#8-b03-validatorpolicytest-harness-자기보호) | `star-checks/validator_guard`, `star-evaluation` |
| B04 | [계약·환경](contract-compatibility-and-environment.md), [Migration](migration-performance-and-platform.md) | `star-checks/contract_architecture` |
| B05 | [7단계 유지보수](failure-security-and-dependency-maintenance.md) | `star-checks/security_supply_chain` |
| B06 | [7단계 유지보수](failure-security-and-dependency-maintenance.md) | `star-checks/failure_recovery` |
| B07 | [6단계 계약·환경](contract-compatibility-and-environment.md) | `star-checks/docs_environment` |
| B08 | [8단계 성능 계약](migration-performance-and-platform.md#성능build-측정-계약) | `star-checks/performance_build` |
| B09 | 이 문서, [검사·증거](validation-and-evidence.md) | `star-checks/release_deploy`, `star-validation/release_gate` |
| C01 | [최종 16개 Profile](../features/profiles.md), [ProfileDescriptor](config-and-catalog.md#profiledescriptor) | `catalog/profiles`, `star-config` |
| D01 | [9단계 ChangeBundle](cross-repo-change-bundle.md) | `star-vcs/multi_repo·remote_operation` |
| D02 | 이 문서의 EvaluationRun·Radar 연결 | `star-evaluation`, `evals/` |
| D03 | 이 문서, [설치·공개 배포](../operations/installation.md) | `packaging/windows`, `star-application/release` |

23개 기능 모두 기본 의미 정본과 물리 owner가 있다. 상세 Package 표의 단일 정본은 [Repository·Package 구조](../architecture/repository-layout.md#23개-구현-기능의-소유-package)다.

## 공통 개발 관리 자산 소유권 감사

| 자산 | canonical/source | derived/runtime | 변경 writer·경계 |
|---|---|---|---|
| local management DB | Git source·Catalog·contract가 의미 정본 | global/project management repository | Controller application transaction만; backend는 `star-state` private |
| Project Catalog | project declarations와 actual filesystem/Git | `ProjectCatalogSnapshot` | Controller read-only discovery transaction |
| Code Index | actual source byte·toolchain·adapter contract | `CodeIndexSnapshot`·cache | Controller scan generation; cache는 current truth 아님 |
| Finding DB | 별도 DB 정본 없음 | 공통 project store의 Finding·Occurrence·Diagnostic projection | Controller만; scanner별 DB 금지 |
| Managed Symbol Registry | Git `.star-control/managed-registry/` | `ManagedRegistrySnapshot` derived Index | source는 승인된 M4 PatchApplication, DB direct write 금지 |
| ChangeRecipe | built-in/project Catalog source | resolved CatalogSnapshot·RecipeExecution | source review, 실행은 M4 application |
| CrossRepo ChangeBundle | project별 source·plan·Gate와 global bundle document | global/project projection·evidence | Controller 9단계 application; Git/remote adapter는 receipt만 |
| ReleaseManifest | version·package policy·source revision·final artifact byte | management document·`.ai-runs`·remote snapshot | Controller 10단계 application; CI/registry adapter 직접 writer 금지 |
| EvaluationRun | versioned eval corpus·baseline·policy·candidate definition | management document·evaluation artifact | Controller application; recommendation 자동 역쓰기 금지 |
| Rust style source/policy | Project Git rustfmt/Cargo lint/Clippy/toolchain source와 versioned Profile/Catalog/user grant | RustToolchainBinding·PolicySnapshot·CoverageMatrix·StepExecution, Patch/Evidence projection | source는 review/M4, 실행·projection은 Controller; DB/tool 직접 writer 금지 |

`Finding DB`라는 이름으로 scanner별 store를 새로 만들지 않는다. Finding·Occurrence·Diagnostic·Suppression·Baseline·Disposition은 0·3단계 공통 management repository와 evidence 계약을 재사용한다.

## 단일 정본과 Writer 감사

| 정보 | 단일 정본 | 파생물·증거 |
|---|---|---|
| 계약 type | `star-contracts` Rust type | generated Schema·reference |
| 설정 key·병합 | [설정과 Catalog 계약](config-and-catalog.md)과 config source | EffectiveConfig |
| Task·Tool·Check·Rule·Profile·Recipe | built-in/project Catalog source | CatalogSnapshot |
| permission·policy | PolicyProfileDescriptor와 user/project constraint | PermissionPlan·ApprovalRequest |
| source·manifest | Project Git/source | management Index·Finding |
| Rust formatting/lint/toolchain | Project Git config/source와 versioned Catalog policy | resolved M11 binding·coverage·Diagnostic·Patch/Evidence |
| 검증 사실 | ValidationRun·Diagnostic·GateDecision | EvidenceBundle·ReviewPack |
| release version·package policy | canonical version source와 `packaging/release.toml` 목표 | ReleaseManifest·file manifest |
| remote publish 사실 | provider after `RemoteStateSnapshot` | RemoteOperationRecord·published projection |
| evaluation policy·case | `evals/` source | EvaluationRun·report·Radar ref |

모든 persisted current projection은 Controller single writer를 유지한다. CLI, MCP, Codex adapter, CI provider, build tool, signer, installer, scanner, profiler와 remote provider는 canonical source·DB row·Gate·release status를 직접 쓰지 않는다.

## 제외 기능·전문 도구 재구현 감사

최종 dependency graph에 다음 기능을 넣지 않는다.

- local AI, 다른 AI provider, OpenAI API 직접 호출과 OpenAI-compatible server
- browser Star-Control UI, HTTP control UI와 자체 예약 실행
- provider registry·GPU manager·AI failover
- compiler, parser/type checker 전체, scanner, debugger, profiler, package manager·resolver
- CI service, build farm, artifact registry, installer technology, signing/PKI service, deploy platform
- Git hosting provider의 rule·permission·state engine 복제

Star-Control이 소유하는 것은 이 도구의 **선택 가능한 descriptor, typed invocation, permission, evidence normalization, Gate, 상태·복구·승격 순서**다. adapter가 tool 기능을 노출하더라도 core Package가 provider SDK type, raw command string과 credential을 소유하지 않는다.

## 구현 상태와 외부 gate 분리

| 구분 | 현재 상태 |
|---|---|
| 구현됨 | MCP 기반 수직 Slice, P0 공통 개발 관리 첫 수직 Slice, P-0026 Windows installer·installation record·Codex Plugin 렌더링 transport Slice, P-0039 4-EXE updater lifecycle |
| 설계 확정·구현 전 | M1~M11 전체와 이 10단계 release/evaluation, 최종 16 Profile 완성, release/evaluation engine |
| 외부/환경 gate | current 통합 binary의 Codex·Inspector evidence, required core 17개 owner, exact 24H2·clean x64 Stable 수명주기, ARM64 Preview cross-build·simulation, Authenticode certificate·timestamp provider |
| 별도 사용자 승인 gate | package/dependency 설치, system setting, paid CI/signing, publish·deploy·withdrawal·remote/account effect |

문서 설계 완료를 product ready나 release ready로 표시하지 않는다. `ready`·`approved`·`published`는 실제 구현과 current evidence가 생긴 뒤에만 runtime 상태로 사용할 수 있다.

## stable 오류와 event

대표 stable code는 [오류와 진단 계약](errors-and-diagnostics.md)의 10단계 section이 소유한다. 최소 domain family는 다음을 구분한다.

- `RELEASE_SUBJECT_STALE`
- `RELEASE_PROFILE_MISMATCH`
- `RELEASE_CLEAN_ENVIRONMENT_REQUIRED`
- `RELEASE_ARTIFACT_SUBJECT_MISMATCH`
- `RELEASE_ARTIFACT_DIGEST_MISMATCH`
- `RELEASE_REBUILD_FORBIDDEN`
- `RELEASE_PACKAGE_CONTENT_INVALID`
- `RELEASE_METADATA_INCOMPLETE`
- `RELEASE_PLATFORM_EVIDENCE_MISSING`
- `RELEASE_INSTALL_LIFECYCLE_FAILED`
- `RELEASE_APPROVAL_REQUIRED`
- `RELEASE_APPROVAL_STALE`
- `RELEASE_REMOTE_RESULT_UNVERIFIED`
- `RELEASE_ROLLBACK_REQUIRED`
- `EVALUATION_NOT_COMPARABLE`
- `EVALUATION_GROUND_TRUTH_INCOMPLETE`
- `EVALUATION_VALIDATOR_WEAKENING`
- `EVALUATION_POLICY_CHANGED`
- `CATALOG_LIFECYCLE_MIGRATION_REQUIRED`

최소 event 순서는 다음이다.

```text
release.draft_created
release.preflighted
release.candidate_sealed
release.validation_completed*
release.ready | release.blocked
release.approval_recorded
release.publish_started
release.publish_verified | release.publish_outcome_unknown
release.rollback_required
release.rollback_verified | release.rollback_failed
release.withdrawal_verified

evaluation.planned
evaluation.case_completed*
evaluation.compared
evaluation.recommended
catalog.item_deprecated | catalog.item_retired | catalog.item_rejected
```

event에는 artifact byte, raw provider response, secret와 전체 case source를 inline으로 넣지 않고 DocumentRef·ArtifactRef와 fingerprint만 둔다.

## 구현 순서

10단계 P8 evaluation은 M1~M9 제품 Gate 뒤, P9 final release는 M1~M9와 M11 제품 Gate가 실제로 통과한 뒤 다음 순서를 지킨다.

1. `ReleaseManifest` v2, `EvaluationRun` v2와 final validation/evidence v6 type·Schema·minimal/full/invalid/future fixture. release v5 reader와 M11 Rust binding migration을 포함
2. version/changelog/license/package policy loader와 single-source drift pure checker
3. release subject·Profile closure·clean environment preflight pure engine
4. artifact entry·artifact set digest·included-files manifest canonical hash golden
5. fake clean builder/package adapter의 build-once·crash·rebuild-forbidden conformance
6. local_quick→target→full→release tier와 phase Gate pure aggregation
7. package dry-run·architecture·path·license·conditional supply-chain Corpus
8. fake x64/ARM64 installer adapter의 install·safe_default·update·rollback·uninstall state machine
9. ready→approved→publishing→published/outcome-unknown 상태 reducer와 approval staleness
10. fake remote publisher/deployer의 before/after snapshot·partial·timeout·rollback conformance
11. EvaluationRun case/adjudication/metric/comparability pure engine
12. validator weakening guard, trial/accept/reject/needs_review와 Radar/deprecation migration
13. CLI-only E2E와 Codex-integrated recorded adapter를 별도 cohort로 검증
14. 실제 clean Windows x64·installer·CI·signing·GitHub Releases adapter와 ARM64 cross-build·simulation adapter를 하나씩 연결

실제 publish·deploy는 전체 conformance와 release Gate 뒤에도 현재 사용자 action별 승인 없이는 실행하지 않는다.

## Fixture와 Corpus

### release identity·계층

- 같은 Task ID지만 다른 source revision/config/Profile/tool version
- local quick pass, target fail과 full stale
- full pass 뒤 source 또는 Catalog 변화
- clean build와 dirty/cache-contaminated build 구분
- x64 success, ARM64 cross-build success지만 native runtime unverified

### artifact·package

- 같은 filename, 다른 digest
- build-once artifact와 release 재build byte 차이
- signing 전/후 byte 변화와 unsigned evidence 상속 거부
- missing/extra file, case collision, path escape, wrong architecture
- Plugin에 runtime/user state 포함, package에 `legacy/`·`.ai-runs`·DB 포함
- SBOM/provenance/signing required/not-required/unavailable/incomplete
- version/changelog/license/notice mismatch

### install lifecycle

- clean x64 install·safe_default first-run과 ARM64 fake lifecycle. 후자는 native install evidence가 아님
- current-user startup opt-out와 관리자 service 부재
- supported previous version update·state migration·unknown config 보존
- update crash 전/후 activation, previous artifact rollback
- binary rollback 가능하지만 store downgrade 불가
- uninstall program 제거·user state 보존과 별도 purge 거부

### publish·deploy

- ready지만 approval 없음, approved지만 effect 없음
- approval의 digest/channel/provider/expiry mismatch
- adapter success response 뒤 after snapshot missing/mismatched
- timeout 뒤 remote published, not published, ambiguous 세 case
- publish success 뒤 deploy smoke fail과 rollback
- withdrawal after snapshot과 historical evidence 유지

### evaluation

- actual defect·false positive·unresolved·suppression 구분
- denominator 0, partial adjudication과 missing cost
- same case의 baseline/candidate tool/environment mismatch
- faster candidate지만 false negative·rollback 증가
- finding 감소가 Rule disable·severity 하향·suppression 확대 때문인 candidate
- flaky last-retry pass와 raw attempts 보존
- CLI-only 유효·Codex 무효, Codex 유효·CLI 무효 context 분리
- trial expiry, deprecated reference, retired item 새 plan 선택 거부
- Recipe partial recovery가 retired historical definition을 계속 해석

## 설계 수용 기준

- 0~11단계 정본, M11 conformance와 release handoff가 빠짐없이 연결되고 제품 미구현 상태가 분리된다.
- local_quick, target, full, release가 같은 identity를 유지하면서 서로 다른 Gate를 가진다.
- clean Windows x64 Stable build·test·package·native runtime·install과 ARM64 Preview cross-build·simulation evidence 요구가 명확하다.
- source revision과 final artifact digest가 연결되고 build-once·verify·promote 원칙을 우회할 수 없다.
- package file list, version, changelog, metadata, license와 conditional SBOM·provenance·signing 판정이 구현 가능하다.
- install, safe_default first run, update, rollback, uninstall과 user data preservation state machine이 있다.
- ready, approved, publishing, published와 outcome unknown이 다른 상태다.
- published는 exact remote after snapshot 없이 생성되지 않는다.
- Rule·Check·Profile·Recipe별 duration·finding·actual defect·FP·flaky·suppression·rework·failure·verified cost를 비교한다.
- 기존 부채보다 새 code의 new/worsened 문제 방지를 우선한다.
- candidate가 validator·Corpus·Gate를 약화해 통과율을 높일 수 없다.
- Maintenance Radar가 evaluation과 item lifecycle·replacement·migration을 연결한다.
- CLI-only와 Codex-integrated 효용이 다른 cohort로 측정된다.
- A01~D03, 최종 16 Profile, management DB·Project Catalog·Code Index·Finding·Managed Registry·Recipe·ChangeBundle·release/evaluation 소유권이 모두 있다.
- `rust_style_auto_fix`가 stable pinned toolchain·exact allowlist·complete coverage·isolated PatchSet·idempotence·`personal_auto` exact approval와 Windows x64 CLI-only 및 ARM64 target/cfg simulation evidence를 가진다.
- 계약·설정·Profile·정책·증거의 정본과 Controller single writer가 유지된다.
- local AI, 다른 AI provider, OpenAI API 직접 호출, browser UI와 자체 scheduler가 다시 들어오지 않는다.
- compiler·scanner·debugger·profiler·package manager·CI/deploy service를 Star-Control이 재구현하지 않는다.
- 문서만으로 contract, state, adapter, approval, evidence, fixture와 구현 순서를 재구성할 수 있다.

## 구현 전 남은 외부 gate

- required core 17개 owning handler·Schema와 current MCP completion evidence
- historical success가 아닌 current 통합 binary의 Codex·Inspector integration evidence
- exact Windows 11 24H2 baseline과 clean disposable x64에서 current-user install·first-run·update·failure rollback·repair·uninstall의 실제 수명주기 evidence
- ARM64 Preview cross-build·PE architecture·file manifest·signature·installer model·fake lifecycle evidence. native ARM64는 `native_unverified` limitation으로 보존
- public Authenticode signer·certificate·timestamp provider와 비용 승인. 없으면 release `blocked_external`
- GitHub Releases immutable artifact·digest·after-state adapter conformance
- M11 Rust ToolDescriptor/Catalog/Schema, complete Corpus와 native Windows x64 CLI-only 및 ARM64 target/cfg simulation conformance
- 실제 tag·GitHub draft·asset upload·publish·withdrawal remote effect의 현재 사용자 승인

이 gate가 남아 있는 동안 문서는 설계 완료일 수 있지만 제품·release는 `ready`, `approved`, `published`가 아니다.
