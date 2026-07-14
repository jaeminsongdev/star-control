# 상태 기록과 이어하기

## 목표

앱을 닫거나 작업이 실패해도 처음부터 다시 조사하지 않도록 목표, 단계, source 관찰, 결과와 다음 행동을 안전하게 저장한다. 동시에 source code와 공유 선언이 로컬 DB에 갇히지 않게 정본·projection·evidence를 분리한다.

Project·ScanRun·Finding·PatchSet·Baseline·Suppression과 관리 DB lifecycle은 [공통 개발 관리와 로컬 관리 DB 계약](../contracts/development-management.md), ProjectCheckout·ProjectCatalogSnapshot·CodeIndexSnapshot과 freshness 의미는 [Project Catalog·Code Index 계약](../contracts/project-catalog-and-code-index.md), TaskSpec·ScopeRevision·ImpactAnalysis·affected output은 [변경 계획·영향 분석 계약](../contracts/change-planning-and-impact.md), RecipeExecution·PatchApplication과 복구 순서는 [안전한 Patch·Refactor·codemod 엔진 계약](../contracts/safe-patch-and-codemod.md), ManagedDeclaration·manifest·binding·consumer·lifecycle은 [Managed Registry 계약](../contracts/managed-symbol-registry.md), ValidationRun·DiagnosticEvaluation·GateDecision·EvidenceBundle은 [검사·완료·증거 계약](../contracts/validation-and-evidence.md), M6 contract·docs·environment snapshot은 [계약 호환성·환경 계약](../contracts/contract-compatibility-and-environment.md), M7 failure·supply-chain·dependency·Radar snapshot은 [실패 재현·보안·의존성 유지보수 계약](../contracts/failure-security-and-dependency-maintenance.md), M8 migration·performance·language/platform document와 state machine은 [Migration·성능·언어·플랫폼 계약](../contracts/migration-performance-and-platform.md), 9단계 project/worktree/merge/remote coordination은 [CrossRepo ChangeBundle 계약](../contracts/cross-repo-change-bundle.md), EventEnvelope, RunSnapshot, Checkpoint, Handoff와 전이 불변식은 [이벤트와 상태 계약](../contracts/events-and-state.md)이 소유한다.

## 작업 상태

| 상태 | 의미 |
|---|---|
| draft | 목표가 처음 만들어짐 |
| clarifying | 필요한 질문을 확인 중 |
| planned | 단계와 배정 결과가 만들어짐 |
| approved | 계획이 실행 가능함 |
| running | 한 개 이상의 단계가 실행 중 |
| paused | 사용자가 일시 중단함 |
| validating | 검사 중 |
| reviewing | 독립 검토 중 |
| merging | 병렬 변경을 통합 중 |
| blocked | 사용자 결정이나 외부 상태가 필요함 |
| failed | 자동 복구 범위를 넘겨 실패함 |
| cancelled | 사용자가 취소함 |
| completed | 완료 조건과 증거가 충족됨 |

상태가 바뀔 때 시간, 이유, 관련 단계를 함께 기록한다.

## 저장 위치

### 저장 계층

| 위치 | 저장 내용 | 저장하지 않는 것 |
|---|---|---|
| 대상 Git repository | Project 선언, source, config, Rule·Check·Gate metadata·ChangeRecipe, Managed Registry root·fragment, shared suppression·baseline, Schema·Catalog, 검토된 Corpus fixture | local scan/validation projection, 개인 path, raw log |
| `%LOCALAPPDATA%\Star-Control\management\global\` | Project directory, ProjectCheckout relation, ProjectCatalogSnapshot, TaskSpec·ScopeRevision·ImpactAnalysis summary·ValidationPlan, cross-project relation·coordination, MultiProjectGoal·CrossRepoChangeBundle·ChangeBundleReleaseHandoff와 global lifecycle summary | project scan·edge/participant detail, source file byte, raw project root |
| `%LOCALAPPDATA%\Star-Control\management\projects\<project-id>\` | project별 revision·workspace·CodeIndexSnapshot·ManagedRegistrySnapshot partition, graph·Finding·Registry binding/consumer·Diagnostic query projection, ChangeSet·ImpactEdge·ChangePlan participant state, RecipeExecution·PatchSet·PatchApplication·recovery journal, ValidationRun·Result·Gate participant, M7 failure/dependency/supply-chain/update/Radar, M8 migration/checkpoint/validation·performance comparison·language equivalence와 9단계 ChangeBundleParticipant·Worktree·MergeQueue·Conflict·MergeResult·Remote snapshot/operation projection, local Baseline·Suppression·Disposition·operation·evidence index | 다른 project detail, source/manifest/lockfile/data/worktree byte·큰 diff·log·dump·trace·profile, tool별 별도 DB, raw project root |
| `%LOCALAPPDATA%\Star-Control\cache\project-index\<project-id>\` | adapter·input fingerprint별 다시 만들 수 있는 content-addressed index intermediate | current pointer, source 전체 복사본, local decision, backup 대상 자료 |
| `<project>\.ai-runs\star-control\` | hash가 있는 diff·patch·redacted log·trace·external report, ReproductionPack·dependency/release manifest, M8 migration manifest/receipt·performance sample/profile·equivalence report, Diagnostic manifest·EvidenceBundle·ReviewPack export | DB backend file, migration 대상 data/DB copy, raw secret·PII, 다른 project 절대 path, source Corpus 정본 |

Git source가 공유 정본이다. 관리 DB는 source-derived projection과 local-only 운영 상태를 함께 가지지만 source code의 유일한 정본이 아니다. `.ai-runs`는 큰 evidence byte를 소유하고 DB는 ArtifactRef만 저장한다.

Managed Registry에서도 Git root·fragment가 정본이고 DB snapshot은 rebuildable derived Index다. source와 다르면 DB를 stale로 표시하고 source를 DB 값으로 되쓰지 않는다.

### Controller 상태

배경 Controller가 다시 시작해도 필요한 내부 상태는 Windows 사용자 로컬 데이터 폴더에 저장한다.

    %LOCALAPPDATA%\Star-Control\

개념 layout은 다음과 같다. 실제 DB filename과 backend 확장자는 public contract가 아니다.

```text
%LOCALAPPDATA%\Star-Control\
  controller\             # instance·health·single-writer lease
  management\
    active-set.json        # global+project generation header·relative locator를 고정하는 hash manifest
    global\
      active\             # 현재 global opaque store generation
      generations\        # migration·rebuild 후보
      backups\            # verified backup
      recovery\           # 손상 원본의 보존 copy
    projects\
      <project-id>\
        active\           # 이 ProjectId의 현재 generation
        generations\
        backups\
        recovery\
    backup-sets\           # 함께 복구할 generation vector manifest
  root-bindings\          # current-user protected opaque checkout root binding
  cache\
    project-index\
      <project-id>\
        <adapter-id>\
          <cache-key>\     # snapshot·config·adapter fingerprint 기반 재생성 cache
  migration-workspaces\   # ProjectId·plan ID별 protected candidate/copy; evidence나 정본 아님
  logs\                   # redaction·retention 적용
