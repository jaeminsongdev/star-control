# 8단계 Migration·성능·언어·플랫폼 계약

## 목적과 상태

이 문서는 Star-Control 8단계인 **데이터·설정·DB migration, 성능·build, 언어·플랫폼 migration**의 설계 정본이다. 현재 상태는 **설계 확정, 제품 구현 전**이다. 이 문서가 존재한다는 사실은 migration 실행기, benchmark runner, profiler, compiler, DB Schema·migration, CLI 또는 제품 code가 구현됐다는 뜻이 아니다.

8단계는 세 Profile을 하나의 공통 흐름에 연결한다.

```text
M1 current Project·Checkout·Index
  + M2 TaskSpec·ImpactAnalysis·ValidationPlan
  + M3 Validation Gate·EvidenceBundle
  + M4 Recipe·PatchSet·PatchApplication
  + M6 compatibility·consumer·environment evidence
  + M7 ReproductionPack·RecoveryPlan·rollback/restore evidence
  -> data_config_db_migration
  -> performance_build
  -> language_platform_migration
  -> project별 verified result 또는 9단계 ChangeBundle handoff
```

Star-Control은 대상 프로젝트의 DB engine, migration framework, benchmark engine, profiler, compiler, transpiler, package manager 또는 build cache를 다시 만들지 않는다. 프로젝트가 이미 가진 도구를 registered `ToolDescriptor`·`CheckDescriptor` adapter로 실행하고, 계획·승인·상태·증거·Gate만 소유한다.

## 범위와 제외 범위

### 포함

- 대상 프로젝트의 data·config·DB·state·file-format version 식별과 지원 범위
- 연속 migration chain, dry-run, consistent backup, restore rehearsal, migration rehearsal, execute, validate, resume와 rollback
- invariant와 exact before/after evidence, partial execution과 crash reconciliation
- 명시적으로 선언된 중요 성능·build workload의 baseline/candidate 비교
- 현재 동작 계약을 기준으로 한 언어·runtime·플랫폼 migration과 단계별 공존·cutover·rollback
- 한 Project 안의 source 변경을 M4 PatchSet으로 준비하고 M3 pre/post Gate로 검증하는 흐름
- 여러 Project migration을 9단계 `ChangeBundle`이 조정할 수 있게 만드는 read-only handoff

### 제외

- 승인 없는 live data·DB·config 변경과 destructive migration
- 범용 DB engine, compiler, profiler, benchmark harness, build analyzer 또는 자동 번역기 구현
- 숫자가 없는 성능 결과 생성, 서로 다른 비교 조건의 수치 합성 또는 측정값 추정
- compile 성공을 기능 동등성으로 간주하는 판정
- Star-Control이 실제 실행하지 않은 OS·architecture·runtime 결과를 통과로 표시하는 행위
- 9단계 전 cross-project source/data write, worktree 조정, merge, commit, push와 remote operation
- 자체 예약 migration·benchmark·refresh, background scheduler와 browser UI

## 선행 계약과 소유권

| 선행 단계 | 8단계가 재사용하는 것 | 8단계가 다시 정의하지 않는 것 |
|---|---|---|
| 0단계 | Controller 단일 Writer, internal store backup·generation·integrity·minimal migration | Star-Control 관리 DB relation·backend·open mode |
| 1단계 | current Project·Checkout·revision·workspace·toolchain·dependency 관찰 | source scanner·index DB |
| 2단계 | 사용자 TaskSpec, scope·impact·risk, selected Check와 fallback | AI 계획 생성, affected selector |
| 3단계 | exact subject binding, Diagnostic, GateDecision, EvidenceBundle·ReviewPack | 별도 migration/performance 완료 model |
| 4단계 | source·config·migration script·language source의 dry-run PatchSet, apply·recovery | live data operation을 diff만으로 표현하는 가짜 PatchSet |
| 6단계 | public contract baseline/current, consumer migration, config trace, environment constraint | contract identity·lifecycle |
| 7단계 | ReproductionPack, RegressionRecord, RecoveryPlan, previous manifest·lockfile, rollback·restore evidence | failure identity와 supply-chain freshness |

이 문서는 M8 Profile workflow와 M8 persisted document 의미를 소유한다. 일반 version 지원과 Star-Control 자체 store migration은 [Version과 Migration 계약](versioning-and-migrations.md), 공통 evidence wire와 Gate aggregation은 [검사·완료·증거](validation-and-evidence.md), 설정·Profile metadata는 [설정과 Catalog 계약](config-and-catalog.md)이 소유한다.

## 범용 Project migration과 Star-Control 자체 DB migration 분리

둘은 같은 안전 원칙을 사용하지만 같은 제품 경로나 정본이 아니다.

| 구분 | Star-Control 자체 관리 DB | 범용 대상 Project migration |
|---|---|---|
| 구현 단계 | 0단계 최소 lifecycle이 먼저 필요 | 8단계에서 범용 Profile·orchestrator 완성 |
| 정본 | `specs/compatibility.toml`, `management_store_version`, `star-state` private migration source | 대상 Git의 `.star-control/migrations.toml` 목표 선언과 registered project tool |
| 대상 | global/project management store generation | project data·config·DB·state·file format |
| 실행 owner | Controller가 주입한 `star-state` private adapter | `star-application`이 M8 workflow를 조정하고 registered adapter가 target effect 수행 |
| backend 지식 | `star-state` 내부에서만 허용 | core는 DB 종류·SQL·connection string을 모름 |
| source 변경 | 없음. logical store generation 변환 | migration script·config·source 변화는 M4 PatchSet |
| 자동 범위 | source-derived rebuildable projection은 0단계 정책이 허용할 수 있음 | destructive/live target은 별도 M8 permission·approval 적용 |
| 여러 Project | active-set의 internal store generation vector | 9단계 전 project별 plan·handoff만 허용 |

`ProjectMigrationManifest`로 Star-Control 자체 management DB를 제어하지 않는다. 반대로 범용 migration을 `star-state`의 private SQL migration이나 DB row write로 구현하지 않는다. 두 흐름이 공유할 수 있는 것은 stable ID, version chain, backup proof, invariant result, M3 evidence와 사용자 승인 계약뿐이다.

## M8 계약 Inventory

M8은 다음 top-level 목표 계약을 추가한다. nested step·metric·equivalence item은 별도 top-level Schema로 복제하지 않는다.

| 계약 | schema ID | 역할 |
|---|---|---|
| `ProjectMigrationManifest` | `star.project-migration-manifest` | project별 target·version source·chain·invariant Git 선언 |
| `MigrationPlan` | `star.migration-plan` | 한 Project·한 target의 immutable 실행 계획 |
| `MigrationCheckpoint` | `star.migration-checkpoint` | durable step boundary와 resume/reconcile 입력 |
| `MigrationAttempt` | `star.migration-attempt` | dry-run·backup·rehearsal·execute·resume·rollback 한 번의 사실 |
| `MigrationValidationReport` | `star.migration-validation-report` | before/after invariant·loss·reference·Gate 판정 |
| `RestoreVerificationRecord` | `star.restore-verification-record` | backup copy의 실제 restore·integrity·behavior 검증 |
| `PerformanceWorkloadSpec` | `star.performance-workload-spec` | 명시적 중요 경로와 측정 protocol Git 선언 |
| `PerformanceRun` | `star.performance-run` | 한 exact subject·mode·attempt의 raw 측정 |
| `PerformanceComparison` | `star.performance-comparison` | comparable baseline/candidate 통계·noise·trade-off 판정 |
| `LanguageMigrationPlan` | `star.language-migration-plan` | source/target stack, 공존, consumer 전환·cutover·rollback 계획 |
| `EquivalenceReport` | `star.equivalence-report` | compile과 기능 동등성을 분리한 dimension별 결과 |
| `CrossProjectMigrationHandoff` | `star.cross-project-migration-handoff` | 9단계 ChangeBundle을 위한 project별 read-only 입력 |

모든 document는 공통 envelope, `schema_version`, immutable revision과 canonical fingerprint를 가진다. `MigrationStep`, `MigrationInvariant`, `Measurement`, `ComparisonCohort`, `BehaviorContract`, `EquivalenceDimension`, `CoexistencePhase`와 `MigrationParticipant`는 owning Schema의 `$defs`에 한 번만 둔다.

