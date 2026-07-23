# 3단계 공통 검증·품질 Gate 상세 설계

## 상태와 문서 소유권

이 문서는 Star-Control 3단계인 **공통 검증·품질 Gate**의 실행 의미와 구현 순서를 소유한다. P-0044는 ready ValidationPlan v2의 CheckGraph를 실행하는 첫 제품 Slice를 구현했고, P-0054는 typed real-process executor, Rule/Baseline/Suppression/Disposition·ReviewPack, exact Task/source/Profile binding, durable evidence, single-use permit와 `patch_pre_apply|patch_post_apply` Controller·CLI 경로까지 확장했다. P-0031의 tracked-path ValidationPlan·cache pure policy와 P-0035의 native validation precursor는 별도 v1 운영 경로로 유지한다. 등록 provider가 없는 Rule family는 결과를 합성하지 않으며 외부 scanner·debugger 실행을 내부 완료 근거로 쓰지 않는다.

3단계는 제품 로드맵 [P5 검사·증거·이어하기](../roadmap/final-implementation.md#p5-검사증거이어하기)의 첫 공통 수직 Slice다. 이 문서에서는 이를 `M3`라고 부른다. 기존 P0의 Finding·ValidationResult·GateDecision 수직 Slice를 폐기하지 않고, M1 Project Catalog·Code Index와 M2 ChangeSet·ImpactAnalysis·ValidationPlan을 소비할 수 있도록 versioned target으로 확장한다.

정본 책임은 다음처럼 나눈다.

| 책임 | 정본 |
|---|---|
| ValidationPlan·ValidationRun·ValidationResult·GateDecision·EvidenceBundle·ReviewPack wire 의미 | [검사·완료·증거 계약](../contracts/validation-and-evidence.md) |
| ErrorEnvelope·Diagnostic·stable code·위치·fingerprint·remediation | [오류와 진단 계약](../contracts/errors-and-diagnostics.md) |
| Rule·Check·Tool·Validator Registry·Profile·Gate policy metadata | [설정과 Catalog 계약](../contracts/config-and-catalog.md) |
| Finding·Baseline·Suppression의 versioned lifecycle | [공통 개발 관리 계약](../contracts/development-management.md) |
| source·evidence·Corpus·baseline 저장 경계 | [상태 기록과 이어하기](../architecture/state-and-artifacts.md) |
| `star-validation`, `star-checks`, Corpus와 test 소유권 | [Repository·Package 구조](../architecture/repository-layout.md) |
| B01~B09의 제품 기능 범위 | [검증과 개발 보조 기능](validation.md) |
| M2가 고정하는 affected Check·scope·fallback 입력 | [변경 계획·영향 분석 계약](../contracts/change-planning-and-impact.md) |
| M5 ManagedDeclaration·manifest·lifecycle·binding·consumer와 drift code | [관리형 Symbol Registry 계약](../contracts/managed-symbol-registry.md) |

이 문서는 위 wire field를 복제하지 않는다. 대신 구현자가 그 계약을 어떤 순서와 불변식으로 소비하고, 어떤 rule family가 어떤 Diagnostic을 생산하며, GateDecision을 어떻게 결정하는지 고정한다.

## 목표

M3의 목표는 다음과 같다.

1. 사용자 작업 계약, M2 계획, 실제 workspace 변경과 완료 주장을 같은 revision에서 대조한다.
2. 선택된 Check를 등록된 ToolDescriptor로만 실행하고 결과를 공통 Diagnostic으로 정규화한다.
3. `pass`, `fail`, `not_run`, `partial`, `unverified`, `stale`, `flaky`를 서로 다른 축으로 보존한다.
4. 기존 부채와 신규·악화 문제를 구분해 기본 ratchet이 새 악화를 막게 한다.
5. suppression이 관찰 결과를 삭제하거나 성공으로 변조하지 않게 한다.
6. validator·policy·test harness 자체를 약화해 Gate를 우회하는 변경을 별도 보호 경로로 검사한다.
7. B01~B07과 외부 도구 결과가 같은 Diagnostic·Gate·EvidenceBundle·ReviewPack 계약을 사용하게 한다.
8. 결정적 도구와 재현 가능한 evidence를 AI 의미 평가보다 먼저 사용한다.
9. 4단계 Patch engine이 source 적용 전과 적용 후에 사용할 exact Gate를 제공한다.

## 제외 범위

M3 자체는 다음을 만들지 않는다.

- compiler, language server, test framework, package manager, 정적 분석기, secret scanner, 취약점 DB 또는 외부 scanner의 재구현
- 프로젝트별 임의 raw shell 문자열, 동적 PowerShell·`cmd` script text 또는 PATH 첫 실행 파일을 검사 정본으로 저장하는 기능
- M2 ValidationPlan에 없는 Check family·scope를 runner가 임의로 다시 고르는 기능
- source, test, snapshot, baseline 또는 suppression을 Gate 통과 목적으로 자동 수정하는 기능
- false positive나 flaky 결과를 삭제·덮어쓰기·마지막 retry 결과만으로 숨기는 기능
- CLI-only mode에서 Codex·다른 AI·OpenAI API를 호출해 독립 검토를 수행하는 기능
- 검사 도구 자동 설치, dependency update, 외부 계정 변경, remote write와 유료 검사 자동 실행

외부 도구가 필요하지만 등록·신뢰·사용 가능 상태가 아니면 `not_run` 또는 unresolved 상태와 reason code를 남긴다. 해당 도구가 없다는 사실을 `not_applicable`이나 성공으로 바꾸지 않는다.

## 선행조건과 현재 경계

| 선행조건 | M3가 요구하는 상태 | 현재 상태와 처리 |
|---|---|---|
| P0 공통 ID·Finding·Scan·Evidence·Gate 기반 | source/DB/evidence 분리, immutable snapshot, Controller 단일 Writer | 첫 수직 Slice 구현. 그대로 재사용 |
| M1 ProjectCatalogSnapshot·CodeIndexSnapshot | 대상 partition의 current/partial/stale와 coverage를 exact ref로 조회 | P-0042 첫 Rust bounded Slice 구현. unsupported·unverified partition은 pass로 승격하지 않음 |
| M2 TaskSpec·ScopeRevision·ChangeSet·ImpactAnalysis·ValidationPlan | `readiness=ready`, accepted scope, bound TaskInvocation과 current fingerprint | P-0043 full planning bundle 첫 Slice 구현. P-0031 precursor와 구분 |
| Baseline·Suppression | Finding과 모든 Diagnostic의 existing/new/worsened·active/expired 구분 | P0 v1은 Finding 중심. M3 target v2와 migration 필요 |
| ValidationRun evidence binding | subject revision·workspace·ChangeSet·config·Catalog·Check·Tool identity exact 결합 | P-0044 generic runner/writer 첫 Slice 구현. provider별 확장 필요 |
| Validator Registry | stable Rule/Check mapping, fingerprint contract, fixture manifest | generic executable binding 구현. 전체 Rule family·Corpus conformance는 후속 확장 |

P-0044 bounded Slice는 P-0042 M1과 P-0043 M2 계약·fixture를 선행 입력으로 구현했다. 이후 확장도 current M1/M2 binding을 요구하며 fake input이나 pure engine fixture만으로 통합 완료를 주장하지 않는다.

## 핵심 축과 용어

서로 다른 상태를 한 `success` boolean으로 합치지 않는다.

| 축 | 값 | 질문 |
|---|---|---|
| execution outcome | `pass`, `fail`, `not_run`, `error`, `cancelled` | 등록된 Check가 실제로 어떻게 끝났는가 |
| completeness | `complete`, `partial`, `unverified` | 계획한 출력과 scope를 전부 해석했는가 |
| freshness | `current`, `stale_source`, `stale_plan`, `stale_config`, `stale_catalog`, `stale_tool`, `stale_environment`, `unverified` | evidence가 지금 판단하려는 subject와 같은가 |
| stability | `stable`, `flaky`, `not_evaluated` | 같은 입력·계약의 시도 결과가 일관되는가 |
| baseline relation | `new`, `existing_unchanged`, `worsened`, `improved`, `not_observed`, `incompatible`, `unbaselined` | 기준 대비 무엇이 달라졌는가 |
| suppression state | `none`, `active`, `expired`, `stale`, `revoked`, `invalid` | 예외가 현재 exact 문제에 유효한가 |
| run satisfaction | `clean_pass`, `ratchet_satisfied`, `unsatisfied`, `waived_for_review` | Gate policy상 required Check가 충족됐는가 |
| Gate decision | `auto_pass`, `human_review`, `block` | 자동으로 다음 단계로 진행할 수 있는가 |

wire enum은 소문자 snake_case를 사용한다. CLI와 문서 표시는 각각 `AUTO_PASS`, `HUMAN_REVIEW`, `BLOCK`으로 렌더링한다. 표시 문자열을 protocol 값이나 분기 조건으로 사용하지 않는다.

`HUMAN_REVIEW`는 성공도 실패도 아니다. 의미 검토, explicit waiver 또는 flaky 판단을 기다리는 terminal decision이며 새 입력이나 사용자 결정으로 새 GateDecision을 만들기 전까지 자동 진행하지 않는다.

## 입력과 출력 document graph

```text
TaskSpec + accepted ScopeRevision
  + ChangePlan[] + ready ValidationPlan
  + planning-baseline ChangeSet[]
  + current Project/Workspace/Catalog/Index probes
  + ValidatorRegistrySnapshot + EffectiveConfig
  + CompletionClaim[]
  + Baseline + Suppression revision set
  -> preflight input check
  -> before/after actual ChangeSet collection
  -> execution-binding preflight
  -> ValidationRun[]
  -> Diagnostic[]
  -> DiagnosticEvaluation[] + RunSatisfaction[]
  -> ValidationResult[]
  -> GateDecision
  -> EvidenceBundle
  -> ReviewPack + ReworkDirective
```

필수 입력은 다음과 같다.

| 입력 | 사용 목적 |
|---|---|
| current TaskSpec·accepted ScopeRevision | 요청, include/exclude, 완료 조건과 사용자 결정을 고정 |
| project별 ChangePlan | planned change unit·postcondition·expected path·permission·risk 연결 |
| `readiness=ready` ValidationPlan | 이미 선택된 Check·scope·fallback·CheckGraph·TaskInvocation 사용 |
| planning-baseline ChangeSet | 작업 전 preexisting dirty change와 task 관계 구분 |
| current WorkspaceSnapshot과 observed-after-change ChangeSet | 실제 add·modify·delete·rename·mode·binary 변화 판정 |
| ProjectCatalogSnapshot·CodeIndexSnapshot freshness proof | ownership·generated·test·docs·architecture rule의 evidence quality 판정 |
| CatalogSnapshot·ValidatorRegistrySnapshot·ToolRegistrySnapshot | Rule·Check·Tool·parser·fingerprint contract 고정 |
| EffectiveConfig와 GatePolicy snapshot | fail threshold, ratchet, retry, review와 resource limit 고정 |
| CompletionClaim set | 사용자가 제출한 변경·검사·호환·수정 완료 주장 대조 |
| active/superseded Baseline과 Suppression revision set | existing/new/worsened와 예외 상태 계산 |

M3는 M2 input을 수정하지 않는다. current probe가 다르면 M3 output을 억지로 맞추지 않고 `replan_required`를 반환한다.

## application use case와 side effect

| use case | 주요 입력 | 결과 | 대상 project source effect |
|---|---|---|---:|
| `validation.preflight` | ValidationPlan, ChangePlan, current probes, Registry snapshots | 실행 가능 CheckGraph 또는 fail-closed reason | 없음 |
| `validation.collect_changes` | base/current snapshot, ScopeRevision, ChangePlan, optional PatchSet | observed ChangeSet과 scope comparison | 없음 |
| `validation.execute` | preflight token, CheckGraph, PermissionPlan | ValidationRun과 raw ArtifactRef | CheckDescriptor 선언에 따름 |
| `validation.normalize` | ValidationRun raw result, descriptor mapping | Diagnostic과 normalized outcome | 없음 |
| `validation.evaluate` | Diagnostic, Baseline, Suppression, policy | DiagnosticEvaluation·RunSatisfaction | 없음 |
| `validation.decide` | exact result set, claim evaluation, policy snapshot | GateDecision | 없음 |
| `validation.package_evidence` | committed result·decision refs | EvidenceBundle·ReviewPack·ReworkDirective | `.ai-runs` evidence write만 |
| `validation.inspect` | immutable refs와 render option | 근거·누락·remaining risk view | 없음 |

`validation.execute`가 source를 바꿀 가능성이 있는 도구를 호출하려면 CheckDescriptor가 side effect와 Permission action을 선언하고 M2 계획과 현재 PermissionPlan이 이를 허용해야 한다. 일반 test/build가 build output을 만들 수 있어도 source file, Git metadata와 shared declaration 변경은 별도 effect다. 선언되지 않은 effect를 발견하면 실행을 중단하고 `VALIDATION_UNDECLARED_SIDE_EFFECT`로 차단한다.

## 공통 Gate 상태 흐름

```text
requested
  -> preflighting_inputs
  -> collecting_actual_changes
  -> preflighting_execution
  -> evaluating_scope_and_claims
  -> executing_checks
  -> normalizing_results
  -> evaluating_baseline_and_suppressions
  -> deciding
  -> decision_committed(auto_pass | human_review | block)
  -> packaging_evidence
  -> completed_auto_pass | completed_human_review | completed_block
     | evidence_packaging_failed | invalidated | failed
```

- `invalidated`는 source, plan, config, Catalog, Rule 또는 Tool identity가 실행 중 바뀌어 같은 판단을 계속할 수 없다는 뜻이다.
- `failed`는 Gate engine 자체의 contract/invariant failure다. 프로젝트 Diagnostic 때문에 `failed`를 사용하지 않는다.
- `evidence_packaging_failed`는 이미 commit된 GateDecision을 다시 쓰지 않지만 Run·Stage 자동 완료 projection을 만들 수 없는 terminal orchestration 상태다.
- retry는 새 ValidationRun attempt를 만들며 기존 attempt와 Diagnostic을 삭제하지 않는다.
- `auto_pass`, `human_review`, `block` 뒤 같은 instance를 다시 열지 않는다. 새 evidence나 사용자 결정은 새 GateDecision ID와 이전 decision ref를 만든다.

## 1. preflight 알고리즘

preflight는 project tool process를 시작하기 전에 다음 순서를 그대로 수행한다.

application service는 이를 두 pass로 실행한다. 첫 pass는 1~3의 Schema·ref coherence만 확인해 read-only ChangeSet 수집이 안전한지 정한다. 이어 [실제 ChangeSet 재수집](#실제-changeset-재수집)을 수행하고, 둘째 pass가 새 WorkspaceSnapshot·ChangeSet을 입력으로 1~12 전체를 다시 확인해 single-use in-memory execution token을 만든다. 두 pass 사이에는 project tool process를 시작하지 않으며 수집 실패·partial 결과로 token을 만들지 않는다.

1. ValidationPlan Schema/version과 `readiness=ready`를 확인한다.
2. TaskSpec과 ScopeRevision이 current이며 ScopeRevision이 `accepted`인지 확인한다.
3. 모든 ChangePlan의 TaskSpec·ScopeRevision·ImpactAnalysis·ChangeSet·ValidationPlan ref와 fingerprint가 일치하는지 확인한다.
4. 각 target Project·Checkout의 current WorkspaceSnapshot·observed ChangeSet을 `PhaseSubjectExpectation`과 비교한다. 일반/stage와 pre-apply는 exact current/before identity, post-apply는 PatchSet에서 계산한 expected-after operation·content fingerprint가 같아야 한다.
5. ValidationPlan `profile_refs`의 activation evidence·parent closure·required family union과 `profile_resolution_fingerprint`가 actual change class·CatalogSnapshot에 여전히 맞는지 확인한다.
6. CatalogSnapshot의 CheckDescriptor·Rule·GatePolicy와 현재 ValidatorRegistrySnapshot hash가 같은지 확인한다.
7. 각 ToolDescriptor의 current descriptor hash, executable identity·version·hash, trust, platform과 availability를 확인한다.
8. CheckGraph가 acyclic이고 모든 required Check의 dependency closure가 실행 가능한지 확인한다.
9. 모든 TaskInvocation의 executable, args, cwd, env ref, timeout, output limit과 scope binding이 typed field로 완전히 해석되는지 확인한다.
10. required candidate에 unresolved, unaccepted waiver, permission block과 fallback floor 위반이 없는지 확인한다.
11. expected artifact budget, process concurrency와 PermissionPlan이 EffectiveConfig 제한 안인지 확인한다.
12. 위 의미 입력을 canonical hash한 `preflight_fingerprint`와 current probe 시각을 만든다.

실행 직전 `preflight_fingerprint`를 한 번 더 계산한다. 값이 달라졌으면 process를 시작하지 않는다.

| 실패 종류 | 결과 |
|---|---|
| 일반/stage source·WorkspaceSnapshot 변경 | `VALIDATION_SUBJECT_CHANGED`, M2 `replan_required` |
| pre-apply before identity 불일치 | `PATCH_PRECONDITION_FAILED`, source effect 금지·`BLOCK` |
| post-apply expected operation·after identity 불일치 | `PATCH_POSTCONDITION_FAILED`와 B01 Diagnostic, `BLOCK`·복구 판단 |
| plan/ChangePlan coherence 불일치 | `VALIDATION_PLAN_INCOHERENT`, `BLOCK` |
| actual change class·Profile closure 불일치 | `VALIDATION_PROFILE_CLOSURE_STALE`, M2 `replan_required` |
| Rule·Check·Catalog 변경 | `VALIDATION_CATALOG_STALE`, `replan_required` |
| Tool descriptor·executable 변경 | `VALIDATION_TOOL_STALE`, `replan_required` 또는 trust block |
| required Check unresolved·unbound | `VALIDATION_REQUIRED_CHECK_UNRESOLVED`, `BLOCK` |
| CheckGraph cycle·missing node | `VALIDATION_CHECK_GRAPH_INVALID`, `BLOCK` |
| permission·cost 승인 없음 | `VALIDATION_PERMISSION_BLOCKED`, 실행하지 않고 `HUMAN_REVIEW` 또는 policy deny `BLOCK` |

runner는 preflight 실패를 검사 결과 `pass`로 만들지 않는다. process를 시작하지 않은 항목은 모두 `outcome=not_run`과 reason code를 가진다.

## 2. 실제 변경·범위·주장 대조

### 실제 ChangeSet 재수집

M3는 보고서나 PatchSet의 file list를 실제 변경으로 간주하지 않는다. Gate 시점의 ProjectRevision과 filesystem byte에서 project별 `observed_after_change` ChangeSet을 다시 수집한다.

1. planning-baseline ChangeSet의 base ProjectRevision·WorkspaceSnapshot을 확인한다.
2. current filesystem byte, staged·unstaged·untracked metadata와 delete tombstone을 수집한다.
3. rename은 Git exact/heuristic 근거와 before/after content identity를 함께 보존한다. rename을 delete+add로만 보았으면 그 limitation을 남긴다.
4. binary·mode·submodule 변화도 entry로 유지한다.
5. preexisting, task-declared, tool-applied와 unknown origin을 보존한다.
6. source 관찰이 partial이면 empty ChangeSet이나 complete 비교를 만들지 않는다.

### expected와 actual 비교

각 actual entry는 accepted ScopeRevision과 ChangePlan을 기준으로 다음 중 하나다.

| relation | 의미 | 기본 Gate 영향 |
|---|---|---|
| `planned_exact` | planned unit의 path·operation·postcondition과 일치 | 다음 검사로 진행 |
| `planned_different_operation` | path는 같지만 add/modify/delete/rename 종류가 다름 | `BLOCK` |
| `necessary_expansion_accepted` | accepted ScopeRevision의 근거 있는 확장 | 다음 검사로 진행 |
| `missing_expected_change` | required planned unit의 실제 변화 없음 | `BLOCK` 또는 의미 확인 `HUMAN_REVIEW` |
| `unexpected_in_scope` | include 안이지만 어떤 unit에도 연결되지 않음 | severity에 따라 review/block |
| `out_of_scope` | accepted planned change scope 밖 | 기본 `BLOCK` |
| `preexisting_unchanged` | 시작 전 사용자 변경이 byte 동일하게 보존됨 | 정보·오염 위험만 기록 |
| `preexisting_modified` | task가 보존해야 할 사용자 변경이 추가로 바뀜 | 기본 `BLOCK` |
| `unknown` | 관찰·ownership·rename 제한으로 분류 불가 | `HUMAN_REVIEW`, critical path면 `BLOCK` |

보고된 변경 목록은 `CompletionClaim(kind=change)`으로 정규화한다. 실제 ChangeSet과 비교해 다음 상태를 만든다.

| claim status | 조건 |
|---|---|
| `verified` | 같은 project·path·operation·after fingerprint의 current evidence 존재 |
| `contradicted` | 실제 entry가 없거나 operation·after identity가 다름 |
| `unverified` | 관찰 scope·tool output이 partial이라 확인 불가 |
| `stale` | claim evidence가 다른 revision·WorkspaceSnapshot·config에 묶임 |
| `not_applicable` | typed claim 조건이 current TaskSpec에 적용되지 않음이 complete evidence로 확인됨 |

`contradicted` 완료 주장은 severity와 무관하게 자동 통과를 막는다. `unverified`·`stale` 주장은 사실로 렌더링하지 않고 ReviewPack의 미확인 표에 둔다.

### B01 stable rule ID

| Rule ID | 기본 severity/confidence | 기본 Gate floor | 관찰 |
|---|---|---|---|
| `star.validation.scope.out-of-scope-change` | `error/high` | `block` | accepted planned scope 밖 actual entry |
| `star.validation.scope.unexpected-change` | `warning/high` | `human_review`; protected scope면 block | include 안이지만 어떤 planned unit에도 연결되지 않은 actual entry |
| `star.validation.scope.operation-mismatch` | `error/high` | `block` | 보고·plan과 actual add/modify/delete/rename 불일치 |
| `star.validation.scope.rename-unverified` | `warning/high` | `human_review` | delete+add를 rename으로 확정할 근거가 부족함 |
| `star.validation.scope.missing-required-change` | `error/high` | required postcondition이면 block | required unit의 actual postcondition 부재 |
| `star.validation.scope.preexisting-change-modified` | `critical/high` | `block` | 사용자 기존 변경 보존 실패 |
| `star.validation.claim.contradicted` | `error/high` | required claim이면 block | current evidence와 완료 주장 충돌 |
| `star.validation.claim.unverified` | `warning/high` | required claim이면 review/block | evidence 불충분·partial |
| `star.validation.claim.stale` | `error/high` | required claim이면 block | 다른 revision evidence 사용 |

Rule ID와 기본 severity는 Validator Registry가 소유한다. 이 표는 required built-in identity를 고정하며 구현 code에 문자열을 중복 하드코딩하지 않는다.

이 문서의 이후 Rule 표에도 같은 규칙을 적용한다. ID는 release built-in Registry 항목이며 message wording이 아니라 stable wire identity다. 기본 severity/confidence와 Gate floor는 project config가 낮출 수 없고, 의미를 바꾸면 Rule SemVer·definition fingerprint·필요 시 fingerprint contract version을 올린다. 기존 ID를 다른 현상에 재사용하지 않는다.

## 3. evidence subject binding과 stale 판정

모든 ValidationRun과 외부 evidence는 `EvidenceSubjectBinding`을 가진다. exact field는 [검사·완료·증거 계약](../contracts/validation-and-evidence.md#evidence-subject-binding)에서 소유한다.

binding 비교는 다음 의미 input을 모두 확인한다.

- ProjectId·CheckoutId·ProjectRevisionId·WorkspaceSnapshotId와 workspace content fingerprint
- ChangeSet/PatchSet ID·revision·fingerprint와 Gate phase
- TaskSpec·ScopeRevision·ImpactAnalysis·ValidationPlan ID·revision·fingerprint
- EffectiveConfig·GatePolicy fingerprint
- CatalogSnapshot·ValidatorRegistrySnapshot fingerprint
- Rule·CheckDescriptor·ToolRegistrySnapshot·ToolDescriptor·observed executable identity
- invocation input, nonsecret execution environment와 parser/normalizer contract fingerprint

다음 경우 evidence는 `stale`이다.

| 변화 | stale reason |
|---|---|
| source byte·workspace entry 변화 | `stale_source` |
| ValidationPlan·ChangePlan·ScopeRevision 변화 | `stale_plan` |
| EffectiveConfig·GatePolicy 변화 | `stale_config` |
| Rule·Check·parser·fingerprint contract 변화 | `stale_catalog` |
| ToolDescriptor·executable path identity·version·hash 변화 | `stale_tool` |
| OS·arch·toolchain·runtime·nonsecret environment 의미 변화 | `stale_environment` |
| current probe 자체를 완료하지 못함 | `unverified` |

timestamp가 최근이라는 이유로 current가 되지 않는다. 반대로 timestamp만 달라도 의미 fingerprint가 모두 같으면 stale로 만들지 않는다.

다른 revision의 evidence는 참고·history로 ReviewPack에 연결할 수 있지만 required positive evidence, regression after evidence 또는 `AUTO_PASS` 입력으로 사용할 수 없다.

## 4. Check 실행과 결과 정규화

### CheckGraph 실행

runner는 M2 CheckGraph를 다음처럼 소비한다.

1. stable CheckPlan ID byte-order로 ready queue를 만든다.
2. `requires`, `provides_input`, `must_run_after` edge를 지킨다.
3. optional Check가 required dependency이면 이미 M2에서 required로 승격됐는지 확인한다. 누락이면 실행하지 않고 replan한다.
4. parallel group과 EffectiveConfig 동시성 상한의 교집합만 사용한다.
5. required predecessor가 `unsatisfied`면 dependent는 `not_run`과 `dependency_unsatisfied`를 가진다.
6. independent group은 failure policy가 `continue_independent`일 때만 계속한다.
7. timeout·cancel 뒤 process tree와 side effect 상태를 확인한다. 결과를 모르면 자동 retry하지 않는다.
8. retry는 CheckDescriptor가 idempotent·retryable 조건과 최대 횟수를 선언한 경우만 허용한다.

마지막 retry만 남기지 않는다. 모든 attempt의 invocation, outcome, completeness, Diagnostic, stdout/stderr ArtifactRef와 termination reason을 보존한다.

### raw shell 금지

- CheckPlan은 trusted CheckDescriptor를 가리킨다.
- CheckDescriptor는 trusted ToolDescriptor와 typed argument binding을 가리킨다.
- runner는 `executable`과 `args[]`를 process API에 그대로 전달한다.
- shell/script host만으로 실행되는 검사는 direct script file을 허용하지 않는다. 등록된 native EXE, package-manager EXE의 typed task ID 또는 adapter EXE ToolDescriptor로 표현할 수 없으면 unresolved다.
- TaskSpec, Profile, CheckDescriptor와 evidence에 동적 `cmd /c <text>`, `powershell -Command <text>` 또는 자유 형식 command line을 저장하지 않는다.

### 외부 도구 Diagnostic 정규화

외부 tool adapter는 다음 순서로 결과를 변환한다.

1. process 시작 여부·exit code·termination·output limit을 기록한다.
2. CheckDescriptor가 고정한 parser contract로 structured output을 읽는다.
3. external rule/code를 stable Star-Control Rule ID로 매핑한다.
4. severity·confidence·location·message parameter·remediation을 typed mapping으로 변환한다.
5. project-relative path로 안전하게 바꿀 수 없는 위치는 opaque LocalPathRef로 분리한다.
6. raw output은 redaction 후 ArtifactRef로 저장하고 Diagnostic message에 복사하지 않는다.
7. 매핑하지 못한 error severity 결과가 있으면 버리지 않고 `star.validation.external.unmapped-diagnostic`을 `unverified`로 만든다.
8. parser crash·truncation·unknown schema면 completeness를 `partial|unverified`로 낮추고 `pass`를 금지한다.

외부 scanner 이름·고유 severity를 core enum으로 확장하지 않는다. 원래 tool ID·version·external code는 provenance field로 보존하고 공통 severity·confidence는 descriptor mapping에 따라 결정한다.

여러 producer가 같은 Rule fingerprint를 관찰해도 raw Diagnostic을 하나로 덮지 않는다. Gate는 fingerprint별로 group하고 각 producer·evidence ref를 보존한 채 가장 높은 protected severity/Gate effect를 사용한다. occurrence/count ratchet은 Rule comparison key의 unique occurrence를 세며 tool 수를 문제 수로 더하지 않는다. producer 간 결과가 모순되면 confidence를 임의 평균하지 않고 `unverified` 또는 `HUMAN_REVIEW`로 남긴다.

외부 정규화 실패의 required built-in Rule은 다음과 같다.

| Rule ID | 기본 severity/confidence | 기본 Gate floor | 조건 |
|---|---|---|---|
| `star.validation.external.unmapped-diagnostic` | `error/high` | required Check면 block | 알려지지 않은 error code·mapping 누락 |
| `star.validation.external.output-truncated` | `error/high` | required Check면 block | output limit 때문에 전체 결과를 해석 못함 |
| `star.validation.external.parser-failed` | `error/high` | required Check면 block | Schema·encoding·parser contract 위반 |
| `star.validation.external.location-unresolved` | `warning/high` | 위치가 필수면 review/block | project-relative location으로 안전하게 bind 못함 |

## 5. Diagnostic 생성 규칙

모든 B01~B07과 외부 도구 결과는 [공통 Diagnostic 계약](../contracts/errors-and-diagnostics.md#diagnostic-계약)을 사용한다.

### 필수 생성 순서

1. stable `RuleRef`의 Rule ID·version·definition fingerprint를 고정한다.
2. 관찰 사실만 message parameter로 만든다. baseline·suppression 판단은 아직 적용하지 않는다.
3. severity는 사실일 때의 영향, confidence는 그 사실일 가능성으로 계산한다.
4. location은 ProjectId + project-relative path + 1-based/exclusive-end range + optional symbol을 사용한다.
5. evidence ref에는 source range, ChangeSet entry, ValidationRun 또는 ArtifactRef를 연결한다.
6. rule별 fingerprint contract로 안정 anchor를 정규화한다.
7. remediation은 안전한 다음 행동, 자동 수정 가능 여부, 필요한 재검사를 구조화한다.
8. secret·개인 경로·민감 literal 원문과 그 hash는 Diagnostic·fingerprint·artifact에 저장하지 않는다.

### fingerprint 원칙

기본 input은 다음과 같다.

```text
Rule ID + fingerprint contract version
  + ProjectId
  + normalized ownership anchor
  + stable problem key
  + optional normalized external code
```

line number, message text, timestamp, absolute path, tool render order와 retry attempt는 기본 fingerprint에서 제외한다. 내용상 다른 assertion·dependency edge·Schema key를 한 문제로 합치지 않도록 rule별 `stable problem key`를 명시한다.

external code는 Rule fingerprint contract가 서로 다른 문제 종류를 구분한다고 선언할 때만 normalized token으로 포함한다. 여러 tool의 동등 code는 diagnostic mapping에서 같은 stable problem token으로 바꿔 같은 issue fingerprint를 만들고, 원래 code는 producer provenance에만 남긴다.

Rule definition이나 fingerprint contract가 바뀌면 이전 Baseline과 Suppression을 자동 적용하지 않는다. compatible migration이 명시되지 않으면 `incompatible|stale`로 처리한다.

## 6. Baseline ratchet

### 목적과 적용 경계

Baseline은 “이미 존재하므로 올바르다”는 선언이 아니다. complete Scan/Validation evidence에서 검토한 issue fingerprint와 당시 severity·scope·Rule identity를 고정해 **새 악화만 기본 차단**할 수 있게 하는 비교 기준이다.

기본 `ratchet` policy는 다음처럼 작동한다.

| relation | 의미 | 기본 처리 |
|---|---|---|
| `new` | active compatible baseline에 없고 현재 관찰됨 | threshold 이상이면 `BLOCK` |
| `existing_unchanged` | 같은 fingerprint·severity·scope로 계속 관찰 | 숨기지 않고 remaining risk; ratchet-eligible Check만 만족 가능 |
| `worsened` | severity 상승, 범위 확대, occurrence 증가 threshold 초과 또는 더 위험한 evidence | 새 악화로 `BLOCK` |
| `improved` | severity·scope·occurrence가 감소 | 정보로 기록, baseline 자동 수정 금지 |
| `not_observed` | complete current scope에서 보이지 않음 | resolved candidate, baseline 자동 삭제 금지 |
| `incompatible` | Rule/fingerprint/scope/config 의미가 달라 비교 불가 | `HUMAN_REVIEW` 또는 새 baseline 요구 |
| `unbaselined` | baseline이 없거나 current scope를 포함하지 않음 | 모든 current issue를 new로 취급 |

baseline contract를 비교할 수 없는 사실은 `star.validation.baseline.incompatible` (`warning/high`, 기본 `HUMAN_REVIEW`)로 기록한다. baseline 부재 자체는 오류가 아니며 `unbaselined` relation과 current issue의 원래 Rule ID를 사용한다.

비교 알고리즘은 다음 순서다.

1. ProjectId, coverage scope, RuleRef/fingerprint contract, comparison contract, config와 Validator Registry가 모두 compatible한 active Baseline revision 하나를 선택한다. 둘 이상 동률이면 임의 선택하지 않고 `incompatible`이다.
2. BaselineEntry와 current issue를 `rule_id + stable ownership/scope comparison key`로 group한다.
3. 같은 issue fingerprint는 severity·scope·count를 비교해 `existing_unchanged|worsened|improved`를 정한다.
4. fingerprint는 다르지만 Rule이 versioned comparison key migration을 선언한 항목만 같은 issue lineage로 비교한다. mapping이 없으면 current는 `new`, old entry는 `not_observed`다.
5. 한 current issue가 여러 entry와 match하거나 반대면 해당 group은 `incompatible`이며 가장 유리한 match를 고르지 않는다.
6. unmatched current issue는 `new`, complete current coverage에서 unmatched baseline entry는 `not_observed`다. coverage가 partial이면 `not_observed`를 만들지 않는다.

severity order, scope widening과 occurrence/count threshold는 Rule의 comparison contract가 소유한다. message text·line 이동·timestamp만으로 worsened/new를 만들지 않는다.

### 기존 부채 onboarding

ratchet을 처음 도입한다고 현재 issue를 자동 Baseline으로 복사하지 않는다.

1. source·config·Catalog·Validator Registry가 고정된 trusted revision에서 complete `report_only` Scan/Validation을 실행한다.
2. 모든 current issue, coverage와 RuleRef를 가진 Baseline candidate를 별도 application command로 만든다.
3. 사용자가 candidate diff·redaction·scope·remaining risk를 검토하고 exact candidate fingerprint를 승인한다.
4. shared면 review된 source change, local이면 management transaction으로 새 active Baseline revision을 publish한다.
5. **다음** Gate부터 `ratchet_new_and_worsened`를 적용한다. candidate를 만든 현재 Gate 결과를 소급해서 pass로 바꾸지 않는다.

validator/policy/test harness를 함께 바꾼 task, partial·stale·flaky run, active secret critical과 protected invariant failure에서는 candidate를 만들 수 없다. 이 절차로 기존 부채는 명시적으로 보이게 유지하면서 이후 new·worsened만 차단한다.

### ratchet 만족 조건

raw Check outcome과 Gate satisfaction을 분리한다. 다음을 모두 만족할 때만 failed diagnostics-based Check를 `ratchet_satisfied`로 분류할 수 있다.

1. CheckDescriptor가 `ratchet_eligible=true`를 명시한다.
2. 실행은 실제로 시작됐고 output parsing·scope coverage가 `complete`, freshness가 `current`다.
3. launch error, timeout, crash, output truncation, unknown external code가 없다.
4. 실패를 만든 모든 Diagnostic이 `existing_unchanged` 또는 policy가 허용한 active Suppression이다.
5. validator guard, secret critical, functional test, build/compile, migration invariant와 regression-before/after Check가 아니다.
6. flaky attempt가 없다.

`ratchet_satisfied`는 ValidationRun을 `pass`로 바꾸지 않는다. ReviewPack에는 raw `fail`과 “기존 부채만 관찰돼 Gate ratchet 충족”을 함께 표시한다.

`not_observed`는 과거 Diagnostic을 `resolved`로 덮어쓰지 않는다. current Diagnostic이 없으므로 BaselineEntry를 subject로 한 DiagnosticEvaluation을 새로 만들고 issue lifecycle projection만 갱신한다.

## 7. Suppression

Suppression은 exact 문제 또는 제한된 Rule·scope에 적용하는 versioned 예외다.

필수 조건은 다음과 같다.

- stable Suppression ID와 revision
- `shared|local` 정본 위치와 ProjectId
- exact Diagnostic/Finding fingerprint 또는 Rule ID + project-relative scope selector
- 이유 code와 비어 있지 않은 설명
- 대상 Rule definition·fingerprint contract와 subject/config constraint
- 생성자 ActorRef, created_at, expires_at
- permanent인 경우 `allow_permanent_suppressions=true`인 상위 PolicyProfile, 별도 justification과 exact 승인

적용 순서는 `parse -> scope normalize -> subject/rule fingerprint compare -> expiry -> revoked/stale check`다. 하나라도 실패하면 active match가 아니다.

| suppression state | Gate 처리 |
|---|---|
| `active` | Diagnostic을 계속 표시하고 suppression ref·이유·만료를 연결. policy가 허용한 ratchet에만 사용 |
| `expired` | current issue를 unsuppressed로 평가하고 `star.validation.suppression.expired` 추가 |
| `stale` | Rule·source·config 의미 불일치. 자동 적용 금지, review |
| `revoked` | 적용 금지, history만 보존 |
| `invalid` | declaration Diagnostic과 Gate block/review |

Suppression은 severity, confidence, raw outcome과 evidence를 바꾸지 않는다. suppression file을 넓히거나 만료를 늘린 변경은 validator guard 대상이다.

suppression lifecycle Rule ID는 `star.validation.suppression.expired` (`warning/high`), `star.validation.suppression.stale` (`warning/high`), `star.validation.suppression.invalid` (`error/high`)로 고정한다. 원래 issue가 blocking이면 expired/stale도 원래 issue를 unsuppressed로 평가해 차단하고, declaration 자체가 invalid이면 기본 `BLOCK`이다.

## 8. B03 validator·policy·test harness 자기보호

### 보호 surface

M3는 다음 source class/ownership을 `validator_surface`로 분류한다.

- `star-validation`, `star-checks`와 Diagnostic normalizer
- Rule·CheckDescriptor·GatePolicy·RiskPath·Profile metadata
- ToolDescriptor result parser·diagnostic mapping
- test runner wrapper, assertion helper, fixture loader와 retry/timeout policy
- Corpus manifest·expected Diagnostic·baseline fixture
- Schema generator·contract conformance와 CI required command 정의
- shared suppression·baseline declaration

### two-snapshot guard

현재 변경된 validator가 자기 자신을 통과시키는 단일 판정은 허용하지 않는다.

1. pre-change trusted Catalog/Rule snapshot과 current candidate snapshot을 모두 고정한다.
2. 변경 의미 diff는 last-known-good guard implementation 또는 release에 포함된 immutable guard rule로 평가한다.
3. current candidate validator의 self-test 결과는 추가 evidence일 뿐 유일한 통과 근거가 아니다.
4. guard implementation 자체가 변경되면 parent guard와 contract/corpus golden이 그 변경을 평가한다.
5. bootstrap root인 built-in guard identity·minimum severity·required fixture rule은 product invariant이며 project config로 낮출 수 없다.

trusted side 선택 순서는 다음으로 고정한다.

1. 현재 작업의 planning-baseline에서 이미 active였고 hash가 검증된 ValidatorRegistrySnapshot
2. Controller가 보존한 같은 product contract major의 last-known-good snapshot과 producer binary identity
3. release binary에 포함된 `GuardMinimumManifest`

workspace의 current candidate file, 이번 변경에서 갱신한 fixture와 아직 active publish되지 않은 Registry는 trusted side 후보가 아니다. 1~3을 하나도 검증할 수 없으면 protected validator change는 `star.validation.guard.self-approval`과 `BLOCK`이며 clean project의 일반 Check로 fallback하지 않는다.

`GuardMinimumManifest`는 protected Rule/Check ID, minimum version·severity/confidence, immutable applicability floor, required fixture kind, forbidden ratchet family와 manifest fingerprint만 가진다. analyzer code·raw shell·allowlist를 data로 넣지 않는다. 제품 release에서 guard engine 자체가 바뀌면 이전 release guard로 candidate manifest·Corpus를 검사하고, 새 engine의 결과는 교차 evidence로만 사용한다. 이전 release를 실행할 수 없는 bootstrap 변경은 deterministic contract/golden과 별도 사람 review가 있어도 자동 통과하지 않고 `HUMAN_REVIEW` 또는 security floor `BLOCK`이다.

비교 결과 `GuardComparison`은 trusted/candidate Registry·producer fingerprint, changed protected field path, old/new typed value fingerprint, 적용 Rule ID, coverage, outcome과 evidence ref를 가진다. message text나 전체 config byte를 diff 정본으로 사용하지 않는다.

### 약화 변화

| 변화 | Rule ID | 기본 severity/confidence | Gate floor |
|---|---|---|---|
| Rule·required Check 삭제·disable | `star.validation.guard.required-rule-removed` | `error/high` | block |
| severity·confidence floor 하향 | `star.validation.guard.severity-lowered` | `error/high` | block |
| applicability·exclude·allowlist 확대 | `star.validation.guard.scope-weakened` | `error/high` | block |
| required command·dependency edge 제거 | `star.validation.guard.required-check-removed` | `error/high` | block |
| assertion 삭제·expected value 완화 | `star.validation.guard.assertion-weakened` | `error/high` | block |
| skip·ignore·only 추가 | `star.validation.guard.execution-bypassed` | `error/high` | block |
| timeout·retry 증가, stability minimum 하향·group key 축소 | `star.validation.guard.failure-masked` | `error/high` | block |
| baseline·suppression 범위/만료 확대 | `star.validation.guard.exception-expanded` | `error/high` | block |
| result parser가 error를 warning/pass로 변경 | `star.validation.guard.normalization-weakened` | `critical/high` | block |
| required fixture 누락·무단 기대값 재작성 | `star.validation.guard.fixture-missing` | `error/high` | block |
| changed guard의 self-test만 승인 근거 | `star.validation.guard.self-approval` | `critical/high` | block |

### Rule 변경 fixture gate

Rule, parser, fingerprint contract 또는 GatePolicy 변경은 최소 다음 fixture를 모두 요구한다.

| fixture | 증명할 내용 |
|---|---|
| positive | 허용해야 할 정상 입력을 진단하지 않음 |
| negative | 잡아야 할 대표 결함을 stable Rule ID로 진단 |
| edge | empty, boundary, encoding, path, partial·unsupported 같은 경계 처리 |
| regression | 과거 실제 결함 또는 우회가 다시 통과하지 않음 |

Rule 의미가 보안·권한·validator guard에 영향을 주면 adversarial fixture도 필수다. fixture manifest는 RuleRef, input hash, expected Diagnostic fingerprint/severity/status와 expected GateDecision을 고정한다. fixture 누락, 기대값 대량 재작성 또는 현재 변경에서 자동 생성된 승인 없는 baseline은 `BLOCK`이다.

CLI-only mode는 별도 AI reviewer를 요구하지 않는다. deterministic guard로 확정할 수 없는 의미상 정당성은 `HUMAN_REVIEW`로 남긴다.

## 9. B02 테스트 신뢰성

### 입력

- M2가 선택한 related test CheckPlan과 coverage 근거
- test source·fixture·snapshot ChangeSet entry
- owning production source와 `tests` edge
- pre-change test metadata와 current metadata
- 버그 수정이면 before-failure와 after-success evidence requirement

### 결정적 탐지 항목

| 변화 | Rule ID | 요구 evidence | 기본 판단 |
|---|---|---|---|
| test file/case 삭제 | `star.validation.test.case-deleted` | 삭제 이유, 대체 coverage, owning requirement mapping | `error/high`; 없으면 block |
| assertion 삭제·조건 완화 | `star.validation.test.assertion-weakened` | before/after assertion structure와 expected behavior mapping | `error/medium`; review/block |
| expected value를 current output에 맞춤 | `star.validation.test.expected-value-currentized` | requirement/bug evidence와 독립된 기대 근거 | `warning/medium`; 없으면 review |
| skip·ignore·disable 추가 | `star.validation.test.execution-bypassed` | issue ref, scope, 만료·재활성 조건 | `error/high`; required test면 block |
| focus·only 추가 | `star.validation.test.focused-only` | 전체 선택 결과에서 단독 실행 방지 | `critical/high`; block |
| timeout·retry 증가 | `star.validation.test.failure-masked` | 측정 근거와 flaky root cause | `warning/high`; 기본 review, 반복 은폐면 block |
| 대규모 snapshot 갱신 | `star.validation.test.snapshot-mass-change` | changed item count·semantic summary·review scope | `warning/high`; threshold 초과 review |
| related test 미선택 | `star.validation.test.related-check-unresolved` | M2 mapping/fallback 근거 | `error/high`; `not_found`면 block/replan |
| 새 test가 구현을 그대로 복제 | `star.validation.test.oracle-coupled` | independent oracle·invariant 여부 | `warning/medium`; 의미 검토 |

언어별 AST·test framework 지식은 adapter가 제공한다. text-only heuristic은 `suspected`·low/medium confidence이며 확정 assertion 약화로 표시하지 않는다.

snapshot mass 판정은 CheckDescriptor의 TestTrustPolicy를 사용한다. built-in 기본값은 snapshot file `5`개 이상, semantic item `100`개 이상, changed after byte 합계 `1,048,576` 이상 또는 전체 registered item 대비 `25%` 이상 중 하나다. 각 분모·측정 adapter·실제 값과 trigger threshold를 evidence에 남긴다. semantic item 수를 구할 수 없으면 file/byte threshold만 사용하고 item ratio를 0으로 추정하지 않는다. threshold를 올리거나 snapshot classifier 범위를 줄이는 변경은 `star.validation.guard.scope-weakened`다.

### 회귀 evidence

버그 수정의 required regression pair는 다음 조건을 만족한다.

```text
before evidence
  = 수정 전 subject fingerprint
  + 같은 test identity·input·environment
  + expected failure fingerprint

after evidence
  = 수정 후 current subject fingerprint
  + 같은 test identity·input·environment
  + pass outcome·complete·current·stable
```

before run을 안전하게 다시 실행할 수 없으면 과거 current evidence 또는 deterministic reproduction artifact를 사용할 수 있지만 compatibility가 exact해야 한다. 둘 다 없으면 “회귀 확인됨”으로 표시하지 않고 `HUMAN_REVIEW`다.

회귀 pair Rule ID는 `star.validation.test.regression-before-missing`, `star.validation.test.regression-after-incompatible`, `star.validation.test.regression-after-flaky`로 고정한다. 기본값은 모두 `error/high`이며 before 부재는 `HUMAN_REVIEW`, 다른 after binding은 `BLOCK`, protected regression의 flaky는 `BLOCK`이다.

## 10. B04 계약·아키텍처·하드코딩·생성물

### package dependency와 공개 경계

Project Catalog·Code Index의 current graph와 project Contract metadata를 사용해 다음을 검사한다.

- 허용 layer 방향과 금지 import
- package/module cycle
- private implementation을 건너뛴 import
- 공개 API·CLI·Schema·config·error code의 예상 밖 확대·삭제
- dependency manifest와 실제 graph의 drift

semantic graph가 없으면 syntax/declared adapter의 실제 tier를 기록하고 확정할 수 없는 dynamic/reflection edge는 `unverified`로 둔다. text match만으로 cycle·public boundary 위반을 확정하지 않는다.

결정적 graph 검사 순서는 다음과 같다.

1. 같은 CodeIndexSnapshot과 architecture policy fingerprint에서 package/module/export node와 typed edge를 ProjectId별로 정렬한다.
2. actual ChangeSet seed의 direct/transitive affected closure와 policy가 요구한 reverse consumer closure를 만든다.
3. `depends_on|imports` confirmed edge에 stable SCC 알고리즘을 적용한다. SCC identity는 정렬된 node key와 내부 edge key의 hash이며 단순 탐색 순서를 넣지 않는다.
4. before/current SCC를 비교해 새 cycle, node/edge가 늘어난 worsened cycle과 unchanged baseline cycle을 분리한다.
5. 각 confirmed import edge를 `(from layer/package, relation, to public/private owner)` forbidden policy와 exact match한다.
6. exported stable key별 before/current signature·visibility·consumer set을 비교해 add/remove/breaking/widening을 분리한다.
7. unresolved/dynamic edge frontier는 별도 limitation으로 남기고 0건 위반으로 합성하지 않는다.

multi-project edge는 exported entity와 ProjectId를 모두 identity에 넣는다. 다른 Project의 private symbol이나 raw path를 global graph에 복제하지 않는다.

| Rule ID | 기본 severity/confidence | 기본 Gate floor |
|---|---|---|
| `star.validation.architecture.forbidden-import` | `error/high` | new/worsened면 block |
| `star.validation.architecture.dependency-cycle` | `error/high` | new/worsened면 block |
| `star.validation.architecture.public-boundary-drift` | `error/high` | protected API면 block, 그 외 review |
| `star.validation.architecture.dependency-graph-drift` | `warning/high` | review; manifest invariant이면 block |

### hardcoding과 정본 drift

M1 hardcoding Finding은 candidate다. M3 validator는 다음 추가 evidence가 있을 때만 Gate Diagnostic으로 승격한다.

- authoritative config/contract/entity와 동일한 값이 source에 중복됨
- source ownership·public boundary를 벗어난 raw path·endpoint·command
- canonical document·Schema·Catalog와 구현 값이 불일치
- 허용된 fixture/docs-example/generated/vendor 분류가 아님

candidate 자체는 block하지 않는다. `warning|review` assessment, 별도 deterministic drift 결과 또는 policy threshold가 있어야 Gate에 영향을 준다.

hardcoding candidate는 `star.validation.hardcoding.candidate`의 `warning/medium`, authoritative drift가 증명되면 `star.validation.hardcoding.canonical-drift`의 `error/high`를 사용한다. 전자는 기본 `HUMAN_REVIEW`, 후자는 new/worsened이면 기본 `BLOCK`이다.

hardcoding 비교 key는 literal 원문이 아니라 `category + owning Project/source/symbol + canonical entity key + normalized use kind`다. endpoint·path·command·limit·error code·config value의 정본이 Catalog/Schema/contract에 등록돼 있고 current source 값이 다르거나 중복 소유될 때만 confirmed drift다. secret·credential·개인 절대 path는 candidate byte와 hash를 저장하지 않고 redacted category/location만 남긴다.

### generated source

1. generated classification과 `generated_by` provenance를 확인한다.
2. actual ChangeSet이 generated output만 바꾸고 source/generator input을 바꾸지 않았는지 확인한다.
3. 등록된 generator ToolDescriptor가 있으면 clean regeneration 결과와 current output fingerprint를 비교한다.
4. generator가 없거나 실행하지 못하면 direct edit를 성공으로 보지 않고 `HUMAN_REVIEW` 또는 protected path `BLOCK`으로 둔다.
5. source input 변경 뒤 generated output이 갱신되지 않은 상태와 output 직접 편집을 다른 Rule ID로 진단한다.

Schema·generated reference는 typed contract source에서 재생성한 diff가 0이어야 한다. 생성된 file을 직접 편집해 diff를 없애는 것은 통과가 아니다.

| Rule ID | 기본 severity/confidence | 기본 Gate floor |
|---|---|---|
| `star.validation.generated.direct-edit` | `error/high` | block |
| `star.validation.generated.output-stale` | `error/high` | block |
| `star.validation.generated.provenance-missing` | `warning/high` | human review; protected path면 block |
| `star.validation.generated.schema-drift` | `error/high` | block |

### Managed Registry contract·consumer

Registry 검사는 current Git manifest를 authoritative input으로 사용한다. DB ManagedRegistrySnapshot은 query·impact용 derived Index이고 source fingerprint와 다르면 `stale_registry_index`다. stale row를 source 기대값으로 사용하거나 source를 DB 값으로 고치지 않는다.

결정적 검사 순서는 다음과 같다.

1. root와 명시 fragment의 Schema·hash·namespace claim·owner를 확인한다.
2. stable declaration ID, kind별 public uniqueness scope와 영구 tombstone에서 duplicate·collision·reuse를 검사한다.
3. lifecycle transition과 AliasRecord의 replacement·consumer scope·유한 version window·cycle을 검증한다.
4. M1 current Index의 definition/reference, Schema·documentation·generated output binding을 BindingSpec과 비교한다.
5. consumer별 current version, minimum supported version, accepted declaration version, required binding과 migration deadline을 계산한다.
6. codegen output은 authoritative input·generator identity·declared output manifest와 비교하고 direct edit를 별도 위반으로 남긴다.
7. removal은 모든 required consumer 전환, old/alias reference 0건, alias window 종료와 complete evidence가 아니면 차단한다.

| Rule ID | 기본 severity/confidence | 기본 Gate floor |
|---|---|---|
| `star.validation.registry.binding-drift` | `error/high` | required binding이면 block |
| `star.validation.registry.consumer-not-migrated` | `error/high` | deprecate/removal change면 block |
| `star.validation.registry.deprecated-reference` | `warning/high` | window 안 review; deadline 뒤 block |
| `star.validation.registry.removed-reference` | `error/high` | block |
| `star.validation.registry.alias-window-expired` | `error/high` | block |
| `star.validation.registry.generated-output-stale` | `error/high` | block |
| `star.validation.registry.generated-direct-edit` | `error/high` | block |
| `star.validation.registry.docs-schema-drift` | `error/high` | block |

`candidate` 자체와 `local_implementation_constant`는 이 Rule family의 ownership 위반이 아니다. candidate promotion에는 사용자의 분류·owner 승인이 필요하고, 같은 raw 값만으로 managed declaration을 합치지 않는다. error display message만 바뀐 경우 stable code mismatch를 만들지 않지만 meaning·owner·recovery가 달라진 code는 새 declaration을 요구한다.

## 11. B07 문서·설정·환경 정적 검사

문서 Gate는 최소 다음을 검사한다.

- Markdown local link와 anchor가 실제 file/heading을 가리키는지
- 문서에서 참조한 command가 등록된 Tool/Task/CLI descriptor에 존재하는지
- code fence와 config example이 문법·Schema를 만족하는지
- config key·default·required field와 generated reference가 EffectiveConfig Schema와 일치하는지
- CLI·Schema·Catalog 목록과 canonical docs의 drift
- Windows path, line ending, encoding, case-sensitivity와 clean environment limitation

문서 command를 검사하기 위해 문서의 shell text를 그대로 실행하지 않는다. command example parser가 executable/args를 typed candidate로 만들고 Catalog의 known command와 exact match할 때만 등록된 ToolDescriptor를 실행한다. destructive·network·paid command example은 정적 검증만 하거나 별도 PermissionPlan을 요구한다.

문서 링크 존재 검사는 결정적이다. 자연어 내용의 정확성처럼 결정적 도구로 확정할 수 없는 항목은 `HUMAN_REVIEW`이며 CLI-only에서 AI reviewer를 자동 호출하지 않는다.

문서 CheckDescriptor는 Markdown dialect/parser version, heading slug contract, repository logical case policy, config/code-fence language mapping과 command example grammar를 고정한다. relative link는 문서 directory에서 ProjectPathRef로 normalize하고 root escape·drive·UNC를 거부한다. fragment는 대상 문서의 같은 parser/slug version으로 만든 anchor set과 비교하며 duplicate heading suffix도 보존한다. Windows에서 파일이 열리더라도 repository logical case가 다르면 case-drift Diagnostic을 만든다.

config example은 fence metadata가 가리킨 Schema ID/version으로 parse하고 unknown field·default·required constraint를 확인한다. CLI/schema/generated reference는 generated manifest의 item ID·hash와 비교한다. parser가 없거나 fence language를 모르면 실행하지 않고 `unverified`로 남긴다.

| Rule ID | 기본 severity/confidence | 기본 Gate floor |
|---|---|---|
| `star.validation.docs.broken-link` | `error/high` | block |
| `star.validation.docs.broken-anchor` | `error/high` | block |
| `star.validation.docs.command-unregistered` | `error/high` | required example이면 block |
| `star.validation.docs.command-unsafe` | `warning/high` | human review |
| `star.validation.docs.config-example-invalid` | `error/high` | block |
| `star.validation.docs.schema-drift` | `error/high` | block |
| `star.validation.docs.generated-reference-drift` | `error/high` | block |
| `star.validation.docs.environment-unverified` | `warning/high` | required environment이면 human review/block |

## 12. B05 secret·위험 command와 외부 scanner

기본 built-in 경량 검사는 다음을 탐지한다.

- 알려진 credential format·private key header·고위험 config key의 secret 후보
- source·docs·log·artifact의 raw 개인 절대 path와 credential 포함 URL
- dynamic shell, download-and-execute, recursive delete, force push, release publish 같은 위험 command 후보
- workflow 권한 확대와 unpinned external action 후보

secret 후보 원문과 hash를 persistence하지 않는다. 위치, category, redaction 상태와 source ownership만 남긴다.

전문 SAST, dependency vulnerability, license와 secret scanner는 ToolDescriptor/CheckDescriptor로 연결한다. Star-Control은 그 scanner나 취약점 DB를 다시 만들지 않는다. database timestamp·tool version·advisory source를 evidence에 남기고 stale DB 결과는 current 보안 통과 근거로 사용하지 않는다. M7의 `SupplyChainSnapshot`·`ExternalDataSnapshot` exact field와 dependency/update 경계는 [7단계 정본](../contracts/failure-security-and-dependency-maintenance.md)이 소유한다.

위험 command는 문맥 없이 문자열만으로 확정하지 않는다. docs example, fixture, test와 quoted data는 source class/facet에 따라 confidence를 낮추되, 실제 실행 경로·ToolDescriptor·workflow step과 연결되면 severity를 올린다.

secret detector는 source byte를 process memory에서만 검사하고 match byte·부분값·hash를 Diagnostic, log, Corpus와 artifact에 쓰지 않는다. Rule/category, ProjectPathRef·range, detector version, redaction 성공 여부만 남긴다. 위험 command detector는 parsed executable/argument/operator와 source facet을 사용하며 문자열 포함만으로 executable path를 확정하지 않는다.

외부 security CheckDescriptor는 advisory database identity·published/updated timestamp와 policy의 maximum age를 선언한다. timestamp가 없거나 age를 계산할 수 없으면 `unverified`, maximum age를 넘으면 `stale`이다. Star-Control은 advisory 내용을 복제하거나 자체 최신 여부 DB를 만들지 않고 tool evidence를 공통 Diagnostic으로만 정규화한다.

| Rule ID | 기본 severity/confidence | 기본 Gate floor |
|---|---|---|
| `star.validation.security.secret-candidate` | `critical`, confidence는 detector evidence | confirmed면 block, suspected면 human review |
| `star.validation.security.redaction-failed` | `critical/high` | block |
| `star.validation.security.dangerous-command-candidate` | `warning/medium` | human review |
| `star.validation.security.dangerous-command-executable` | `critical/high` | block |
| `star.validation.security.workflow-permission-widened` | `error/high` | protected workflow면 block, 그 외 review |
| `star.validation.security.external-action-mutable-ref` | `error/high` | release/protected workflow면 block, 그 외 review |
| `star.validation.security.external-database-stale` | `error/high` | required current security Check면 block |
| `star.validation.security.external-scan-unverified` | `error/high` | required Check면 block |
| `star.validation.security.release-manifest-incomplete` | `error/high` | required release면 block |

## 13. B06 실패 재현과 수정 전·후 증거

compile, test, runtime, tool과 environment failure는 다음을 같은 failure identity로 묶는다.

- stable Rule/external code
- normalized stack top 또는 owning symbol
- input/seed fingerprint
- environment·tool identity
- redacted failure signature

line·timestamp·random temp path는 fingerprint에서 제외한다. revision을 넘어 재발을 묶는 `family_fingerprint`와 exact revision·structured args·input·seed·environment·tool을 묶는 `occurrence_fingerprint`를 분리한다. retry마다 같은 family인지, 같은 exact occurrence인지, 새 failure인지 평가한다.

첫 원인은 확정 문자열이 아니라 evidence·confidence가 있는 `root_candidate`다. cascade Diagnostic은 cycle 없는 causality edge로 연결하며 단순 출력 순서만으로 원인을 확정하지 않는다.

ReproductionPack은 일반 run log와 별도인 curated manifest다. 최소 command ToolDescriptor ref, typed args, source revision, config/Catalog/tool fingerprint, input/seed, expected result, actual result와 redacted artifact를 가진다. `quarantined|unknown` artifact는 default report에서 제외하며 재현하지 못한 외부 조건은 `blocked_external|unverified`다. exact contract는 [7단계 정본](../contracts/failure-security-and-dependency-maintenance.md)이 소유한다.

수정 전 실패와 수정 후 성공은 같은 failure/test identity와 호환 환경에서 연결한다. after evidence가 다른 revision이거나 flaky면 회귀 성공으로 사용하지 않는다.

| Rule ID | 기본 severity/confidence | 기본 Gate floor |
|---|---|---|
| `star.validation.failure.reproduction-unverified` | `warning/high` | human review |
| `star.validation.failure.identity-changed` | `error/high` | required reproduction이면 block |
| `star.validation.failure.after-evidence-incompatible` | `error/high` | block |
| `star.validation.failure.after-flaky` | `error/high` | protected regression이면 block, 그 외 review |
| `star.validation.failure.recovery-plan-unverified` | `error/medium` | human review/block |
| `star.validation.failure.sensitive-artifact-unsafe` | `critical/high` | default report·completion block |

### M7 Maintenance Radar projection

Radar는 이 Gate의 입력이나 별도 completion engine이 아니다. common Finding·DiagnosticEvaluation·Suppression, RegressionRecord, Dependency/SupplyChain/ExternalData snapshot과 M6 drift를 읽어 `blocking/protected → risk → freshness → recurrence → evidence → due/age → stable ID`로 정렬한다. 같은 input과 `evaluation_time`에서 같은 순서가 나와야 하며, optional AI가 priority나 GateDecision을 바꾸지 못한다.

## 14. flaky와 false positive 정책

### flaky

같은 subject binding·CheckDescriptor content hash·Tool identity·typed invocation·normalizer fingerprint인 attempt group에서 서로 다른 pass/fail outcome이 관찰되면 `stability=flaky`다. 다른 revision·environment·args를 한 group으로 합치지 않는다.

- 모든 attempt를 보존한다.
- 마지막 pass만으로 `stable` 또는 `AUTO_PASS`를 만들지 않는다.
- required Check의 flaky는 기본 `HUMAN_REVIEW`다.
- validator guard, secret, migration invariant, release와 regression Check의 flaky는 기본 `BLOCK`이다.
- retry·timeout을 늘려 흔들림을 숨기면 별도 guard Diagnostic을 만든다.
- known flaky suppression도 만료와 exact fingerprint를 요구하며 raw 결과는 남긴다.

기본 `single_attempt` contract는 complete·current한 started attempt 하나를 `stable`로 볼 수 있다. `repeat_on_failure|sampled`는 CheckDescriptor의 minimum comparable attempts를 채워야 하며 미달이면 `not_evaluated`다. 어느 mode에서도 pass/fail 혼합을 stable로 만들 수 없다.

공통 표시 Rule ID는 `star.validation.stability.flaky` (`error/high`)다. 이 Diagnostic은 원래 pass/fail Diagnostic을 대체하지 않고 attempt sequence를 evidence로 가리킨다.

### false positive

false positive는 Rule 결과를 삭제하는 상태가 아니다.

1. 원래 Diagnostic과 evidence를 보존한다.
2. 사용자가 근거를 검토해 false-positive Disposition을 만들고, Gate effect를 완화해야 하면 별도 bounded Suppression을 만든다.
3. 같은 Rule의 FP count·rate와 scope를 EvaluationRun metric에 남긴다.
4. Rule을 고치면 positive/negative/edge/regression fixture와 기존 suppression 재평가를 요구한다.
5. broad allowlist로 경고를 없애는 변경은 validator guard가 검사한다.

Disposition만으로 Diagnostic을 pass·suppressed로 바꾸지 않는다. ReviewPack은 false-positive 판단, 연결된 Suppression 유무·만료와 raw Diagnostic을 함께 보여준다.

## 15. GateDecision 결정 알고리즘

### 입력 정렬과 순서

Gate engine은 CheckPlan ID, Diagnostic fingerprint, ProjectId, path와 stable ref를 byte-order로 정렬해 같은 입력에서 같은 결정을 만든다. 표시/audit timestamp와 표시 순서는 decision fingerprint에서 제외하지만 application이 주입한 `evaluation_time`, suppression·approval·external DB freshness의 `valid_until` 같은 semantic time input은 포함한다. pure Gate engine이 system clock을 직접 읽지 않는다.

판정 순서는 다음과 같다.

1. input/schema/coherence invariant 검사
2. evidence freshness와 completeness 검사
3. required Check별 RunSatisfaction 계산
4. Diagnostic baseline relation 계산
5. Suppression 상태 계산
6. new/worsened severity threshold 적용
7. validator guard·secret·protected invariant override 적용
8. claim contradiction·missing evidence 적용
9. flaky·manual observation·waiver·semantic review 적용
10. remaining risk와 decision fingerprint 생성

### `BLOCK`

다음 중 하나면 기본 `BLOCK`이다.

- ValidationPlan/ChangePlan coherence·CheckGraph·typed binding invariant 실패
- evidence가 다른 source revision·WorkspaceSnapshot·plan·config·Catalog·Tool·execution environment를 가리킴
- required Check의 `not_run`, `error`, `cancelled`, `partial`, `unverified`, stale 또는 `unsatisfied`가 `RunSatisfaction.gate_effect=block`으로 분류됨
- required Check의 actual failure가 ratchet-eligible 조건을 충족하지 못함
- fail threshold 이상의 new·worsened unsuppressed Diagnostic
- out-of-scope change, contradicted required completion claim 또는 preexisting user change 손상
- validator guard protected invariant 위반·fixture 누락
- confirmed secret critical, forbidden destructive command execution path 또는 redaction failure
- expired/stale suppression 뒤 다시 활성화된 blocking issue

### `HUMAN_REVIEW`

다음은 성공으로 바꾸지 않고 `HUMAN_REVIEW`다.

- 결정적 도구로 확정할 수 없는 의미·설계·문서 내용 검토
- suspected high-impact·low-confidence Diagnostic
- required Check의 flaky 결과
- explicit user waiver 또는 manual evidence requirement
- required Check가 ActionPolicy `prompt`의 permission/approval을 기다려 `not_run`이며 `gate_effect=human_review`임
- baseline compatibility가 불명확함
- optional Check 실패가 policy상 block은 아니지만 중요한 remaining risk임
- active suppression을 새 revision에서 갱신할지 판단해야 함
- CLI-only에서 Profile이 의미 검토를 요구함

### `AUTO_PASS`

다음을 모두 만족해야 한다.

1. preflight와 final current probe가 같은 fingerprint다.
2. 모든 required Check가 `clean_pass` 또는 허용된 `ratchet_satisfied`다.
3. 모든 required evidence가 complete·current이며 flaky가 아니다.
4. blocking new·worsened Diagnostic, contradicted claim과 expired/stale blocking suppression이 없다.
5. required manual observation·waiver·semantic review가 없다.
6. GateDecision input ref가 모두 commit됐고 decision graph에 순환 reference가 없다.
7. remaining risk가 policy가 자동 통과를 허용한 정보·existing debt 범위 안이다.
8. time boundary가 있으면 `time_source_state=verified`이고 현재 시각이 GateDecision `valid_until` 이전이며 boundary input이 current다.

`AUTO_PASS`는 모든 raw Check가 깨끗하게 exit 0이었다는 뜻이 아니다. ratchet-satisfied raw failure가 있으면 ReviewPack에 명확히 표시하고 GateDecision의 RunSatisfaction ref로 설명한다.

`AUTO_PASS` decision 뒤에도 EvidenceBundle과 ReviewPack packaging이 `complete`로 commit돼야 Run·Stage를 자동 완료할 수 있다. packaging 실패를 기존 decision의 `BLOCK`으로 다시 쓰지는 않지만 완료 projection을 만들지 않고 evidence packaging failure로 남긴다.

## 16. EvidenceBundle·ReviewPack·재작업 지시

### EvidenceBundle

기계 정본은 최소 다음 ref와 fingerprint를 가진다.

- TaskSpec·ScopeRevision·ChangePlan·ValidationPlan
- before/current ProjectRevision·WorkspaceSnapshot·ChangeSet
- preflight·final probe와 EvidenceSubjectBinding
- Catalog·ValidatorRegistry·ToolRegistry·EffectiveConfig·GatePolicy snapshot
- 모든 ValidationRun attempt와 raw ArtifactRef
- Diagnostic·DiagnosticEvaluation·RunSatisfaction
- Baseline·Suppression·Disposition·Waiver input revision
- GateDecision과 decision fingerprint
- 누락·redaction·quarantine·stale 이유

EvidenceBundle은 GateDecision을 참조하고 ReviewPack은 EvidenceBundle을 참조한다. GateDecision에서 두 산출물을 역참조하지 않아 immutable hash 순환을 만들지 않는다.

### ReviewPack

사람용 순서는 다음으로 고정한다.

1. `request_and_completion_criteria`: 요청 목표와 완료 조건
2. `planned_vs_actual_changes`: 계획한 변경과 actual add·modify·delete·rename 비교
3. `completion_claims`: verified·contradicted·unverified·stale 완료 주장
4. `check_results`: required/optional Check별 pass·fail·not_run·completeness·freshness·flaky
5. `diagnostic_relations`: new·worsened·existing·suppressed·expired Diagnostic 표
6. `quality_security_highlights`: validator/test/architecture/docs/security 핵심 문제
7. `gate_decision`: GateDecision과 정확한 reason code
8. `remaining_risks_and_questions`: remaining risk·질문·사용자 선택지
9. `evidence_identity`: EvidenceBundle ID·hash

ReviewPack은 raw result를 다시 해석해 사실을 바꾸지 않는다.

### ReworkDirective

`BLOCK`이면 다음만 포함한다.

- blocking Diagnostic ID·fingerprint
- failed/missing CheckPlan ID
- expected vs actual scope/claim 차이
- 안전한 remediation과 필요한 재검사
- 재계획이 필요한지 같은 plan에서 재실행 가능한지

source replacement text, raw shell과 자동 승인 지시는 넣지 않는다.

## 17. 4단계 Patch engine Gate

### 적용 전 `patch_pre_apply`

입력은 accepted ChangePlan, immutable PatchSet preview, current planning-baseline ChangeSet, `recipe_preview` ChangeSet, RecipeExecution preview·idempotence replay, WorktreeDecision, ready ValidationPlan과 current snapshots다. exact field와 준비 순서는 [안전한 Patch·Refactor·codemod 엔진 계약](../contracts/safe-patch-and-codemod.md)이 소유한다.

필수 검사:

1. PatchSet이 ChangePlan revision과 exact fingerprint로 연결됨
2. 모든 operation이 accepted planned change scope 안에 있음
3. actual before hash·mode·존재 상태와 PatchSet precondition 일치
4. preexisting change와 byte range·rename·delete overlap 없음
5. expected add·modify·delete·rename과 reported PatchSet summary 일치
6. validator/test/policy/generated/protected surface 변화에 guard rule 적용
7. secret·위험 command·generated direct edit의 preview 정적 검사
8. apply PermissionPlan·approval scope·rollback 가능성 확인
9. Recipe stable ID·version·definition fingerprint, validated input Schema와 resolved selector 고정
10. external transformer의 ToolDescriptor·package·executable identity·typed arguments와 redacted output evidence 고정
11. idempotence replay가 no-op이고 preview ChangeSet·M2 impact·selected Check reconciliation이 complete
12. WorktreeDecision과 preexisting manifest가 current이며 preview·forward·reverse artifact hash가 일치

자동 Patch 적용은 `patch_pre_apply` GateDecision이 `AUTO_PASS`일 때만 허용한다. `HUMAN_REVIEW`는 사용자의 exact PatchSet fingerprint 승인과 policy가 수동 진행을 허용할 때만 적용할 수 있으며 자동 수정으로 세지 않는다. `BLOCK`은 적용할 수 없다.

pre-apply decision 뒤 source, PatchSet, plan, config, Catalog, Tool 또는 approval scope가 달라지거나 `valid_until`에 도달하면 decision은 stale이며 apply 직전에 다시 계산한다.

Registry Patch의 pre-apply에는 추가로 before/expected-after ManagedRegistrySnapshot, source manifest·declaration·namespace/tombstone fingerprint, binding·consumer compatibility table와 alias/lifecycle evaluation을 고정한다. duplicate ID·namespace collision·ID reuse, invalid alias, unresolved required binding, stale/partial Index 또는 consumer 미전환이 있으면 permit을 만들지 않는다. DB/UI가 만든 change request도 이 검사를 우회할 수 없다.

source write port는 persisted GateDecision ID만으로 열리지 않는다. application service가 apply 직전 current binding set·PatchSet·PermissionPlan을 재검증해 process-memory `PatchApplyPermit`을 만든다. permit은 GateDecision ref, exact PatchSet fingerprint, before binding set fingerprint, permission/approval fingerprint와 `automatic|manual_approved` kind를 가지며 한 번만 사용할 수 있다. 직렬화·재사용·다른 PatchSet 전달은 금지한다. `automatic` permit은 `AUTO_PASS`에서만, `manual_approved` permit은 `HUMAN_REVIEW`와 exact 사용자 승인에서만 만들며 후자를 자동 성공 metric에 포함하지 않는다.

### 적용 후 `patch_post_apply`

Patch engine은 source effect 뒤 즉시 새 WorkspaceSnapshot과 `observed_after_change` ChangeSet을 수집한다.

필수 검사:

1. 실제 operation manifest와 PatchSet expected result 비교
2. 대상 밖 preexisting change byte 동일성
3. partial apply·undeclared file·unexpected generated output 탐지
4. M2가 선택한 모든 required Check 실행
5. test regression before/after, validator guard와 affected architecture/docs/security Check 실행
6. after evidence가 새 WorkspaceSnapshot·config·Catalog·Tool fingerprint와 exact 일치
7. final completion claim·EvidenceBundle·ReviewPack 생성

자동 완료는 `patch_post_apply`가 `AUTO_PASS`일 때만 가능하다. `HUMAN_REVIEW`는 적용 결과를 보존한 채 완료 대기, `BLOCK`은 실패/복구 상태다. rollback은 별도 exact reverse precondition과 PermissionPlan을 요구하며 Gate 실패만으로 사용자 기존 변경을 자동 되돌리지 않는다.

Registry Patch의 post-apply는 actual manifest와 definition/reference/generated output을 다시 scan해 actual-after ManagedRegistrySnapshot·RegistryConsistencyRecord를 만든다. expected-after와 exact 일치, complete required binding·consumer coverage와 blocking Registry Diagnostic 0건이 모두 있어야 `registry_current`를 verified로 만들 수 있다. cross-project consumer는 read-only evidence이며 9단계 전 다른 Project apply를 성공으로 묶지 않는다.

PatchApplication이 `partially_applied|outcome_unknown|recovery_required`이면 실제 after snapshot과 operation receipt를 보존하되 성공 Gate를 계산하지 않는다. 복구는 완료 receipt의 역순 reverse PatchSet 또는 Star-Control이 소유한 격리 worktree 폐기만 사용하며, primary checkout hard reset·사용자 변경 삭제·검사 생략으로 상태를 정리하지 않는다.

이 pre/post protocol은 4단계 Patch engine뿐 아니라 이후 ChangeRecipe, codemod, migration patch와 자동 수정 기능의 공통 mutation port 불변식이다. 자동 수정 handler는 source write port를 직접 호출할 수 없고 exact PatchSet으로 `patch_pre_apply`를 통과한 뒤에만 effect를 시작한다. post Gate와 complete evidence packaging 없이 “수정 완료” event를 만들 수 없다. 새 자동 수정 종류가 이 protocol을 우회하려면 기능 추가가 아니라 architecture violation이다.

### M11 Rust style candidate Gate

[Rust 코드 스타일 자동 교정](rust-code-style-auto-fix.md)은 공통 decision algorithm에 다음 typed input과 floor를 추가한다. M3가 cargo/rustfmt/Clippy command를 직접 조립하거나 lint를 선택하지는 않는다.

candidate pre-validation의 required input은 다음과 같다.

- current exact RustToolchainBinding과 project-pinned stable completeness
- RustStylePolicySnapshot과 exact Profile/pipeline/ToolDescriptor/parser definition hash
- package/target/feature/triple/cfg/ownership별 RustStyleCoverageMatrix
- ordered RustStyleStepExecution, raw/normalized Diagnostic·suggestion·hunk mapping과 step/final complete filesystem diff
- M2 preview impact reconciliation, full-pipeline idempotence replay와 candidate ValidationPlan

candidate가 `AUTO_PASS`하려면 공통 8개 조건과 다음을 모두 만족해야 한다.

1. pinned stable cargo/rustc/rustfmt/clippy-driver identity, parsing/style edition, MSRV, host/target와 config source가 complete·current다.
2. Catalog가 required로 선언한 모든 coverage cell이 실행됐고 `completeness=complete`다. inactive/unknown cfg·missing target를 0건으로 간주하지 않는다.
3. final fixed `cargo fmt <typed-scope> -- --check`가 no drift이고 required coverage 전체 Clippy check와 M2 selected build/test/contract Check의 RunSatisfaction이 충족된다.
4. Clippy actual fix hunk 각각이 exact allowlist lint ID의 `MachineApplicable` suggestion span/replacement와 대응한다.
5. 모든 step의 complete filesystem manifest에 handwritten in-scope `.rs` modify 이외 operation과 build script/proc macro side effect가 없다.
6. expected-after 새 preview에서 전체 `rust_style_v1` mutation replay operation이 0이다.
7. public API delta, dirty overlap/unknown, feature/target conflict, parser limitation, redaction/evidence gap과 stale binding이 없다.

allowlist 밖 Diagnostic이나 non-MachineApplicable suggestion은 수정하지 않고 ReviewPack에 남긴다. 그 존재만으로 project lint policy를 새로 강화하지는 않지만, 기존 Cargo/source lint level·selected Check·GatePolicy에서 blocking이면 그대로 `BLOCK`이다. M11이 `#[allow]`를 추가하거나 lint level을 내려 통과시킬 수 없다.

`patch_pre_apply`는 위 candidate binding과 PatchSet fingerprint, current source/dirty manifest, policy approval을 다시 확인한다. `safe_default`는 exact 사용자 approval이 필요하다. `personal_auto`는 user standing grant만으로 통과하지 않고 policy evaluator가 exact Project/Profile/pipeline/style policy/scope/action/diff/PatchSet/evidence에 묶어 해소한 ApprovalRequest가 있어야 한다. candidate 또는 pre Gate가 `HUMAN_REVIEW|BLOCK`이면 automatic permit을 만들지 않는다.

`patch_post_apply`는 actual operation이 PatchSet의 `.rs` modify와 exact 일치하고 대상 밖 byte가 보존됐는지 먼저 확인한 뒤, actual-after exact-byte isolated validation mirror에서 같은 resolved toolchain/config/coverage의 fmt check, Clippy check와 M2 affected Check를 다시 실행한다. mirror subject와 target actual-after binding이 다르면 stale이다. apply 중 tool/config/source drift, partial receipt, unexpected file, post Check 실패와 evidence packaging incomplete은 success가 아니라 stale/recovery state다. post Gate 실패를 자동 rollback 성공으로 바꾸지 않는다.

## 18. Profile 결합

세부 metadata는 [개발 작업 Profile](profiles.md)이 소유한다.

| Profile | 공통 Gate에 추가하는 것 |
|---|---|
| `ai_development_validation` | 모든 Codex 생성·수정 결과에 B01, evidence freshness, validator guard, ReviewPack과 pre/post Gate 필수 |
| `test_correctness` | B02, related test coverage, assertion·skip·retry·snapshot 약화, regression pair 필수 |
| `architecture_quality` | B04 dependency/cycle/public boundary/forbidden import, hardcoding·canonical drift, baseline ratchet 필수 |
| `api_contract_change` | Registry lifecycle·consumer compatibility·contract/docs/Schema drift, removal Gate 필수 |
| `refactor_codemod` | managed selector, generated ownership, exact PatchSet pre/post binding·idempotence 필수 |
| `rust_style_auto_fix` | pinned toolchain·style policy·coverage·step/hunk evidence, complete candidate check, replay, exact policy approval와 pre/post Gate 필수 |

여러 Profile의 required set과 위험 floor는 M2 계획 시 합집합·가장 엄격한 값으로 결합하고 ValidationPlan `profile_refs`·`profile_resolution_fingerprint`에 materialize한다. M3는 이 closure와 actual change class를 검증할 뿐 Check family를 추가·제거하거나 permission을 넓히지 않는다. closure mismatch는 실행 시 동적 보정이 아니라 `VALIDATION_PROFILE_CLOSURE_STALE`과 재계획이다.

## 19. Corpus와 conformance

각 built-in Rule case는 다음 manifest를 가진다.

- case ID·version·owner rule family
- fixture kind `positive|negative|edge|regression|adversarial`
- input Project/Workspace/Catalog/Tool/config fingerprint
- expected Diagnostic RuleRef·fingerprint·severity·confidence·location key
- expected baseline/suppression relation
- expected ValidationRun outcome/completeness/freshness/stability
- expected GateDecision과 reason code
- secret/redaction expectation

최소 공통 case는 다음을 포함한다.

- actual modify를 add로 보고한 claim, 누락된 delete, rename 오분류
- 다른 revision의 pass evidence와 current source drift
- required test `not_run`, partial parser output와 stale tool
- unchanged existing debt, new issue, severity worsened, improved/not-observed
- exact active suppression, expired·stale·broad suppression
- validator severity 하향, allowlist 확대, fixture expectation 대량 변경
- assertion 삭제, skip/only, timeout·retry 증가와 snapshot mass update
- before fail/after pass, after flaky와 다른 environment
- dependency cycle·forbidden import·public boundary 확대
- hardcoding candidate false positive와 confirmed canonical drift
- generated output direct edit와 source/generated mismatch
- broken Markdown link, unknown command, invalid config example와 Schema drift
- redacted secret, dangerous command docs example와 executable workflow path
- external tool unmapped Diagnostic와 truncated output
- CLI-only semantic review가 Codex 호출 없이 `HUMAN_REVIEW`가 되는 case
- patch pre-apply stale decision과 post-apply unexpected file
- Registry duplicate ID·namespace collision·ID reuse·alias cycle/window와 stale DB Index
- error message-only 변경, deprecated code alias, consumer 미전환 removal 차단과 removed-reference
- Registry generated output stale·direct edit와 docs·Schema·language binding drift

Corpus test가 실패했을 때 expected file을 현재 output에 맞춰 자동 갱신하지 않는다. 의미 변경과 review된 fixture diff가 있어야 한다.

## 20. Package 구현 경계

```text
star-application
  -> star-validation/preflight: input coherence와 current probe 비교
  -> star-validation/runner: CheckGraph·TaskInvocation 실행 조정
  -> star-checks/*: B01~B09 rule producer
  -> star-validation/normalize: 공통 Diagnostic 변환
  -> star-validation/ratchet: baseline·suppression·stability 평가
  -> star-validation/gate: pure GateDecision
  -> star-evidence: ArtifactRef·EvidenceBundle·ReviewPack export
```

- `star-validation/gate`는 filesystem, process, Git, DB와 external scanner를 직접 호출하지 않는 pure decision engine이다.
- `star-checks` module은 서로 import하지 않고 공통 contract를 통해 Diagnostic을 반환한다.
- external process I/O는 ToolExecutorPort adapter가 담당한다.
- persistence와 transaction은 `star-state`, 큰 byte·redaction은 `star-evidence`가 담당한다.
- CLI·MCP handler는 ValidationRun을 다시 집계하거나 GateDecision을 재해석하지 않는다.

## 21. 구현 순서

제품 구현은 다음 순서를 바꾸지 않는다.

1. M3 target contract type, Schema ID/version, valid/invalid/future-version fixture
2. Rule·Baseline·Suppression·Disposition v1→v2와 Diagnostic historical projection migration dry-run·backup·rollback fixture
3. Validator Registry·GatePolicy·Profile metadata conformance와 fingerprint golden
4. pure preflight·EvidenceSubjectBinding freshness·claim/scope comparator
5. fake ToolExecutor의 CheckGraph runner·attempt·timeout·output limit conformance
6. external Diagnostic normalizer와 unmapped/truncated/redaction fixture
7. DiagnosticEvaluation·baseline ratchet·suppression·RunSatisfaction pure engine
8. B01 change scope와 B03 validator guard의 first vertical slice
9. B02 test trust, B04 architecture/generated/hardcoding, B05 secret, B06 regression, B07 docs rule family
10. GateDecision·EvidenceBundle·ReviewPack·ReworkDirective transaction
11. `patch_pre_apply`·`patch_post_apply` fake Patch engine contract test
12. CLI-only E2E: AI dependency 0, source effect와 permission manifest 확인
13. Corpus 전체, false-positive·flaky metric과 Windows x64·ARM64 gate

## 설계 수용 기준

- 실제 ChangeSet과 작업 계약·보고된 add/modify/delete/rename을 같은 revision에서 비교한다.
- 실행하지 않은 Check, partial·unverified·stale·flaky evidence는 pass가 아니다.
- 다른 source/config/Catalog/Tool revision evidence로 `AUTO_PASS`할 수 없다.
- test·architecture·hardcoding·docs·security 결과가 같은 Diagnostic 계약을 사용한다.
- 기존 부채, 신규 문제, 악화, active suppression, expired/stale suppression이 독립 상태다.
- validator를 약화해 통과시키는 변경은 previous/current snapshot guard와 Corpus가 별도로 탐지한다.
- Rule 변경은 positive·negative·edge·regression fixture 없이는 Gate를 통과할 수 없다.
- M2 selected Check·scope를 runner가 재선택하지 않는다.
- raw shell이 검사 정본이 아니며 실행은 등록·신뢰된 ToolDescriptor를 통한다.
- false positive와 flaky가 evidence·ReviewPack·metric에서 사라지지 않는다.
- CLI-only 의미 검토는 AI 호출 없이 `HUMAN_REVIEW`로 남는다.
- 4단계 Patch engine의 적용 전·후 Gate와 invalidation 조건이 명확하다.
- Recipe·tool·preview·idempotence·worktree evidence와 partial apply 복구 상태가 Gate 입력에서 누락되지 않는다.
- 현재 상태가 문서 설계이며 제품 구현 완료가 아님을 모든 정본이 일관되게 표시한다.