```

global DB에는 Project directory·ProjectCheckout·ProjectCatalogSnapshot·cross-project coordination, TaskSpec·ScopeRevision·ImpactAnalysis summary, ValidationPlan과 multi-project Gate summary를, ProjectId별 DB에는 source-derived CodeIndexSnapshot partition, project별 ChangeSet·ImpactEdge·ChangePlan, ValidationRun·ValidationResult·DiagnosticEvaluation participant, event·projection, local decision과 application 상태를 둔다. EvidenceSubjectBinding·GateDecision은 global summary와 project detail을 content fingerprinted ref로 연결하며 다른 Project의 source 위치·Diagnostic detail을 복제하지 않는다. TaskSpec·ScopeRevision에는 사용자가 선언한 ProjectPathRef·stable selector를 보존할 수 있지만 observed source range·literal·private symbol detail은 project store의 fingerprinted ref로만 연결한다. 이 planning/validation document는 local operational state이며 source scan만으로 복구됐다고 주장하지 않는다. raw project root는 어느 DB에도 저장하지 않고 `root_binding_id`만 둔다. 실제 root locator는 별도 adapter가 Windows current-user protection으로 암호화한 opaque locator를 해석하며 plaintext는 process memory 밖으로 노출하지 않는다. root binding은 management backup·export에 포함하지 않는다.

0단계 현재 `Project` schema v1은 Project 하나에 `root_binding_id` 하나를 둔다. 1단계 구현은 [Project Catalog·Code Index 계약의 선행 gap](../contracts/project-catalog-and-code-index.md#0단계-선행조건과-호환성-gap)에 따라 binding을 `ProjectCheckout`으로 이동하는 schema migration을 먼저 거친다. migration 전 row를 복수 checkout으로 추정 복제하지 않으며, migration이 끝나기 전에는 단일 attached checkout만 current로 취급한다. 이 문서 반영은 schema·DB 구현 완료를 뜻하지 않는다.

cache는 store generation과 별도다. 삭제·miss·손상 시 같은 source와 fingerprint로 재생성해야 하며 `active-set.json`, backup-set, integrity 성공과 current 판정의 필수 자료가 아니다. cache key는 ProjectId·WorkspaceSnapshotId·partition·adapter fingerprint·index config fingerprint로 만들고 directory 이름에 project명·path·사용자명을 넣지 않는다. source 전체 byte, secret, 개인 절대 경로와 민감 literal은 cache에 저장하지 않는다.

v1 management DB byte 전체를 암호화하지 않는다. 관리 directory, DB auxiliary file, backup과 recovery copy에는 current user와 SYSTEM만 허용하는 Windows ACL을 적용하고 persistence 전 redaction을 강제한다. 이 경계는 다른 일반 사용자에 대한 보호이며 관리자 또는 이미 침해된 current-user process에 대한 비밀 저장소를 주장하지 않는다.

### 프로젝트 증거

프로젝트별 실행 증거는 대상 프로젝트에 둔다.

    <project>\.ai-runs\star-control\runs\<run-id>\

Star-Control 저장소 자체가 아니라 실제 작업 대상 프로젝트에 기록한다.

### 여러 프로젝트 작업

전체 목표의 연결 정보, ScopeRevision의 project ref, `MultiProjectGoal`, `CrossRepoChangeBundle`, `ChangeBundleReleaseHandoff`와 management `CoordinatedOperation`은 global store에 둔다. project 상세 ChangeSet·ImpactEdge·ChangeBundleParticipant·WorktreeRecord·MergeQueue/Conflict/Result·RemoteOperation과 participant receipt는 각 project store, 변경·검사·conflict·remote evidence byte는 각 프로젝트 `.ai-runs/`에 둔다. 모든 project-scoped DB record는 ProjectId partition을 가지며 서로의 root binding과 절대 위치를 복제하지 않는다. cross-project 관계는 ProjectId, stable exported entity/contract ID와 project-relative path만 사용한다.

global bundle state는 project document ref·fingerprint·local/remote summary만 가진다. project receipt가 없거나 fingerprint가 다르면 성공으로 투영하지 않는다. `CoordinatedOperation=completed`는 관리 store commit이 복구 가능하다는 뜻이며 여러 Git history·remote service effect가 원자적으로 완료됐다는 뜻이 아니다.

## 실행 증거 폴더 예시

    <run-id>\
      goal.json
      task-specs\
        <task-spec-id>.json
      plan.json
      capability-snapshot.json
      events.jsonl
      stages\
        <stage-id>\
          stage.json
          scope-revision.json
          impact-analysis.json
          change-plan.json
          route.json
          context-summary.json
          permission-plan.json
          validation-plan.json
          validation\
            preflight.json           # subject binding·Registry·plan coherence
            runs\                    # CheckPlan·attempt별 raw/normalized manifest
            result.json              # completeness·freshness·stability
            gate-decision.json       # RunSatisfaction·DiagnosticEvaluation·claims
          result.json
          checkpoint.json
      evidence\
        changes.json
        validations.json
        claims.json
        diagnostics.jsonl
        baseline-comparison.json
        evidence-bundle.json
        cost.json
        risks.json
        final-summary.md
      review\
        review-pack.json             # star.review-pack 구조화 정본
        review-pack.md
        rework-directive.json
      merge\
        merge-plan.json
        conflicts.json
        result.json

Goal Run 밖의 CLI-only scan·change evidence는 별도 scope를 사용한다.

```text
<project>\.ai-runs\star-control\management\
  scans\<scan-run-id>\              # catalog/index refs·source manifest·freshness·coverage·scan report
  plans\<task-spec-id>\<scope-revision-id>\ # Task·scope·ChangeSet·impact trace·affected selection
  recipes\<recipe-execution-id>\    # resolved input·selector·tool identity·preview/idempotence manifest
  patches\<patch-set-id>\           # forward/reverse patch·before/after manifest·WorktreeDecision
    applications\<patch-application-id>\ # per-operation receipt·apply/recovery report
  validations\<validation-result-id>\ # binding·attempt·log·Diagnostic·Gate·EvidenceBundle·ReviewPack
  failures\<failure-record-id>\       # occurrence·causality·regression refs
    reproduction\<reproduction-pack-id>\ # curated manifest와 redacted artifact refs
    recovery\<recovery-plan-id>\      # rollback·roll-forward·restore plan/attempt
  security\<supply-chain-snapshot-id>\ # workflow·release·external source evidence
  dependencies\<dependency-snapshot-id>\ # relation·state·freshness snapshot
    updates\<dependency-update-plan-id>\ # candidate·approval·PatchSet·before lockfile refs
  maintenance\<radar-snapshot-id>\    # input refs·evaluation time·deterministic priority
  migrations\<migration-plan-id>\     # plan·attempt·checkpoint·invariant·restore evidence
    attempts\<migration-attempt-id>\  # redacted receipt·report·target manifest refs
  performance\<comparison-id>\        # workload spec ref·raw cohort·profile/build report
  language-migrations\<plan-id>\      # behavior baseline·equivalence·cutover/rollback refs
  change-bundles\<bundle-id>\          # 이 Project participant·worktree·merge·remote evidence