## ProjectMigrationManifest

`ProjectMigrationManifest`의 목표 위치는 `<project>/.star-control/migrations.toml`이다. 이 파일은 migration을 자동 실행하는 script가 아니라 review 가능한 선언이다. 실제 command는 Catalog의 registered descriptor를 참조한다.

| 필드 | 의미 |
|---|---|
| `manifest_version` | manifest 자체 SemVer |
| `project_id` | 선언 owner Project |
| `target_specs` | stable target ID, kind, owner, locator class와 sensitivity |
| `version_sources` | current/target version을 읽는 registered read-only probe |
| `supported_ranges` | reader/writer/execute 지원 범위와 unsupported 이유 |
| `migration_chains` | 연속 `from_version -> to_version` step edge |
| `invariant_specs` | preserved·transition·loss-budget invariant |
| `backup_specs` | consistent snapshot 방식, manifest·integrity·retention 요구 |
| `rehearsal_specs` | 사본·disposable target, input sanitization과 검증 절차 |
| `activation_specs` | side-by-side pointer, atomic replace 또는 proven transaction 경계 |
| `rollback_specs` | 이전 generation/pointer, reverse step 또는 verified restore |
| `consumer_refs` | M6 consumer와 compatibility window |
| `tool_refs`, `check_refs` | registered adapter와 required M3 Check |
| `cross_project_relations` | provider/consumer·ordering hint. M8에서는 read-only |
| `content_fingerprint` | timestamp·raw path·secret을 제외한 canonical hash |

`target_kind` v1은 `data`, `config`, `database`, `state`, `file_format`이다. `ipc`, `plugin`, `language_runtime`, `platform`은 version vector와 호환성 관찰 대상으로 선언할 수 있지만 data/config/DB execute chain으로 섞지 않는다. 언어·플랫폼 source 전환은 뒤의 `LanguageMigrationPlan`이 소유한다.

manifest는 raw SQL, shell·PowerShell·`cmd` text, credential, connection string과 개인 절대 경로를 포함하지 않는다. `ToolDescriptor`와 typed argument binding으로 표현할 수 없는 step은 unresolved이며 자동 실행할 수 없다.

## Version 식별과 지원 범위

### MigrationVersionVector

한 숫자로 모든 변화를 대표하지 않는다. M8 `MigrationVersionVector`는 적용되는 axis만 정렬해 가진다.

| axis | 예 | version source |
|---|---|---|
| `project_data` | domain dataset·record format | registered metadata probe·manifest |
| `project_config` | config file schema | file header·Schema declaration |
| `project_database` | logical DB schema | project migration framework의 version table probe |
| `project_state` | persisted runtime state | state header·contract manifest |
| `file_format` | import/export format | magic/header·Schema ID |
| `public_contract` | API·CLI·Schema·error/config ID | M6 ContractSurfaceSnapshot |
| `toolchain_runtime` | language·runtime·SDK | M6 EnvironmentSnapshot |
| `ipc_protocol` | process protocol | native handshake/version manifest |
| `plugin_format` | plugin manifest/package | plugin manifest |

각 entry는 `axis_id`, `owner`, `observed_version`, `version_scheme`, `source_ref`, `source_fingerprint`, `coverage`, `observation_state`를 가진다. version을 찾지 못하면 `unknown`이고 `0`, `latest` 또는 current product version으로 채우지 않는다.

### 지원 판정

| 상태 | 의미 | 실행 가능성 |
|---|---|---|
| `current_supported` | current writer target과 같음 | migration 불필요 |
| `migratable` | current에서 target까지 유일한 검증 chain 존재 | preflight 진행 가능 |
| `read_only_supported` | 읽기·export만 가능 | live execute 금지 |
| `future_version` | 현재 reader보다 높음 | opaque inspection만 |
| `chain_gap` | 연속 step이 없음 | 차단 |
| `ambiguous_chain` | 같은 version에서 둘 이상의 선택이 자동 결정 불가 | 사용자/manifest 수정 필요 |
| `unknown_version` | version source가 없거나 partial | 차단 |
| `corrupt` | version·hash·structure 신뢰 불가 | 원본 격리, recovery만 |

branch 이름, directory mtime, DB 최신 row, tool의 “up to date” 문자열과 파일 존재만으로 version을 추측하지 않는다.

## 단계별 migration chain

`MigrationChain`은 한 target axis의 연속 edge다. `v1 -> v3` direct step을 선언해도 내부 검증 근거가 `v1 -> v2 -> v3`이면 plan에는 실제 edge 둘을 materialize한다.

`MigrationStep` 최소 필드는 다음과 같다.

| 필드 | 의미 |
|---|---|
| `step_id`, `step_version` | stable ID와 정의 version |
| `from_version`, `to_version` | 정확히 한 연속 edge |
| `preconditions` | expected target identity·version·hash·free space·exclusive access |
| `invocation_template_ref` | registered Tool/Task와 typed input Schema |
| `effect_class` | `read_only`, `copy_write`, `live_nondestructive`, `live_destructive` |
| `write_scope` | target-relative/opaque locator와 예상 output class |
| `idempotency_contract` | `replay_safe`, `detect_already_applied`, `not_replay_safe` |
| `checkpoint_policy` | durable boundary 전·후 probe와 receipt |
| `unknown_field_policy` | preserve/opaque/block 가운데 target contract가 허용한 값 |
| `invariant_refs` | step 전·후 required invariant |
| `expected_output` | target version, shape·count·digest constraint |
| `rollback_ref` | reverse step, prior pointer 또는 backup restore |
| `tool_ref`, `normalizer_ref` | descriptor version/hash와 output mapping |

chain은 version이 strictly 진행해야 하며 cycle, self-edge, 중복 edge와 target을 건너뛴 숨은 default를 거부한다. step definition, tool binding 또는 invariant가 바뀌면 plan fingerprint가 바뀌므로 기존 승인·dry-run·rehearsal을 재사용하지 않는다.

## MigrationPlan

`MigrationPlan`은 **한 Project·한 Checkout·한 target axis**만 실행 대상으로 가진다. 같은 프로젝트의 config와 DB를 함께 바꿔야 하면 별도 plan과 명시적 dependency edge를 만들며 하나의 성공 상태로 뭉치지 않는다.

| 필드 | 의미 |
|---|---|
| `migration_plan_id`, `revision` | immutable plan identity |
| `task_spec_ref`, `scope_revision_ref`, `impact_analysis_ref` | 사용자 요청·범위·영향 |
| `project_id`, `checkout_id` | 한 Project 실행 경계 |
| `source_subject_binding` | current revision·workspace·config·Catalog·Tool·environment |
| `manifest_ref`, `target_id` | 선언과 exact target |
| `observed_version_vector`, `target_version_vector` | before와 의도한 after |
| `support_decision` | 지원 범위·chain 판정 |
| `ordered_steps` | resolved step ID/version/hash와 순서 |
| `invariant_refs` | before·per-step·after·rollback 검사 |
| `strategy` | `side_by_side`, `atomic_replace`, `transactional_in_place` |
| `resource_estimate` | byte·time·temporary space. 모르면 unknown |
| `dry_run_plan`, `backup_plan`, `rehearsal_plan` | 각 phase input·output·stop condition |
| `activation_plan` | candidate 검증 뒤 visible 전환 방식 |
| `resume_plan` | checkpoint·reconcile·retry 한도 |
| `rollback_plan_ref` | M7 RecoveryPlan과 target-specific precondition |
| `validation_plan_refs` | M3 pre/rehearsal/post/rollback Gate |
| `permission_checkpoints` | backup, live effect, destructive effect, rollback |
| `source_patch_refs` | 필요한 M4 PatchSet·PatchApplication lineage |
| `consumer_compatibility_refs` | M6 report·window·migration guide |
| `cross_project_handoff_ref` | 여러 Project일 때 read-only handoff |
| `plan_fingerprint` | 모든 의미 input의 canonical hash |

`strategy=side_by_side`가 기본이다. `transactional_in_place`는 target adapter가 전체 chain과 activation을 한 transaction으로 보장하고 rollback 의미를 descriptor·fixture로 증명한 경우에만 선택할 수 있다. “DB가 transaction을 지원한다”는 일반 설명만으로 충분하지 않다.

## 공통 lifecycle

### phase 순서

```text
discover
  -> plan
  -> dry_run
  -> backup_create
  -> backup_verify
  -> restore_rehearsal
  -> migration_rehearsal
  -> pre_execute_gate
  -> execute_or_resume
  -> post_execute_validate
  -> activate
  -> startup_or_consumer_validate
  -> succeeded

failure/outcome_unknown
  -> reconcile
  -> resume | rollback | human_recovery
  -> post_rollback_validate
```

phase를 생략하려면 `not_required` 판정의 owning invariant·strategy·evidence가 있어야 한다. 단순 미실행은 생략이 아니다. live destructive migration에서는 dry-run, backup integrity, restore rehearsal, migration rehearsal와 pre-execute Gate를 `not_required`로 만들 수 없다.

approval과 Gate는 순환 참조를 만들지 않는다. effect 전 `PermissionDecision`은 exact plan, ValidationPlan·GatePolicy fingerprint와 expiry에 결합하고, `migration_pre_execute`가 그 decision과 모든 precondition을 평가한다. Gate가 허용된 뒤 Controller가 PermissionDecision과 실제 GateDecision을 함께 결합한 single-use in-memory permit을 발급한다. approval이 미래 GateDecision ID를 미리 주장하거나 persisted approval·Gate ID만으로 target port를 여는 경로는 없다.

### immutable attempt

각 phase 실행은 새 `MigrationAttempt`다.

| 필드 | 의미 |
|---|---|
| `attempt_id`, `attempt_no` | phase별 immutable 시도 |
| `plan_ref`, `plan_fingerprint` | 승인·실행한 exact plan |
| `phase` | `dry_run\|backup\|restore_rehearsal\|migration_rehearsal\|execute\|resume\|validate\|activate\|rollback\|post_rollback_validate` |
| `step_ref` | 해당하면 exact chain step |
| `subject_binding_before` | target·source·environment exact before |
| `checkpoint_before_ref` | resume/reconcile 기준 |
| `permission_decision_ref` | effect가 있으면 exact approval |
| `invocation`, `tool_observation` | typed command와 actual executable identity |
| `receipt_refs` | target adapter가 반환한 operation receipt |
| `subject_binding_after` | 재관찰한 actual state |
| `checkpoint_after_ref` | durable boundary가 확인된 경우만 |
| `outcome` | `completed\|failed\|cancelled\|timeout\|outcome_unknown` |
| `diagnostic_refs`, `artifact_refs` | redacted result와 큰 자료 |
| `attempt_fingerprint` | 의미 input/output hash |

adapter exit code 0은 `completed` 후보일 뿐 migration success가 아니다. `subject_binding_after`, invariant와 M3 Gate가 없으면 final success로 승격하지 않는다.

## Dry-run

dry-run은 live target을 쓰지 않고 다음을 생성한다.

- observed/target `MigrationVersionVector`와 selected chain
- step별 expected read/write/delete/rename·record class와 destructive marker
- unknown field·enum·extension 발견 목록과 보존 가능성
- expected row/item/byte delta와 loss budget. 계산할 수 없으면 `unknown`
- required lock/quiesce, temporary disk, timeout와 external dependency
- backup·restore·rehearsal target과 activation 방식
- invariant evaluation plan과 M3 selected Check
- consumer·public contract·config·dependency 영향
- rollback 가능 범위와 irreversible boundary
- permission checkpoint와 exact plan fingerprint

dry-run이 실제 target byte를 만들 필요가 있으면 `copy_write`인 격리 사본만 사용하고 이를 live dry-run으로 부르지 않는다. tool의 `--dry-run` 문자열만 믿지 않고 write scope probe로 live target 무변경을 검증한다.

## Backup과 restore 검증

backup 존재와 restore 가능성은 별도 상태다.

| 상태 | 증명하는 것 | 증명하지 않는 것 |
|---|---|---|
| `not_created` | 없음 | 복구 가능성 |
| `created_unverified` | byte와 manifest가 생성됨 | integrity·restore 가능성 |
| `integrity_verified` | size/hash/header·snapshot consistency 통과 | 새 환경 restore·동작 |
| `restore_rehearsed` | 사본 환경에 실제 restore하고 structural invariant 통과 | live rollback 완료 |
| `restore_validated` | restore 뒤 required behavior/consumer Check와 M3 Gate 통과 | 미래 시점의 무조건적 복구 |

`RestoreVerificationRecord`는 backup ID·byte hash·version vector, source stop point, restore target class, adapter identity, restore attempt, integrity/invariant result, required behavior Check, environment fingerprint, GateDecision과 limitation을 가진다. backup file 존재, copy exit 0 또는 checksum 일치만으로 `restore_rehearsed|restore_validated`를 만들지 않는다.

consistent backup을 만들 수 없으면 live execute를 차단한다. 여러 파일·store·service가 하나의 logical target이면 같은 quiesced point의 set manifest가 필요하며 서로 다른 시점의 copy를 하나의 backup이라고 부르지 않는다.

backup·candidate·손상 원본은 retention plan과 별도 permission 전 삭제하지 않는다. 이 문서 설계는 어떤 backup도 실제로 만들거나 삭제하지 않는다.

## Rehearsal과 execute

### rehearsal

migration rehearsal은 production/live target과 분리된 copy 또는 disposable target에서 exact chain을 끝까지 실행한다.

- source version·input manifest는 live preflight와 compatible해야 한다.
- sensitive data copy는 project policy가 허용한 sanitized fixture 또는 protected local copy만 사용한다.
- tool/config/Catalog/environment fingerprint는 execute 계획과 같거나 descriptor가 선언한 compatible class여야 한다.
- step receipt, checkpoint, before/after invariant와 post-migration consumer Check를 모두 만든다.
- rehearsal 뒤 candidate를 live target으로 몰래 재사용하지 않는다. activation plan이 명시적으로 immutable candidate 승격을 지원할 때만 같은 hash를 참조할 수 있다.
- rehearsal failure·partial·flaky는 execute approval 입력에서 숨기지 않는다.

### execute와 activation

1. Writer/external target lock을 얻고 새 mutation을 quiesce한다.
2. plan의 source·target·version·config·Catalog·Tool·environment fingerprint를 다시 확인한다.
3. backup·restore·rehearsal 상태와 approval scope를 검증한다.
4. `side_by_side`이면 candidate target에 step을 적용하고 active target은 계속 서비스한다.
5. 각 durable step 뒤 receipt·after probe·checkpoint를 commit한다.
6. 전체 chain과 invariant가 통과한 candidate만 atomic pointer/rename으로 활성화한다.
7. `transactional_in_place`이면 adapter가 transaction commit 전 동일한 invariant를 실행하고 commit receipt를 반환한다.
8. activation 뒤 startup/consumer Check와 M3 post Gate를 새 subject에서 수행한다.
9. post Gate 실패 시 rollback plan과 irreversible boundary를 평가하고 자동 성공으로 남기지 않는다.

atomic replace가 불가능한 target은 partial visibility와 compensation 의미를 manifest에 선언해야 한다. 이를 선언·검증할 수 없으면 live execute를 지원하지 않는다.

## Invariant와 before/after evidence

`MigrationInvariant`는 자연어 완료 문장이 아니라 evaluator와 expected result를 가진다.

| kind | 예 | 판정 |
|---|---|---|
| `preserved` | stable ID uniqueness, reference integrity, config permission, unknown extension byte | before와 after가 같은 계약을 만족 |
| `transformed` | old field가 new field로 lossless mapping, target version 도달 | declared mapping과 golden/actual evidence |
| `count_balance` | source count = migrated + explicitly rejected/quarantined | 누락·중복 0 또는 승인된 loss budget |
| `semantic` | consumer-visible behavior·error·ordering | contract/differential test 또는 HUMAN_REVIEW |
| `security` | secret·ACL·permission·encryption boundary | registered security Check |
| `operational` | startup, read/write, resume, rollback | exact environment run |
| `performance_required` | migration downtime·critical workload budget | numeric PerformanceComparison |