```

이 폴더는 DB layout이 아니라 evidence export layout이다. 파일 이름은 export 구현에서 달라질 수 있지만 각 파일이 담는 의미는 [데이터 계약 지도](../contracts/README.md)의 Schema ID를 따른다. Controller가 event·projection을 commit한 뒤 export하며 export가 늦거나 손상되면 committed 계약과 ArtifactRef에서 다시 만든다.

## M3 evidence·baseline·suppression·Corpus 경계

3단계 [공통 검증·품질 Gate](../features/common-validation-gate.md)의 source와 runtime 자료를 다음처럼 분리한다. 이 layout은 목표 설계이며 M3 DB·Corpus가 현재 구현됐다는 뜻이 아니다.

| 자료 | 정본·저장 위치 | Writer | 불변식 |
|---|---|---|---|
| built-in Rule·Check·Gate metadata | Star-Control `catalog/validators/` | review된 source 변경 | stable ID·version·definition fingerprint |
| project validator metadata | 대상 repo `.star-control/` 선언 | review된 project source 변경 | trusted ToolDescriptor만 참조, raw shell 금지 |
| shared Baseline·Suppression | 대상 repo `.star-control/baselines/`, `suppressions.toml` | review된 source 변경 | 자동 active 생성 금지 |
| local Baseline·Suppression·Disposition | ProjectId별 management store | Controller application transaction | backup 없이는 rebuild 불가 |
| ValidationRun·Result·raw Diagnostic·DiagnosticEvaluation·GateDecision | project store와 global coordinator ref | Controller 단일 Writer | exact EvidenceSubjectBinding·immutable observation/result |
| raw log·external report·diff·trace | 대상 repo `.ai-runs` ArtifactRef | `star-evidence` atomic finalize | redaction·size·hash 검증 뒤 DB ref |
| EvidenceBundle·ReviewPack | committed contract ref + `.ai-runs` export | GateDecision 뒤 Controller packaging/export | `GateDecision -> EvidenceBundle -> ReviewPack` 단방향 hash, report가 raw 사실을 바꾸지 않음 |
| built-in 회귀 Corpus | Star-Control source `corpus/` | Rule 변경 작업 | positive·negative·edge·regression manifest |
| project-specific Corpus | 대상 repo의 review된 project fixture 경로 | project source 변경 | secret·개인 path 제거, source ownership 명시 |
| Corpus 실행 결과 | `.ai-runs` validation artifact | runner가 결과를 방출하고 `star-evidence`가 finalize | source Corpus를 수정하지 않음 |

Baseline candidate는 complete current ScanRun 또는 ValidationResult에서만 만들며 자동으로 active가 되지 않는다. active Baseline도 pass나 suppression이 아니며 current Diagnostic과 `new|existing_unchanged|worsened|improved|not_observed|incompatible|unbaselined`을 비교하는 입력이다.

Suppression은 이유, exact/bounded selector, Rule/fingerprint contract, scope, actor, 생성·만료와 subject/config constraint를 가진다. 만료·stale·revoked declaration을 삭제하지 않고 GateDecision의 DiagnosticEvaluation에 적용 실패 상태를 남긴다.

Corpus는 제품 runtime DB에 import해 truth로 삼지 않는다. runner는 case manifest와 expected fingerprint를 읽어 test 결과를 만들고, expected output 갱신은 일반 source review를 거친다. 현재 validator가 자기 Corpus 기대값을 자동 변경하는 path는 없다.

EvidenceSubjectBinding의 current probe가 실패하거나 source·plan·config·Catalog·Tool fingerprint가 달라지면 기존 result를 stale/unverified로 표시한다. 이전 evidence byte는 보존할 수 있지만 current Gate의 positive evidence ref로 승격하지 않는다.

## M4 Recipe·Patch application 상태 경계

4단계 [안전한 Patch·Refactor·codemod 엔진](../contracts/safe-patch-and-codemod.md)의 `ChangeRecipe`는 대상 Git repository의 review된 Catalog source가 정본이다. resolved input·selector·Tool identity·preview output을 담는 `RecipeExecution`, immutable `PatchSet`, 실제 effect와 per-path receipt를 담는 `PatchApplication`은 ProjectId별 local operational record다. source scan만으로 동일한 사용자 입력·승인·실행 순서를 복원할 수 없으므로 backup과 evidence export 대상이다.

큰 forward/reverse patch, preview diff, 외부 tool stdout·stderr·구조화 output과 before/after manifest는 `.ai-runs`의 hash·redaction 검증된 ArtifactRef로 둔다. DB에는 source byte나 전체 diff를 넣지 않고 Recipe·tool·input/output fingerprint, ArtifactRef, operation 상태와 recovery pointer만 둔다. Recipe input에 secret 원문이나 raw literal replacement payload를 evidence로 영구 저장하지 않는다.

`PatchSet`은 preview 이후 immutable이며 적용 여부를 field로 덮어쓰지 않는다. 재시도·부분 적용·outcome unknown·reverse 복구는 각각 새 `PatchApplication` 또는 attempt record와 append-only receipt로 남긴다. Controller crash 뒤에는 receipt와 actual filesystem probe를 비교해 상태를 복원하고, 확인하지 못한 operation을 성공 또는 미적용으로 추측하지 않는다. 폐기 가능한 격리 worktree의 locator는 durable evidence에 raw absolute path로 남기지 않고 protected root binding과 ownership token으로 참조한다.

## M5 Managed Registry 상태 경계

`ManagedRegistrySnapshot`, ManagedBindingObservation, ConsumerObservation과 RegistryConsistencyRecord는 project store의 source-derived generation이다. declaration fragment byte, generated output byte와 다른 Project private source detail은 DB에 넣지 않고 CanonicalSource·content fingerprint·ArtifactRef로 연결한다. candidate/local classification은 current CodeIndexSnapshot ref가 없으면 current가 아니다.

Registry 변경 intent·ChangePlan·RecipeExecution·PatchSet·approval·PatchApplication은 local operational/evidence state이고 Git manifest를 대신하지 않는다. 향후 terminal management view도 이 document를 만들 뿐 DB row나 source를 직접 편집하지 않는다. actual source write의 단일 Writer는 M3 pre Gate permit을 소비하는 M4 PatchApplication이다.

## M7 실패·보안·dependency·Radar 상태 경계

[7단계 정본](../contracts/failure-security-and-dependency-maintenance.md)의 구조화 문서는 ProjectId별 공통 management store와 evidence index를 재사용한다. scanner·debugger·package manager별 DB, 별도 Finding table과 별도 완료 상태 machine을 만들지 않는다.

| 자료 | 상태 위치 | 큰 byte 위치 | 불변식 |
|---|---|---|---|
| FailureRecord·RegressionRecord | project store document/event | 없음 또는 ArtifactRef | occurrence를 덮어쓰지 않고 family relation을 projection |
| ReproductionPack·RecoveryPlan | project store canonical document | curated manifest export와 redacted refs | 일반 log와 role 분리, 외부 조건 unverified 보존 |
| DependencySnapshot·SupplyChainSnapshot | project store source-derived generation | manifest/lockfile diff·scanner report ArtifactRef | exact M1/M6 subject, tool DB로 분리 금지 |
| ExternalDataSnapshot | project store immutable input record | approved payload ArtifactRef가 있으면 연결 | source/query/schema·coverage·freshness·valid_until |
| DependencyUpdatePlan | project store operational document | PatchSet·before manifest/lockfile ArtifactRef | 기본 awaiting approval, apply result를 field overwrite하지 않음 |
| MaintenanceRadarSnapshot | global summary ref + project detail | default-safe report export | 원본 Finding/Diagnostic을 복제하지 않는 rebuildable projection |

raw stdout·stderr·scanner report·dump·trace·manifest·lockfile byte는 DB blob에 넣지 않는다. ReproductionPack은 이 artifact 중 재현에 필요한 refs만 선별하고 `artifact_role=reproduction_required`를 붙인다. `quarantined|unknown`은 default ReviewPack/Radar/dashboard export에서 빠진다.

external data refresh 실패는 이전 snapshot을 덮어쓰지 않는다. 새 attempt와 error를 남기고 이전 snapshot은 `stale|unknown` 상태로 계속 참조한다. Radar current pointer는 input revision, `evaluation_time`과 `valid_until`을 모두 통과할 때만 publish한다.

dependency preview와 apply는 M4 상태 규칙을 그대로 사용한다. package manager가 만든 actual diff, immutable PatchSet, before manifest·lockfile, PermissionDecision과 GateDecision을 연결하며, dashboard row가 PatchApplication의 정본을 대신하지 않는다. rollback은 새 PatchApplication/RecoveryAttempt이고 이전 lockfile을 “current”로 직접 덮지 않는다.

## M8 migration·performance·language/platform 상태 경계

[8단계 정본](../contracts/migration-performance-and-platform.md)의 source 선언, local operational state와 evidence byte를 다음처럼 나눈다.

| 자료 | 정본·상태 위치 | 큰/민감 byte 위치 | 불변식 |
|---|---|---|---|
| ProjectMigrationManifest | 대상 Git `.star-control/migrations.toml` 목표 source | 없음 | DB가 chain·invariant·tool 선언을 역으로 쓰지 않음 |
| MigrationPlan·Checkpoint·Attempt·ValidationReport | ProjectId별 management repository document/event | receipt·report·manifest ArtifactRef | immutable attempt에서 상태 projection, 한 plan은 한 Project·target |
| backup/restore metadata | project store ref와 `RestoreVerificationRecord` | 실제 backup/data copy는 owner target 또는 protected workspace | backup 존재·integrity·restore rehearsal·behavior를 분리 |
| candidate migration target | `%LOCALAPPDATA%\Star-Control\migration-workspaces\<project-id>\<plan-id>\`의 opaque protected locator 또는 project tool 소유 target | target owner 위치 | evidence/backup/source 정본 아님, raw locator DB 저장 금지 |
| PerformanceWorkloadSpec | 대상 Git reviewed source·Catalog | 없음 | explicit activation, DB가 workload를 생성하지 않음 |
| PerformanceRun·Comparison | project store document/event | raw sample·profile·build report ArtifactRef | cohort exact binding, numeric unit·collector, raw attempt 보존 |
| LanguageMigrationPlan·EquivalenceReport | project store local operational/evidence document | differential result·generated report ArtifactRef | compile과 equivalence 분리, source patch는 M4만 |
| CrossProjectMigrationHandoff | global summary ref + project participant refs | optional redacted matrix ArtifactRef | read-only, 9단계 ChangeBundle/approval 아님 |

`migration-workspaces`는 migration 대상 data·DB byte를 일반 `.ai-runs` evidence로 복사하지 않기 위한 protected staging 경계다. path는 Controller process 안 opaque binding으로만 해석하고 DB·report에는 ProjectId, plan ID, target fingerprint와 retention state만 둔다. workspace가 민감 data를 포함하면 default report·backup export·content hash에 raw value를 넣지 않는다.

MigrationCheckpoint는 성공 증거가 아니다. durable ordered prefix, in-flight step, receipt와 actual target probe가 있어야 resume input이 되고, actual이 checkpoint before/expected-after 어느 쪽과도 일치하지 않으면 `outcome_unknown|diverged`다. `partially_succeeded`는 previous complete active target을 숨기거나 current success pointer를 교체하지 않는다.

Performance raw sample은 warmup/measured, baseline/candidate와 clean/incremental/cache hit/miss를 분리한다. 제외된 outlier도 보존하고 comparison projection이 raw value를 덮어쓰지 않는다. 수치가 없으면 DB에 0 또는 이전 값을 채우지 않는다.

Language migration의 behavior baseline, boundary/consumer state, platform matrix와 compatibility window는 report가 source contract를 대신하지 않는다. generated source·codegen input·consumer source writer는 M4 PatchApplication이고 equivalence evaluator는 derived evidence만 쓴다.

## 9단계 ChangeBundle·worktree·merge·remote 상태 경계

| 자료 | 상태 위치 | 큰 byte 위치 | 불변식 |
|---|---|---|---|
| MultiProjectGoal·CrossRepoChangeBundle | global store immutable document/event | 없음 | participant detail inline 금지, project ref·fingerprint만 |
| ChangeBundleParticipant | owning project store | project EvidenceBundle refs | 한 Project·repository·Checkout만 소유 |
| WorktreeRecord | owning project store + protected root binding | worktree source byte는 artifact 아님 | raw locator 저장 금지, owner token·Git registration 확인 |
| MergeQueueRecord·MergeConflictRecord·ProjectMergeResult | owning project store | conflict/diff/Git report ArtifactRef | repository별 직렬 queue, 양쪽 intent·contract 보존 |
| RemoteStateSnapshot·RemoteOperationRecord | owning project store + global summary ref | redacted provider response ArtifactRef | observation·approval·effect·after probe 분리 |
| ChangeBundleReleaseHandoff | global small document + project input refs | artifact byte는 project ArtifactRef | project별 commit·artifact·Gate exact binding |

worktree directory는 source 정본·backup·evidence가 아니다. WorktreeRecord를 삭제 상태로 덮어쓰지 않고 create/probe/retain/discard event를 보존한다. 실제 cleanup 뒤에도 PatchSet·PatchApplication·MergeResult·Gate·ArtifactRef가 남아야 한다.

local 상태 `validated_worktree|local_commit|local_branch_updated`와 remote 상태 `pushed|pr_open|checks_pending|merged`를 독립 field로 유지한다. local branch나 adapter call response를 이용해 remote 상태를 합성하지 않는다. remote after snapshot이 결과를 확인하지 못하면 operation과 bundle을 `outcome_unknown|held`로 둔다.

일부 participant만 완료된 경우 current 성공 pointer 하나로 축약하지 않는다. global bundle은 completed/pending/partial/rollback/unknown participant set, dependency-blocked downstream과 compatibility window를 보존한다. resume·roll-forward·compensation은 새 plan·approval·effect record이고 original success/failure를 삭제하지 않는다.

## 10단계 Release·Evaluation 상태 경계

[10단계 CI·Release·평가 정본](../contracts/ci-release-evaluation-and-product-completion.md)의 application 상태를 다음 위치에 저장한다. 이 표는 wire field나 상태 전이를 복제하지 않고 저장·보존 경계만 소유한다.

| 자료 | 상태 위치 | 큰 byte 위치 | 불변식 |
|---|---|---|---|
| ReleaseManifest v2·status projection | global management store + project source refs | final artifact는 immutable release store/ArtifactRef | source·config·Profile·artifact set digest exact binding |
| build/package/verification run | owning project store와 global release summary | `.ai-runs/star-control/<run-id>/release/` | local_quick·target·full·release와 phase 분리 |
| included-files·metadata·license·supply-chain report | ReleaseManifest document refs | release evidence ArtifactRef | final package byte에서 계산, source 추측 금지 |
| install/update/rollback/uninstall report | release summary + disposable target opaque binding | redacted installer log·state manifest ArtifactRef | 실제 user root를 fixture로 복제하지 않음 |
| ApprovalRequest·RemoteOperation·publish/deploy proof | global release summary + role별 target remote refs | redacted provider response ArtifactRef | ready·approved·published와 action별 before/after observation 분리 |
| EvaluationRun v2·case result | global evaluation document + project/case refs | `.ai-runs/star-control/<run-id>/evaluation/` | cli_only·codex_integrated cohort 분리 |
| eval corpus·baseline·policy·candidate definition | Git `evals/` source | runtime result 아님 | recommendation이 source를 자동 역쓰기하지 않음 |
| Catalog lifecycle·Radar ref | Catalog source와 management projection | migration/evaluation report ArtifactRef | active/deprecated/retired/rejected·tombstone 유지 |

artifact candidate는 새 staging generation에서 file hash·included-files manifest·artifact set digest를 finalize한 뒤에만 visible pointer를 바꾼다. 검증·promotion은 같은 byte를 사용하고 recompile·재압축·signing으로 byte가 달라지면 새 candidate generation이다. `ready` candidate와 rollback target이 참조하는 byte에는 retention hold를 건다.

ReleaseManifest status transition은 immutable revision과 event로 남긴다. approval stale 뒤 과거 `approved` revision을 삭제하지 않고 current projection만 `ready|blocked`로 이동한다. provider adapter receipt를 `published` projection으로 직접 쓰지 않으며 exact after RemoteStateSnapshot이 필요하다. publish·deploy·withdraw·rollback은 `remote_actions[]`의 target별 상태·승인·operation·before/after ref를 유지하고 한 target의 결과로 다른 target을 채우지 않는다.

EvaluationRun raw case·attempt·adjudication·metric을 summary가 덮어쓰지 않는다. actual defect, false positive, unresolved, flaky와 suppression은 별도 field이고 missing duration·usage·cost를 0으로 채우지 않는다. Catalog item이 retired돼도 historical CatalogSnapshot·EvaluationRun·Recipe recovery에 필요한 exact definition byte와 tombstone을 보존한다.

## 저장 원칙

- Controller 하나만 management repository와 evidence index를 쓴다.
- event, projection, idempotency와 store revision은 같은 logical store 안에서 한 repository transaction으로 commit한다.
- cross-store command는 global prepared operation, project participant receipt와 final completion으로 복구하며 하나의 DB transaction이라고 주장하지 않는다.
- scan 결과는 invisible generation에 batch write한 뒤 complete finalization에서만 visible pointer를 바꾼다.
- ProjectCatalogSnapshot과 CodeIndexSnapshot은 immutable content fingerprint를 가지며 current pointer와 freshness probe 결과를 snapshot 본문과 분리한다.
- ManagedRegistrySnapshot도 immutable generation이며 current pointer는 source manifest hash·namespace/tombstone·binding/consumer integrity를 통과한 경우에만 publish한다. invalid source에서 이전 generation을 current로 유지하지 않는다.
- incomplete·failed generation, stale cache와 parse no-result는 이전 complete current generation을 교체하지 않는다.
- invalidated ScopeRevision·ImpactAnalysis·ChangePlan·ValidationPlan을 덮어쓰거나 삭제하지 않고 새 revision이 supersedes로 연결한다.
- ValidationRun retry는 attempt별 immutable record이며 마지막 pass가 앞선 fail·flaky를 덮지 않는다.
- stale·partial·unverified ValidationResult와 GateDecision을 current success로 다시 쓰지 않고 새 subject binding에서 새 result·decision을 만든다.
- Baseline·Suppression·Disposition decision은 raw Finding·Diagnostic·ValidationResult와 별도 record로 유지한다.
- RecipeExecution·PatchSet·PatchApplication을 서로 덮어쓰지 않고 preview·proposal·actual effect와 recovery fact를 별도 immutable/append-only record로 유지한다.
- MultiProjectGoal·CrossRepoChangeBundle·participant·worktree·merge·remote operation을 서로 덮어쓰지 않고 global plan, project effect, local integration과 remote observation을 분리한다.
- 여러 repository의 Git/remote result를 management `CoordinatedOperation` 하나의 transaction success로 축약하지 않는다.
- ReleaseManifest candidate·ready·approved·publishing·published·unknown·rollback revision을 덮어쓰지 않고 artifact byte·approval·remote proof를 분리한다.
- EvaluationRun recommendation이 eval source·Catalog·Rule·Check·Profile·Recipe를 자동 수정하지 않는다.
- 중요한 store generation과 evidence manifest는 중간 상태가 보이지 않게 안전하게 교체한다.
- event export는 순서대로 추가하며 DB event revision과 hash를 기록한다.
- 잘못된 상태는 조용히 무시하지 않는다.
- 모르는 새 필드는 가능한 한 보존한다.
- DB와 evidence에는 secret, 사용자 이름, 개인 절대 경로와 민감 source literal을 저장하지 않는다.
- source file, 전체 diff, stdout·stderr와 trace를 DB blob에 넣지 않고 ArtifactRef로 연결한다.
- CLI, MCP와 향후 Codex entry adapter는 DB나 evidence file을 직접 열지 않고 같은 application service를 사용한다.

## 이어하기 기록

이어하기 기록에는 다음만 남긴다.

- 현재 목표와 단계
- 이미 끝난 결과
- 실패 원인과 시도한 방법
- 아직 남은 일
- 건드리면 안 되는 범위
- 관련 파일
- 다음 검사
- 다음 단계에 필요한 모델과 실행 방식
- 현재 작업 복사본과 병합 상태
- ChangeBundle이면 project별 local/remote state, dependency-ready/blocked step, compatibility window와 partial/rollback/hold 전략
- release이면 manifest revision·artifact set digest, layer/phase Gate, ready/approved/published·unknown·rollback 상태와 user-data hold
- evaluation이면 subject/context·baseline/candidate·recommendation·limitation과 Catalog lifecycle next action

전체 대화와 전체 로그를 다음 Codex에 그대로 전달하지 않는다.

## 보관 기간

보관 정책은 설정할 수 있다.

- 실행 중 기록: 삭제하지 않음
- 완료 요약과 핵심 증거: 장기 보관
- 큰 원문 로그: 설정된 기간 후 정리 가능
- 임시 파일: 안전한 종료 뒤 정리
- 실패 재현에 필요한 기록: 문제가 닫힐 때까지 보관

설계 기본값은 완료 run의 큰 원문·중간 artifact 90일, 해결된 실패 재현 자료 180일이다. 최종 요약·manifest, 실행 중 자료, 보존 hold와 미해결 실패 자료는 자동 정리하지 않는다. 공개 배포 전 실제 사용량을 측정해 기본값 변경이 필요한지 검토한다.

M7에서는 일반 log와 curated reproduction evidence의 retention을 분리한다. resolved failure의 ReproductionPack manifest·fingerprint·before/after·rollback ref는 180일 class를 사용할 수 있지만 pack이 참조한 raw dump·trace·stdout 전체가 같은 기간 자동 보존된다는 뜻은 아니다. unresolved regression·security Finding·dependency rollback evidence는 closure까지 `hold` 후보이며, raw secret·token·PII·unsafe memory dump는 hold를 이유로 보존을 연장하지 않고 quarantine/drop 정책을 우선한다.

M8 migration backup/candidate와 evidence retention도 분리한다. plan·attempt·checkpoint·validation·restore manifest는 evidence class일 수 있지만 실제 data/DB copy는 target owner policy를 따르며 일반 evidence retention이 보존을 강제하지 않는다. live partial·outcome unknown·rollback failure에 필요한 backup/candidate는 recovery hold 후보이고 자동 cleanup은 금지한다. performance raw profile은 report보다 짧은 별도 class를 사용할 수 있지만 comparison이 참조하는 minimum sample manifest·hash는 유지한다.

9단계 participant worktree·integration branch·conflict artifact·remote response도 상태와 byte retention을 분리한다. `partially_applied|rollback_required|held|outcome_unknown` participant의 owned worktree와 recovery evidence는 hold 후보이고 자동 cleanup하지 않는다. remote snapshot byte가 남아 있어도 `valid_until` 뒤 current remote truth가 아니며, worktree가 삭제돼도 merge/effect receipt와 ownership audit는 보존한다.

10단계 release artifact와 evaluation 자료도 상태와 byte retention을 분리한다. candidate 중 최소 current ready/published, 이전 검증된 rollback artifact와 open `publish_outcome_unknown|rollback_required`가 참조하는 byte는 hold 후보다. uninstall은 이 release/evidence hold나 user config·management store를 자동 삭제하지 않는다. EvaluationRun report보다 raw case artifact를 짧게 보관할 수 있지만 case/adjudication/metric hash·recommendation·decision·Catalog tombstone은 historical 판단을 설명할 수 있게 유지한다.

외부 advisory·license·version snapshot은 retention과 freshness가 별개다. byte가 남아 있어도 `valid_until`을 지나면 current evidence가 아니며, 삭제돼도 tombstone·source·digest·관찰 시각·deletion reason을 남겨 과거 Gate의 provenance를 설명한다.

관리 DB는 latest complete generation, incomplete staging, scan detail, resolved Finding, local decision과 migration backup을 서로 다른 retention class로 관리한다. 정확한 기본값과 merge 전략은 [설정과 Catalog 계약](../contracts/config-and-catalog.md)이 소유한다. source, shared declaration과 `.ai-runs` byte는 DB retention이 삭제하지 않는다.

정리는 startup 또는 수동 command에서만 실행하며 자체 예약 실행을 만들지 않는다. 먼저 candidate와 protected reason을 담은 retention plan을 만들고 같은 store revision·plan fingerprint와 필요한 permission에서만 적용한다.

## backup·손상·재구축

- migration·repair·active generation 교체 전 store별 consistent backup을 만든다. 여러 store가 관련되면 global과 affected project generation의 hash·revision을 한 backup-set manifest로 고정한다.
- backup byte·manifest 생성은 restore 가능성의 증명이 아니다. integrity 확인, 별도 generation restore, structural invariant와 required behavior Gate를 통과한 수준을 `created_unverified|integrity_verified|restore_rehearsed|restore_validated`로 구분한다.
- backend structural check, relation·partition, event/projection revision, fingerprint와 ArtifactRef hash를 계층적으로 검사한다.
- 손상이 의심되면 read-write open을 중단한다. Controller recovery component가 제시한 read-only mode, verified restore 또는 rebuild 중 활성화할 generation은 사용자가 선택하며 자동 전환하지 않는다.
- 손상 store를 덮어쓰지 않고 verified backup restore 또는 side-by-side rebuild를 수행한다.
- Git 선언·source와 같은 scan 입력이 있으면 current ProjectCatalogSnapshot, ProjectRevision, WorkspaceSnapshot, CodeIndexSnapshot, ManagedRegistrySnapshot, Symbol, Reference와 Finding projection을 재구축할 수 있다.
- `.ai-runs` canonical manifest가 남아 있으면 ValidationRun·ValidationResult·Diagnostic·GateDecision·EvidenceBundle과 ArtifactRef relation을 provenance·completeness와 함께 제한적으로 reindex할 수 있다. export만으로 current subject binding을 재검증했다고 주장하지 않는다.
- local-only Baseline·Suppression·Disposition, TaskSpec·ScopeRevision·ImpactAnalysis·ChangePlan·ValidationPlan, 진행 상태, 과거 actor·timestamp와 idempotency는 backup·export가 없으면 복구할 수 없다고 보고한다.
- 새 generation set 전체를 검증한 뒤에만 `active-set.json` pointer를 atomic replace하고 이전·손상 generation은 승인 전 삭제하지 않는다.

## 비밀정보

- 상태와 증거에 인증키 원문을 넣지 않는다.
- 환경 변수 값은 이름과 사용 여부만 기록한다.
- OS 사용자 이름, email과 개인 절대 경로를 저장하지 않는다.
- source literal은 message code와 redaction된 typed parameter로 바꾼다. secret·사용자 이름·raw 절대 경로·민감 literal 원문과 그 hash는 quarantined 상태에서도 저장하지 않는다.
- 외부로 내보낼 보고서는 한 번 더 가림 검사를 한다.