각 결과는 invariant ID/version, subject before/after, evaluator Tool/Rule version, input coverage, expected/observed, completeness, artifact refs와 Diagnostic을 가진다. `unknown`, `partial`, `not_run`, stale 또는 다른 subject의 결과는 pass가 아니다.

before와 after는 같은 logical data scope·selection·redaction contract를 사용해야 한다. scope가 달라지면 count·hash가 같아도 comparable하지 않다. 전체 data hash를 저장할 수 없으면 privacy-safe partition count·Merkle/content fingerprint와 coverage limitation을 사용하며 secret·민감 raw value의 hash를 새로 저장하지 않는다.

## Partial migration, 재시작과 resume

### MigrationCheckpoint

checkpoint는 성공 선언이 아니라 resume를 위한 durable 사실이다.

| 필드 | 의미 |
|---|---|
| `checkpoint_id` | immutable ID |
| `plan_ref`, `chain_fingerprint` | 실행 계약 |
| `target_ref`, `target_version_observed` | opaque target과 실제 version |
| `completed_step_refs` | receipt·after probe가 확인된 ordered prefix |
| `in_flight_step_ref` | crash 시 시작 여부가 불명확할 수 있는 step |
| `receipt_set_fingerprint` | durable receipt 집합 |
| `active_candidate_state` | active/candidate pointer와 visibility |
| `invariant_summary` | 마지막 complete invariant result refs |
| `resume_preconditions` | expected version·hash·lock·tool·environment |
| `checkpoint_fingerprint` | canonical hash |

resume 전 actual target을 다시 관찰한다.

| actual 상태 | 행동 |
|---|---|
| checkpoint before와 일치, step 미시작 증명 | `replay_safe` step만 재시도 |
| expected after와 일치, receipt/probe 검증 | step 완료를 새 reconciliation attempt로 기록하고 다음 step |
| receipt는 있으나 after를 확인할 수 없음 | `outcome_unknown`, 자동 재실행 금지 |
| before·after 어느 쪽과도 불일치 | `diverged`, rollback/human recovery |
| plan·tool·environment fingerprint 변화 | 기존 approval·resume 무효, 재계획 |

`not_replay_safe` step의 시작 여부를 확정할 수 없으면 resume하지 않는다. 사용자에게 같은 command를 수동 재실행하라고 단순 권고하지 않고 recovery 선택과 데이터 손실 위험을 제시한다.

### 결과 상태

projection 상태는 immutable attempt·checkpoint·Gate에서 계산한다.

| 상태 | 정확한 의미 |
|---|---|
| `not_started` | effect attempt 없음 |
| `awaiting_approval` | current preconditions은 충족하지만 exact permission 필요. `pending_action=backup\|execute\|resume\|activate\|rollback\|restore`로 구분 |
| `running` | effect가 시작되고 outcome을 추적 중 |
| `paused_resumable` | ordered prefix가 verified되고 다음 step precondition이 유지됨 |
| `outcome_unknown` | effect 시작·commit 여부를 확정할 수 없음 |
| `succeeded` | target version 도달, 모든 required invariant·consumer Check와 post Gate current complete |
| `partially_succeeded` | 하나 이상 durable step은 완료했지만 target version/Gate 미도달. 전체 성공 아님 |
| `failed` | outcome은 알려졌으나 required step·invariant·Gate 실패 |
| `rollback_required` | current active/candidate 상태가 accepted target이 아니고 recovery 필요 |
| `rolling_back` | explicit rollback attempt 진행 중 |
| `rolled_back` | before-compatible state 복귀와 post-rollback invariant·Gate 통과 |
| `rollback_failed` | rollback attempt 또는 검증 실패 |
| `abandoned` | 사용자가 중단했으며 active/candidate·backup 보존 상태가 명시됨 |

`partially_succeeded`는 경고가 붙은 success가 아니다. live target에 partial visibility가 있으면 기본 Gate effect는 `BLOCK`이고, isolated candidate만 partial이면 live active target 유지 여부를 함께 보고한다.

## Rollback, roll-forward와 restore

- **rollback**: 이번 change의 operation을 되돌려 before-compatible active state로 복귀한다.
- **roll-forward**: 실패 원인을 수정하는 새 migration step/plan으로 target version을 향해 진행한다.
- **restore**: backup/snapshot byte를 새 target에 복원한다.

세 행동은 별도 `MigrationAttempt`와 M7 `RecoveryPlan`을 가진다. reverse script 존재만으로 rollback 가능하다고 하지 않는다. rollback 뒤 version, invariant, consumer behavior와 active pointer를 검증해야 `rolled_back`이다.

새 format으로 이미 external write가 발생했거나 downstream consumer가 target version을 사용했다면 binary/source rollback만 수행할 수 없다. data compatibility, reverse migration 또는 bounded dual-reader window가 없으면 `rollback_unavailable`로 표시하고 roll-forward/human recovery를 요구한다.

## Destructive migration 승인

다음 중 하나면 `live_destructive`다.

- record/field/file/table 삭제 또는 의미 손실
- unknown field·extension drop
- 되돌릴 수 없는 encoding·encryption·identity 변경
- old reader가 읽지 못하는 write 활성화
- backup/restore가 검증되지 않은 in-place write
- consumer compatibility window 전 old surface 제거
- scope 밖 target 또는 multiple Project write

승인 요청은 plan fingerprint, target ID·version, destructive step, 예상 loss set/budget, backup·restore·rehearsal 상태, irreversible boundary, downtime, affected consumer, rollback/roll-forward 선택과 expiry를 포함한다. `personal_auto`, project config, Profile 또는 tool manifest가 이를 `auto`로 낮출 수 없다.

dry-run 뒤 destructive scope, target content, chain, tool, config, environment 또는 consumer set이 바뀌면 승인은 무효다. 승인은 migration을 성공으로 판정하거나 M3 post Gate를 생략할 권한이 아니다.

## Unknown field와 version 축 분리

### unknown 보존

- current Schema에 허용되지 않은 field는 producer 오류다.
- 명시된 extension namespace는 byte·key·order contract가 요구하는 방식으로 round-trip한다.
- 미래 version object는 opaque copy로 보존하고 current type으로 다시 쓰지 않는다.
- enum unknown을 `other`, 빈 값 또는 기본값으로 바꾸지 않는다.
- tool이 unknown field 보존을 증명하지 못하면 automatic migration을 차단한다.
- config unknown key는 오타·미래 version이 해소되기 전 삭제·rename하지 않는다.
- 보존할 수 없는 unknown을 버리는 것은 destructive step이며 exact 승인과 loss evidence가 필요하다.

### Star-Control version 축

Star-Control 자체 update에서도 다음을 분리한다.

| 축 | 정본 | 자동 결합 금지 |
|---|---|---|
| product | release SemVer | 제품 update가 모든 persisted contract를 올린다고 가정하지 않음 |
| config | `star.config` schema version | DB/store version과 독립 |
| persisted state contract | document별 `schema_version` | management backend schema와 독립 |
| management DB | `management_store_version` | config·IPC·Plugin과 독립 |
| IPC | `major.minor` negotiation | DB migration 성공으로 protocol 호환을 추정하지 않음 |
| MCP fixed surface | tool별 schema·`mcp_contract_version` | live Tool Registry item version과 독립 |
| Codex Plugin | plugin manifest/product version | Controller DB와 독립 배포 가능 |
| Catalog | descriptor `format_version`·`item_version` | config migration으로 Catalog를 다시 쓰지 않음 |

한 release가 여러 축을 바꾸면 release plan이 각 migration/compatibility step과 순서를 명시한다. 한 축 실패를 다른 축의 최신 version 숫자로 숨기지 않는다.

## 여러 Project와 9단계 ChangeBundle 인계

M8은 여러 Project의 migration 필요를 **read-only로 분석**할 수 있지만 실행 단위는 project별 `MigrationPlan`, M4 `PatchSet`, M3 `GateDecision`으로 유지한다.

`CrossProjectMigrationHandoff` 최소 필드는 다음과 같다.

| 필드 | 의미 |
|---|---|
| `handoff_id`, `task_spec_ref` | 사용자 목표와 handoff identity |
| `participant_projects` | ProjectId, role `provider\|consumer\|data_owner\|tooling`, exact revision |
| `participant_plan_refs` | project별 MigrationPlan/LanguageMigrationPlan과 상태 |
| `dependency_edges` | provider-before-consumer, schema-before-codegen, writer-after-reader 등 |
| `contract_windows` | M6 compatibility/deprecation window와 최소 version |
| `patch_set_refs`, `gate_refs` | project별 source proposal·current evidence |
| `backup_restore_refs`, `rollback_refs` | participant별 recovery readiness |
| `cross_project_invariants` | 9단계가 ChangeBundle에서 materialize할 invariant 후보 |
| `blockers`, `unknowns` | stale·partial·unverified participant와 질문 |
| `handoff_fingerprint` | 정렬된 participant·edge·ref의 canonical hash |

handoff에는 approval token, raw root path, credential, merge/commit/push instruction과 실행 가능한 cross-repo script를 넣지 않는다. 9단계는 이 handoff에서 새 `ChangeBundle` revision을 만들고 project별 precondition·apply order·compensation·combined Gate를 다시 확정해야 한다. M8 handoff 자체는 실행 권한이 아니다.

## `data_config_db_migration` Profile

이 Profile은 앞 절의 manifest·plan·attempt·checkpoint·validation·restore 계약을 사용한다. source migration script, config template 또는 Schema change가 필요하면 먼저 M4 PatchSet을 준비하고 승인·post Gate를 통과한다. 그 뒤 target data effect는 별도 migration attempt로 수행한다.

기본 stop state는 `awaiting_approval`과 `pending_action=execute`다. `awaiting_execute_approval` 같은 별도 비표준 상태를 만들지 않는다. dry-run·backup·rehearsal evidence만으로 live execute를 자동 시작하지 않는다. [설정과 Catalog 계약](config-and-catalog.md)의 user-level `personal_auto`가 lossless·replay-safe single-Project exact scope를 명시적으로 허용한 경우에만 별도 PermissionDecision으로 진행할 수 있고, destructive·unknown-loss·cross-project effect에는 적용할 수 없다. source-derived copy만 만드는 read-only/isolated rehearsal과 live target effect를 같은 permission으로 합치지 않는다.

## 성능·build 측정 계약

### 활성화 경계

`performance_build`는 다음 중 하나가 명시됐을 때만 활성화한다.

- 사용자가 이번 목표에서 중요 경로·budget·비교를 선언
- project Git의 reviewed `PerformanceWorkloadSpec`을 TaskSpec이 참조
- M2 ImpactAnalysis가 declared performance risk path를 match
- migration/language plan이 required downtime·throughput·memory·artifact-size equivalence를 선언

모든 변경에 benchmark를 강제하지 않는다. workload 선언이 없으면 `not_applicable` 또는 `not_declared`이며 임의 command나 수치를 만들지 않는다.

### PerformanceWorkloadSpec

목표 Git 위치는 `<project>/.star-control/performance.toml` 또는 project가 명시한 reviewed Catalog source다.

| 필드 | 의미 |
|---|---|
| `workload_id`, `item_version` | stable workload identity |
| `purpose`, `critical_path` | 사용자가 중요하다고 선언한 이유 |
| `comparison_intent` | `source_change\|toolchain_change\|config_change\|migration_before_after\|platform_change\|repeatability` |
| `allowed_delta_axes` | baseline/candidate 사이 의도적으로 다른 exact field와 reason; 나머지는 동일해야 함 |
| `invocation_ref` | registered Task/Tool과 structured args |
| `input_manifest` | dataset/fixture/generator version·hash·seed |
| `subject_policy` | cohort 내부 exact revision, 허용 candidate delta |
| `environment_constraints` | OS·arch·CPU class·memory·power·runtime·toolchain·nonsecret env |
| `build_mode` | `runtime\|clean_build\|incremental_build\|cache_hit\|cache_miss\|artifact_only` |
| `cache_protocol` | cache identity·prepare/clear/warm rule와 evidence |
| `warmup_runs` | 결과 통계에서 제외하는 사전 실행 수 |
| `measurement_runs` | 최소 3인 measured attempt 수 |
| `metric_specs` | time·memory·artifact size·throughput 등 unit·collector |
| `aggregation` | predeclared median/percentile/mean 등 |
| `noise_policy` | variance/MAD/CV threshold와 추가 실행 상한 |
| `outlier_policy` | predeclared detector·제외 가능 조건·양쪽 결과 보고 |
| `budget_or_threshold` | 사용자가 선언한 absolute/relative 기준. 없을 수 있음 |
| `correctness_checks` | 결과가 같은 기능을 수행했음을 확인할 M3 Check |
| `profiler_refs`, `build_analyzer_refs` | optional external adapter |
| `spec_fingerprint` | canonical comparison protocol hash |

제품 기본값은 `warmup_runs=1`, `measurement_runs=5`, minimum measured runs 3이다. project는 더 많이 요구할 수 있고 runtime budget은 상한으로 제한할 수 있다. 보편적인 “5% 회귀” threshold는 제공하지 않는다. workload별 noise와 사용자 budget이 없으면 수치는 보여도 pass/regression 경계를 발명하지 않는다.

### comparability

baseline과 candidate는 다음 axis를 확인한다.

| axis | 요구 |
|---|---|
| workload | 같은 workload ID/version/spec fingerprint |
| input | 같은 input manifest·seed·selection |
| tool | workload driver·collector version/hash는 동일. `toolchain_change`면 대상 toolchain 차이만 `allowed_delta_axes`에 고정 |
| environment | 같은 exact fingerprint. `platform_change` 자체가 intent이면 차이를 exact binding하고 같은 환경 비교 주장은 금지 |
| build/cache mode | clean·incremental·hit·miss를 서로 섞지 않음 |
| config | 같은 EffectiveConfig fingerprint. `config_change`의 exact field만 `allowed_delta_axes`로 허용 |
| revision | 각 cohort 안 하나의 exact ProjectRevision·WorkspaceSnapshot. source/migration delta가 intent가 아니면 양쪽도 동일 |
| intended delta | source/migration 비교면 exact ChangeSet/PatchSet, 그 밖에는 toolchain/config/platform exact field만 허용 |

code change 비교에서 baseline과 candidate revision은 서로 다를 수 있지만 각 revision은 immutable하고 exact ChangeSet/PatchSet이 유일한 의도 delta여야 한다. toolchain/config/cache repeatability 비교라면 revision은 양쪽 exact 동일해야 한다. 여러 revision의 run을 한 cohort로 합치거나 branch 이름만 같다는 이유로 비교하지 않는다.

둘 이상의 axis를 동시에 바꾸면 end-to-end before/after 관찰은 만들 수 있어도 한 axis의 causal improvement로 판정하지 않는다. 다변수 비교가 필요하면 사용자가 factorial/cohort plan과 interaction 해석을 별도 workload로 선언해야 하며, 선언이 없으면 `HUMAN_REVIEW|not_comparable`이다. `declared_compatible` class는 raw trend를 나란히 표시할 수 있을 뿐 exact-environment Gate pass를 만들지 않는다.

### PerformanceRun

각 warmup·measured attempt를 별도 `PerformanceRun`으로 남긴다.

| 필드 | 의미 |
|---|---|
| `performance_run_id`, `cohort` | `baseline\|candidate`, attempt identity |
| `subject_binding` | exact revision·workspace·config·Catalog·Tool·environment |
| `workload_spec_ref` | 실행 protocol |
| `attempt_kind`, `attempt_no` | `warmup\|measured`와 순서 |
| `cache_state` | declared mode와 actual cache identity/probe |
| `measurements` | numeric value, unit, precision, collector ref |
| `invocation`, `started_at`, `finished_at` | registered execution 사실 |
| `outcome`, `completeness` | pass/fail/not_run/error와 complete/partial/unverified |
| `artifact_refs` | redacted raw samples·profile·build report |
| `diagnostic_refs` | noise·throttle·collector·correctness 문제 |

측정값을 얻지 못하면 metric field를 생략하고 `measurement_unavailable` 이유를 남긴다. 0, 이전 값, timeout 또는 tool exit code로 대체하지 않는다. unit 없는 숫자와 collector identity가 없는 memory/time 값은 invalid다.

### warmup, 반복, noise와 outlier

1. warmup은 measured result와 분리해 모두 보존한다.
2. measured attempt는 predeclared 수를 실행하고 실패 attempt를 삭제하지 않는다.
3. thermal throttling, background load, power mode, cache protocol 위반과 clock/collector 오류를 noise Diagnostic으로 기록한다.
4. outlier detector는 첫 measured run 전에 spec에 고정한다.
5. 제외된 sample도 raw set에 남기고 포함/제외 통계를 모두 계산한다.
6. outlier 제거 뒤 minimum run 수보다 작아지면 비교는 `inconclusive`다.
7. noise threshold 초과 시 bounded additional runs만 제안하며 자동으로 무한 반복하지 않는다.
8. baseline과 candidate에 다른 outlier rule을 적용하지 않는다.

### metric과 build mode

지원 metric family는 adapter가 실제 값을 제공한 경우에만 사용한다.

- wall/CPU duration과 latency percentile
- peak working set·private bytes·allocation 같은 명시된 memory metric
- artifact file/set size와 manifest hash
- throughput·operation count처럼 workload가 정의한 rate
- clean build
- incremental no-change 또는 declared source delta build
- cache hit와 cache miss

clean·incremental·cache hit·cache miss 결과는 별도 comparison item이다. 한 mode의 개선으로 다른 mode의 악화를 상쇄한 단일 점수를 기본 생성하지 않는다. artifact size도 exact output manifest가 같거나 declared compatible일 때만 비교한다.

### PerformanceComparison

| 필드 | 의미 |
|---|---|
| `comparison_id` | immutable result |
| `workload_spec_ref` | fixed protocol |
| `baseline_subject`, `candidate_subject` | exact cohort binding |
| `baseline_run_refs`, `candidate_run_refs` | 모든 warmup/measured attempt |
| `comparability` | axis별 `equal\|declared_compatible\|different\|unverified` |
| `raw_statistics`, `filtered_statistics` | sample count·aggregation·dispersion |
| `outlier_records`, `noise_assessment` | predeclared rule와 limitation |
| `metric_decisions` | `improved\|unchanged\|regressed\|budget_pass\|budget_fail\|inconclusive\|unmeasured` |
| `correctness_gate_refs` | 같은 기능을 수행했다는 current M3 evidence |
| `profiler_refs`, `analyzer_refs` | 원인 후보 evidence. 수치 대체 아님 |
| `tradeoff_review` | memory·size·correctness·maintainability 변화 |
| `comparison_state` | `comparable\|not_comparable\|no_measurement\|noisy\|inconclusive` |
| `completeness` | `complete\|partial\|unverified`; comparability와 직교 |

required metric 하나라도 numeric sample이 없거나 comparability axis가 `different|unverified`이면 해당 metric의 relative delta·percentage·pass를 만들지 않는다. profiler와 build analyzer의 hotspot/cause는 candidate evidence이며 결정적 causal proof가 없으면 `root_candidate` 또는 `HUMAN_REVIEW`다.

### correctness와 유지보수 trade-off

성능 최적화의 post Gate는 M2가 선택한 correctness·contract·test Check를 먼저 만족해야 한다. 더 빠르지만 output, error, ordering, precision, security 또는 public contract가 달라진 결과를 개선으로 승인하지 않는다.

`tradeoff_review`는 최소 다음을 분리한다.

- time/throughput
- memory
- artifact size
- clean/incremental/cache build
- correctness·compatibility
- code complexity·maintainability와 operational cost

결정적 metric은 자동 비교할 수 있지만 유지보수 의미·복잡성 수용은 CLI-only `HUMAN_REVIEW`일 수 있다. 숫자 개선만으로 의미 검토를 자동 통과시키지 않는다.

## 언어·플랫폼 migration 계약

### 현재 동작 계약

언어·runtime·platform 이동 전에 baseline `BehaviorContract`를 고정한다.

| dimension | 예 |
|---|---|
| public surface | API·CLI·Schema·file format·error/config ID |
| input/output | valid/invalid input, normalization, ordering, encoding |
| state transition | lifecycle·transaction·idempotency·recovery |
| error semantics | stable code, retryability, exit/status mapping |
| concurrency | ordering·atomicity·cancellation·timeout |
| filesystem/process | path·case·ACL·environment·signal/Job semantics |
| serialization | unknown field, numeric/date/locale·round-trip |
| security | auth·permission·secret·crypto boundary |
| operational | startup, shutdown, logging, observability |
| performance | 중요 경로가 명시된 경우 budget/equivalence |

baseline은 M6 immutable contract snapshot, current tests/fixtures, M7 ReproductionPack과 exact environment evidence를 참조한다. 기존 구현의 우연한 bug를 자동으로 “계약”으로 승격하지 않는다. 유지/수정 여부를 결정할 수 없으면 `HUMAN_REVIEW`와 explicit decision을 요구한다.

### LanguageMigrationPlan

| 필드 | 의미 |
|---|---|
| `language_migration_plan_id`, `revision` | immutable plan |
| `task_spec_ref`, `impact_analysis_ref` | 요청·영향 |
| `project_id`, `checkout_id` | 한 Project 경계 |
| `source_stack`, `target_stack` | language/runtime/SDK/arch/OS/toolchain exact fingerprint |
| `behavior_contract_refs` | baseline dimension과 approval |
| `boundary_adapter_specs` | old/new 구현을 같은 contract 뒤에 두는 adapter |
| `coexistence_phases` | 단계별 source·consumer·writer/reader 상태 |
| `consumer_transition_order` | consumer ID, minimum version, switch·fallback 순서 |
| `recipe_refs`, `codegen_refs` | M4 codemod/codegen assurance·provenance |
| `comparison_plan` | build·test·contract·differential·performance Check |
| `compatibility_window` | 시작/종료 version·condition과 old path 보존 |
| `cutover_plan` | exact switch, observation, stop condition |
| `rollback_plan_ref` | adapter switch·old implementation·data compatibility |
| `platform_evidence_matrix` | claimed OS/arch와 actual evidence source |
| `unknown_semantics` | unresolved item·owner·HUMAN_REVIEW 질문 |
| `plan_fingerprint` | canonical hash |

### 단계별 공존과 전환 순서

기본 phase는 다음과 같다.

1. **baseline freeze**: current behavior·contract·failure·performance baseline을 고정한다.
2. **boundary introduce**: consumer가 구현 세부가 아니라 stable adapter/port를 사용하게 한다.
3. **target implement**: 새 구현을 old path와 분리된 source/module/artifact로 준비한다.
4. **shadow/differential**: 같은 input에서 old/new 결과를 비교하되 target output을 정본 write로 사용하지 않는다.
5. **reader first**: 새 format/runtime을 읽을 수 있는 consumer를 먼저 배포·검증한다.
6. **bounded consumer switch**: low-risk consumer부터 target adapter로 전환하고 fallback을 유지한다.
7. **writer/source cutover**: 모든 required reader·consumer가 compatible일 때 authoritative writer/source를 전환한다.
8. **compatibility window**: old path·adapter·format을 finite window 동안 유지하고 actual consumer 전환을 관찰한다.
9. **old path removal**: old reference 0, complete consumer coverage, rollback/restore와 M3 post Gate 뒤에만 제거한다.

`dual_write`는 기본 phase가 아니다. 두 구현이 같은 transaction·idempotency·ordering·failure semantics를 증명하고 divergence reconciliation을 가진 경우에만 별도 destructive-risk 설계로 허용한다. 그렇지 않으면 shadow read/differential comparison을 사용한다.

provider/source를 먼저 배포하되 old consumer가 계속 동작하는 additive boundary를 제공하고, consumer를 전환한 뒤에만 old surface를 제거한다. target writer가 old reader가 읽을 수 없는 data를 쓰면 data migration plan과 reader-first Gate가 필수다.

### 경계 adapter

boundary adapter는 다음을 명시한다.

- stable input/output/error/state contract
- source/target implementation mapping과 version range
- unknown/unsupported behavior
- timeout·cancellation·resource ownership
- serialization·path·encoding 변환
- fallback eligibility와 irreversible boundary
- telemetry/diagnostic mapping

adapter가 semantic gap을 default 값으로 숨기면 안 된다. 구현별 raw error·type을 core public contract로 새게 하지 않고 stable Diagnostic과 `HUMAN_REVIEW|BLOCK`으로 정규화한다.

### codegen·codemod Recipe

- authoritative Schema/IDL/generator input을 먼저 정하고 generated output을 직접 편집하지 않는다.
- codegen은 generator ID/version/hash, input manifest, declared output manifest와 replay 결과를 가진다.
- codemod는 M4 `text|syntax|symbol-aware|codegen` assurance를 실제 capability대로 표시한다.
- text-only 변환을 semantic equivalence로 승격하지 않는다.
- dynamic dispatch, reflection, FFI, concurrency, unsafe, numeric/encoding 차이와 platform API는 자동 번역 완료를 보장하지 않는다.
- unresolved meaning은 location·consumer·risk와 함께 `HUMAN_REVIEW`로 남긴다.
- source/consumer PatchSet은 각각 exact Project/Checkout을 유지하며 여러 Project apply는 9단계로 넘긴다.

### EquivalenceReport

| 필드 | 의미 |
|---|---|
| `equivalence_report_id` | immutable report |
| `plan_ref` | LanguageMigrationPlan |
| `baseline_subject`, `candidate_subject` | exact old/new revision·environment |
| `dimension_results` | BehaviorContract dimension별 evidence·status |
| `build_compile_result` | compile/build만의 독립 상태 |
| `test_contract_results` | unit·integration·contract·differential 결과 |
| `performance_comparison_refs` | required workload만 |
| `platform_matrix_results` | OS/arch별 actual/remote/unverified provenance |
| `consumer_results` | consumer별 old/new/fallback 상태 |
| `unknown_semantics` | 사람 판단 항목 |
| `equivalence_state` | 아래 집계 상태 |
| `gate_refs` | cutover 전·후 M3 decision |

dimension status는 `equivalent|not_equivalent|partial|not_run|unverified|human_review|not_required`다. 전체 `equivalence_state`는 다음과 같다.

| 상태 | 의미 |
|---|---|
| `not_evaluated` | required comparison을 시작하지 않음 |
| `partial` | 일부 dimension만 current complete |
| `equivalent` | 모든 required dimension이 current·complete·stable equivalent |
| `not_equivalent` | required dimension 하나 이상 confirmed mismatch |
| `human_review` | evidence complete지만 의미 결정을 자동화할 수 없음 |
| `unverified` | evidence missing/stale/unsupported environment |

compile 성공은 `build_compile_result=pass` 하나만 증명한다. runtime behavior, error, serialization, concurrency, consumer, security와 performance 동등성을 증명하지 않는다. compile pass만 있고 required behavior evidence가 없으면 전체는 `partial|unverified`다.

### OS·platform evidence

Star-Control 제품 runtime의 실제 지원 범위는 Windows다. local runner가 Windows에서 Linux/macOS/mobile/native ARM64 동작을 실행했다고 표시할 수 없다.

- local Windows evidence는 exact OS build·arch·runtime·toolchain fingerprint를 가진다.
- 다른 OS 결과는 authenticated CI/remote adapter가 exact source·input·tool identity를 제공할 때만 그 OS evidence로 수집한다.
- cross-compile 성공은 target OS runtime success가 아니다.
- emulator/simulator 결과는 native device와 구분한다.
- 지원 환경이 없으면 `not_run|unverified|blocked_external`이며 허위 pass를 만들지 않는다.
- platform compatibility mapping이 없는 서로 다른 environment 결과를 같은 equivalence pair로 묶지 않는다.

### cutover와 rollback

cutover는 exact plan·PatchSet·consumer set·compatibility window·M3 pre Gate와 사용자 approval을 요구한다. target adapter 전환 뒤 current source와 consumer를 재관찰하고 EquivalenceReport·post Gate를 새로 만든다.

cutover approval도 exact LanguageMigrationPlan·ValidationPlan·GatePolicy에 먼저 결합하고, Controller는 current `language_cutover` GateDecision 뒤에만 single-use cutover permit을 만든다. approval만으로 writer/source switch port를 열지 않는다.

rollback은 old adapter/implementation이 여전히 compatible하고 target writer가 만든 data를 읽을 수 있을 때만 단순 switch가 가능하다. 그렇지 않으면 data migration `RecoveryPlan`과 verified restore/roll-forward가 필요하다. compatibility window 종료 전 old path를 삭제하지 않는다.

## 외부 adapter와 permission

| 도구 | 연결 방식 | core가 믿지 않는 것 |
|---|---|---|
| DB/config migration tool | typed `ToolDescriptor` 또는 `star_json_stdio_v1` adapter | exit 0, “migrated” 문자열 |
| backup/restore tool | effect·target·consistency capability가 있는 adapter | file 존재만으로 restore 가능 |
| benchmark runner | registered workload invocation | 단일 run과 unit 없는 숫자 |
| profiler | external adapter·ArtifactRef | hotspot을 자동 원인 확정 |
| build analyzer/cache tool | external adapter와 cache protocol | hit/miss 문자열만으로 cache state |
| compiler/test runner | registered Check/Tool | compile pass를 equivalence로 승격 |
| codegen/codemod | M4 transformer·Tool binding | text rewrite를 semantic-safe로 승격 |
| remote CI/platform | authenticated adapter | 다른 revision·환경 결과 재사용 |

network read/download, package install, system setting, live target write, destructive step, process attach, remote execution, cutover와 rollback은 각각 Permission Action으로 분리한다. 하나의 “migration 승인”으로 모두 묶지 않는다.

## M3 Gate 결합

M8 adapter는 GateDecision을 만들지 않는다. M2가 Profile rule/check/evidence floor를 ValidationPlan에 materialize하고 M3가 다음 phase를 판정한다.

| phase | 필수 조건 |
|---|---|
| `migration_pre_execute` | current plan, dry-run, backup integrity, required restore/migration rehearsal, approval, rollback, no unresolved destructive/unknown field |
| `migration_post_execute` | target version, 모든 required invariant, actual before/after, consumer/contract Check, current complete stable evidence |
| `migration_post_rollback` | before-compatible version·state, active pointer, invariant·consumer Check와 rollback Gate |
| `performance_compare` | comparable cohorts, numeric samples·unit·collector, noise/outlier protocol, correctness Gate |
| `language_cutover` | required Equivalence dimension, consumer order/window, rollback readiness, exact platform evidence와 approval |

`succeeded`, `comparison_state=comparable`, `equivalent`는 각각 domain result이고 `AUTO_PASS`와 같은 값이 아니다. M3는 exact result ref를 평가해 `auto_pass|human_review|block`을 만든다.

- required migration invariant는 ratchet 대상이 아니다.
- required performance metric이 unmeasured/noisy/incomparable이면 performance claim은 pass가 아니다.
- required behavior dimension이 partial/not_run/unverified이면 language equivalence는 pass가 아니다.
- complete evidence 뒤 남은 의미 판단은 CLI-only `HUMAN_REVIEW`다.
- evidence missing/stale, destructive approval 누락, outcome unknown, live partial, rollback 실패는 `BLOCK`이다.

## CLI 목표 surface

모든 command는 Controller application service를 호출하며 DB/evidence/target을 직접 열지 않는다. 아래는 목표 설계이고 현재 구현된 명령이 아니다.

```text
star migration inspect --project <id> --target <id>
star migration plan --task-spec <ref> --target <id>
star migration dry-run --plan <id> --fingerprint <sha256>
star migration backup --plan <id> --fingerprint <sha256>
star migration rehearse --plan <id> --fingerprint <sha256>
star migration execute --plan <id> --fingerprint <sha256>
star migration resume --plan <id> --checkpoint <id>
star migration rollback --plan <id> --recovery-plan <id>
star migration status --plan <id>

star performance plan --workload <id> --baseline <subject> --candidate <subject>
star performance run --plan <id> --cohort baseline|candidate
star performance compare --plan <id>

star language-migration plan --task-spec <ref>
star language-migration equivalence --plan <id>
star language-migration cutover --plan <id> --fingerprint <sha256>
star language-migration status --plan <id>
```

`plan`, `inspect`, `status`, `compare`는 source/target read-only다. `dry-run`도 live target write가 없어야 한다. `backup`, `rehearse`, `execute`, `resume`, `rollback`, performance run과 cutover는 effect·target·permission을 명시한다. CLI 이름이 같아도 approval을 암시하지 않는다.

## Stable Diagnostic과 오류 범주

stable ErrorEnvelope namespace·CLI 종료 code의 중앙 정본은 [오류와 진단 계약](errors-and-diagnostics.md)이다. 아래 표는 M8 domain 상태가 그 namespace에 요구하는 mapping이며 문자열 message를 분기 조건으로 사용하지 않는다.

| code | 의미 |
|---|---|
| `MIGRATION_VERSION_UNKNOWN` | current/target version을 확정할 수 없음 |
| `MIGRATION_CHAIN_GAP` | 연속 chain 없음 |
| `MIGRATION_CHAIN_AMBIGUOUS` | 둘 이상의 자동 선택 불가 path |
| `MIGRATION_UNKNOWN_FIELD_UNPRESERVED` | unknown/extension round-trip 불가 |
| `MIGRATION_BACKUP_UNVERIFIED` | backup 존재와 integrity/restore 검증 부족 |
| `MIGRATION_REHEARSAL_REQUIRED` | required rehearsal 누락·불일치 |
| `MIGRATION_APPROVAL_REQUIRED` | live/destructive exact approval 없음 |
| `MIGRATION_PRECONDITION_STALE` | plan subject·tool·config·environment 변화 |
| `MIGRATION_OUTCOME_UNKNOWN` | effect commit 여부 판정 불가 |
| `MIGRATION_PARTIAL` | target version/Gate 전 일부 durable step만 완료 |
| `MIGRATION_INVARIANT_FAILED` | required invariant 불충족 |
| `MIGRATION_ROLLBACK_FAILED` | rollback 또는 post-rollback 검증 실패 |
| `PERFORMANCE_WORKLOAD_NOT_DECLARED` | 중요 경로/workload 선언 없음 |
| `PERFORMANCE_MEASUREMENT_UNAVAILABLE` | numeric value·unit·collector 없음 |
| `PERFORMANCE_NOT_COMPARABLE` | workload/input/tool/environment/mode/revision 조건 불일치 |
| `PERFORMANCE_NOISE_INCONCLUSIVE` | noise/outlier 뒤 결론 불가 |
| `PERFORMANCE_CORRECTNESS_UNVERIFIED` | candidate 기능 correctness 미확인 |
| `LANGUAGE_BEHAVIOR_BASELINE_MISSING` | 현재 동작 계약 없음 |
| `LANGUAGE_EQUIVALENCE_INCOMPLETE` | required dimension partial/not_run/unverified |
| `LANGUAGE_SEMANTICS_HUMAN_REVIEW` | 자동 번역·판정 불가 의미 |
| `PLATFORM_RUNTIME_UNVERIFIED` | 실제 지원 OS/arch runtime evidence 없음 |
| `LANGUAGE_CUTOVER_NOT_READY` | consumer/window/rollback/Gate 미충족 |
| `CROSS_PROJECT_MIGRATION_DEFERRED` | 9단계 ChangeBundle 필요 |

tool-native code와 message는 cause evidence로 보존하되 CLI가 문자열을 parse해 위 상태를 만들지 않는다.

## 구현 순서

M8 제품 구현 승인이 난 뒤 다음 순서를 따른다.

1. 12개 top-level 계약과 nested type, valid/full/invalid/future fixture와 fingerprint golden
2. `ValidationPlan`·`EvidenceSubjectBinding`·`EvidenceBundle` M8 reference와 Gate phase version migration
3. manifest loader, version probe·chain resolver·state projection pure function
4. fake migration target에서 dry-run·checkpoint·reconcile·partial·rollback·restore conformance
5. read-only `inspect|plan|status` CLI와 JSON output
6. isolated backup/restore/migration rehearsal와 M3 pre/post/rollback Gate
7. approval-gated single-Project execute/resume/rollback
8. PerformanceWorkloadSpec loader, fake metric collector, comparability/noise/outlier pure comparison
9. registered profiler/build analyzer adapter와 explicit workload CLI
10. LanguageMigrationPlan·EquivalenceReport, fake boundary adapter·consumer transition corpus
11. M4 codegen/codemod·M6 compatibility·M3 cutover Gate 통합
12. `CrossProjectMigrationHandoff` 생성까지만 구현하고 multi-project execute는 9단계로 보류

실제 project migration framework, benchmark, profiler, compiler와 target OS adapter는 fake contract conformance가 통과한 뒤 하나씩 연결한다. 외부 도구 선택은 구현 직전에 공식 지원 version·license·Windows x64/ARM64·output Schema·headless execution을 다시 확인한다.

## Fixture와 Corpus

### migration

- current/oldest/future/unknown/corrupt version
- unique chain, gap, ambiguous branch, cycle, duplicate step
- unknown field preserved/dropped, enum unknown, extension namespace
- dry-run live-write 시도 탐지
- backup created but corrupt, integrity pass but restore fail, restore validated
- side-by-side activation success/startup fail/atomic rollback
- transactional in-place capability false claim
- crash before step, during non-replay-safe step, after commit before receipt
- resume before/after/diverged/outcome unknown
- succeeded/partial/failed/rolled_back/rollback_failed state projection
- destructive approval stale·scope mismatch
- global/project internal store와 generic project plan 혼용 거부

### performance

- workload 미선언, metric 없음, unit/collector 없음
- baseline/candidate workload·input·tool·environment·cache mode mismatch
- cohort 안 revision 혼합과 undeclared dirty delta
- warmup 분리, measured failure 보존, minimum run 미달
- predeclared outlier, post-hoc outlier 변경 거부, 포함/제외 통계
- high noise·bounded rerun·inconclusive
- clean/incremental/cache hit/miss 분리
- profiler hotspot은 candidate이고 causal proof 아님
- faster but correctness fail·memory/size regression·maintainability review

### language·platform

- compile pass지만 behavior mismatch
- output/error/serialization/ordering/concurrency 차이
- reader-first success, writer-first incompatibility 차단
- boundary adapter fallback과 irreversible data write
- codegen provenance·codemod assurance·text-only semantic gap
- unknown reflection/FFI/platform API `HUMAN_REVIEW`
- Windows local, authenticated remote OS, cross-compile, simulator, native evidence 구분
- compatibility window 전 old path removal 차단
- cutover 뒤 rollback 가능/불가와 data migration 연결
- 여러 Project plan을 M8에서 apply하지 않고 handoff만 생성

## 완료 조건

- migration 성공·부분 성공·실패·outcome unknown·rollback 성공·rollback 실패가 서로 다른 상태다.
- backup byte 존재, integrity 검증, restore rehearsal와 restore 후 behavior 검증이 구분된다.
- 모든 migration은 explicit version source, 연속 chain, dry-run, backup, rehearsal, execute, validate, resume와 rollback 의미를 가진다.
- destructive step과 unknown field loss는 exact 사용자 승인 없이는 실행할 수 없다.
- config, persisted state, management DB, IPC, MCP/Plugin과 Catalog version을 하나로 묶지 않는다.
- Star-Control 자체 관리 DB 최소 migration은 0단계 `star-state` 경계에 있고 범용 Project migration은 M8 Profile이 소유한다.
- 성능 Profile은 declared workload만 활성화하고 같은 protocol·input·tool·environment·mode와 exact cohort revision에서 숫자를 비교한다.
- warmup·반복·noise·outlier와 raw sample이 보존되고 측정값이 없으면 결과를 만들지 않는다.
- profiler·build analyzer·compiler·migration tool은 adapter이고 Gate writer가 아니다.
- 언어 migration은 compile/build와 기능 동등성을 분리하고 source·consumer·writer 전환 순서와 compatibility window를 가진다.
- 자동 번역이 확정하지 못한 의미는 `HUMAN_REVIEW`이고 실제 지원 환경 밖 결과는 `unverified`다.
- 여러 Project migration은 project별 plan·PatchSet·Gate·rollback을 유지한 `CrossProjectMigrationHandoff`로 9단계 ChangeBundle에 전달된다.
- 현재 문서만으로 구현자가 contract, state machine, adapter, permission, evidence, fixture와 구현 순서를 재구성할 수 있다.
