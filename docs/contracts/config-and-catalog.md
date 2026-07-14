# 설정과 Catalog 계약

## 목표와 경계

설정은 사용자가 바꿀 수 있는 선택과 제품이 지켜야 하는 안전 제한을 분리한다. 개인 사용자는 `personal_auto` 정책 Profile로 유료 동작 외의 범위 내 작업을 자동 진행할 수 있고, 공개 배포본은 `safe_default` 정책 Profile로 시작한다. 어느 정책·작업 Profile도 Codex, 운영체제 또는 관리자가 강제한 제한을 약화하지 못한다.

이 문서는 설정의 형식, 병합, 출처 추적과 Catalog descriptor를 소유한다. 여러 Project·언어·문서가 공유하는 config key·default의 stable identity와 binding·lifecycle은 [관리형 Symbol·상수·에러 코드 Registry 계약](managed-symbol-registry.md)이 별도로 소유한다. 설치·update·개인정보와 공개 release 절차는 [설치와 공개 배포](../operations/installation.md), release/evaluation 적용 순서와 상태는 [10단계 CI·Release·평가·최종 제품 완성 계약](ci-release-evaluation-and-product-completion.md)이 소유한다.

## 설정 파일과 형식

| 종류 | 위치 | 역할 |
|---|---|---|
| 사용자 설정 | `%APPDATA%\Star-Control\config.toml` | 모든 프로젝트의 사용자 선호 |
| 프로젝트 설정 | `<project>\.star-control\config.toml` | 저장소별 규칙 |
| 목표 설정 | Controller의 GoalSpec 부속 문서 | 한 목표에만 적용하는 값 |
| 일회성 설정 | `star` 명령 또는 MCP 입력 | 한 명령 또는 한 run에만 적용 |
| 실행 상태 | `%LOCALAPPDATA%\Star-Control\` | EffectiveConfig, snapshot과 상태 |

- 사람이 편집하는 설정은 UTF-8 TOML이다. UTF-8 BOM은 읽을 수 있지만 다시 쓸 때는 BOM 없이 정규화한다.
- 파일에는 최상위 `schema_version`과 `policy_profile`을 둔다. 최종 16개 개발 작업 유형은 별도 `default_work_profile` 또는 StageSpec에서 선택한다.
- 중복 key, 잘못된 type, 범위를 벗어난 값과 알 수 없는 활성 key는 오류다.
- `${NAME}` 같은 일반 문자열 치환은 하지 않는다. secret은 뒤에서 정의한 SecretRef만 사용한다.
- 상대 경로는 값이 선언된 source를 기준으로 해석하고 EffectiveConfig에 해석 기준을 남긴다.
- 설정 파일에는 상태, 실행 결과와 자동으로 측정한 capability를 다시 쓰지 않는다.

## 설정 계산 순서

EffectiveConfig는 다음 순서로 만든다.

1. 코드에 포함된 불변 안전 하한과 상한을 준비한다.
2. 제품 기본값을 불러온다.
3. 제품, 사용자와 명시적 Goal·CLI 입력에서 정책 Profile ID를 결정하고 PolicyProfileDescriptor를 한 번 적용한다.
4. 사용자 설정을 적용한다.
5. 프로젝트 설정을 적용한다.
6. 목표 설정을 적용한다.
7. 일회성 typed override를 적용한다.
8. Codex·운영체제·관리자 제한과 CapabilitySnapshot을 마지막 제약으로 적용한다.
9. 전체 교차 검증 뒤 EffectiveConfig와 provenance를 생성한다.

정책 Profile 선택만 먼저 계산하며 나머지 값은 위 순서대로 한 번만 병합한다. 프로젝트 파일은 더 강한 `required_policy_profile`을 요구할 수 있지만 개인 정책을 더 넓게 바꿀 수 없다. 작업 Profile은 작업 성격·단계·검사 조합만 선택하고 permission을 넓히지 않는다. Profile 상속은 하나의 부모만 허용하고 순환을 거부한다. 같은 source 안에서 중복 선언된 값은 뒤의 값을 택하지 않고 파일 전체를 거부한다.

프로젝트 설정에 `policy_profile`이 있으면 오류로 거부하고 `required_policy_profile`을 사용하라고 안내한다. Goal·CLI·MCP가 현재 사용자 정책보다 더 넓은 정책 Profile을 요청하면 이를 일반 override로 적용하지 않고 사용자 결정과 새 scope hash를 요구한다. `default_work_profile`이 없으면 Planner가 목표 성격에서 Stage별 작업 Profile을 선택한다.

## 병합 규칙

각 설정 필드는 Schema에 `merge_strategy`를 명시한다. 단순히 모든 값을 마지막 값으로 덮지 않는다.

| 전략 | 적용 대상 | 규칙 |
|---|---|---|
| `replace` | 일반 scalar, 순서 있는 array | 뒤 source의 값으로 교체 |
| `deep_merge` | key별 map | key 단위로 재귀 병합 |
| `most_restrictive` | 행동 승인 | `deny > prompt > auto` |
| `minimum_limit` | 비용·시간·동시 실행 상한 | 설정된 값 중 가장 작은 한도 |
| `intersection` | 허용 모델·실행 방식·capability | 모든 제약이 허용한 항목만 남김 |
| `union` | 보호 경로·필수 검사·redaction rule | 하나라도 요구하면 포함 |
| `immutable` | schema version, 외부 강제 제한 | 하위 source가 변경 불가 |
| `policy_allow_then_false_wins` | 기본 금지인 고위험 capability | 제품/사용자 PolicyProfile만 true를 허용할 수 있고 이후 project·Goal은 false로 제한만 가능 |

- 배열은 기본적으로 `replace`다. 일부 추가·제거만 필요하면 해당 필드가 명시적으로 `add`와 `remove` patch를 제공한다.
- 값 부재는 상속이고 `null`이나 빈 문자열은 삭제 의미가 아니다. 삭제 가능한 map 항목만 typed `remove` patch를 허용한다.
- 안전 관련 필드를 일반 `replace`로 선언할 수 없다.
- 서로 모순된 제한으로 허용 집합이 비면 자동 완화하지 않고 `CONFIG_CONSTRAINT_CONFLICT`로 중단한다.
- CLI와 MCP override도 같은 Schema와 병합 규칙을 사용하며 임의 string key를 core로 전달하지 않는다.

## EffectiveConfig 계약

`EffectiveConfig`는 실행할 때의 유일한 설정 입력이다. 원본 StarConfig를 application layer가 각자 다시 읽지 않는다.

| 필드 | 형식 | 의미 |
|---|---|---|
| `schema_id` | string | `star.effective-config` |
| `schema_version` | integer | EffectiveConfig 계약 version |
| `policy_profile_id` | Catalog ID | 적용된 최종 정책 Profile |
| `default_work_profile_id` | optional Catalog ID | 명시한 경우의 기본 작업 Profile |
| `values` | StarConfigValues | 검증과 제약 적용이 끝난 값 |
| `provenance` | map<field, ConfigOrigin> | 각 leaf 값의 source, 파일, 위치와 merge 전략 |
| `constraints` | ConfigConstraint array | Codex·관리자·제품 하한 등 적용된 제약 |
| `warnings` | Diagnostic ref array | 실행을 막지는 않는 설정 문제 |
| `catalog_snapshot_ref` | ArtifactRef | 해석에 사용한 Catalog |
| `capability_snapshot_ref` | ArtifactRef | capability 의존 값이 있을 때의 근거 |
| `fingerprint` | SHA-256 | secret·사용자 이름·raw 절대 경로를 제외한 canonical values와 제약의 hash |
| `computed_at` | RFC 3339 UTC | 계산 완료 시각 |

ConfigOrigin은 `source_kind`, source 문서 ID, 파일 경로를 가린 LocationRef, 선언 위치, 원래 값의 hash를 가진다. secret 값은 provenance와 fingerprint에 포함하지 않고 reference 식별자만 포함한다.

## 최상위 설정 묶음

### 공통 key

| key | type | 제품 기본값 | 병합 |
|---|---|---|---|
| `schema_version` | positive integer | 현재 version | `immutable` |
| `policy_profile` | Catalog ID | `star.policy-profile.safe-default` | 사용자·명시적 Goal·CLI 선택 |
| `required_policy_profile` | optional Catalog ID | 없음 | 프로젝트가 요구하는 더 강한 제한 |
| `default_work_profile` | optional Catalog ID | 없음 | 작업 성격 기본값. permission과 무관 |

CLI와 사용자 문서에서는 `safe_default`, `personal_auto`를 각각 `star.policy-profile.safe-default`, `star.policy-profile.personal-auto`의 짧은 별칭으로 받을 수 있다. EffectiveConfig와 evidence에는 항상 전체 Catalog ID를 저장한다.

### `[controller]`

| key | type | 기본값 | 설명 |
|---|---|---|---|
| `auto_start` | boolean | `true` | pipe가 없을 때 CLI·MCP의 verified fallback 시작 허용. logon entry는 설치·controller 명령이 관리 |
| `shutdown_grace_ms` | integer | `10000` | 정상 종료 유예 |
| `command_timeout_ms` | integer | `300000` | application command 기본 대기 한도 |
| `recovery_on_start` | boolean | `true` | 미완료 effect와 snapshot 검사 |

`controller.auto_start`는 사용자 설정에서만 선언할 수 있다. project·Goal·MCP 입력은 사용자가 끈 fallback start를 다시 켤 수 없다. timeout은 하위 source가 더 짧게만 제한할 수 있다.

### `[codex]`

| key | type | 기본값 | 설명 |
|---|---|---|---|
| `mcp_required` | boolean | `true` | Star-Control MCP가 준비되지 않으면 Goal 시작 거부 |
| `capability_max_age_ms` | integer | `900000` | 새 RouteDecision에 쓸 snapshot 최대 나이 |
| `app_server_start_timeout_ms` | integer | `30000` | readiness 대기 한도 |
| `require_entry_check` | boolean | `true` | Plugin·MCP 활성 상태 확인 |
| `allow_managed_ultra` | boolean | `true` | native Ultra 부재 시 Controller 병렬 조립 허용 |

### `[routing]`

| key | type | 기본값 | 병합 |
|---|---|---|---|
| `default_model_role` | enum | `terra` | `replace` |
| `default_reasoning_effort` | enum | `medium` | capability와 교집합 |
| `plan_reasoning_effort` | enum | `high` | capability와 교집합 |
| `allowed_model_roles` | enum set | 모두 | `intersection` |
| `allowed_execution_modes` | enum set | `single,max,ultra` | `intersection` |
| `unsupported_choice` | enum | `explain_and_fallback` | `replace` |
| `retry_limit` | integer | `1` | `minimum_limit` |
| `escalation_limit` | integer | `2` | `minimum_limit` |
| `max_parallel_codex` | integer | `3` | `minimum_limit` |

`unsupported_choice`는 `fail`, `explain_and_fallback`, `ask` 중 하나다. 안전이나 품질을 낮추는 fallback은 이 값과 무관하게 금지한다.

### `[permissions]`

행동 값은 `auto`, `prompt`, `deny`다. `default_action`은 분류되지 않은 새 행동에 적용하며, 새 행동을 자동 허용하지 않도록 공개 기본값은 `prompt`다.

| key | type | 기본값 | 설명 |
|---|---|---|---|
| `default_action` | action policy | `prompt` | 미분류 행동 |
| `approval_ttl_ms` | integer | `1800000` | 승인 요청 만료 시간 |
| `reuse_approval` | boolean | `false` | 같은 범위·revision에서만 재사용 가능 |
| `require_scope_hash` | boolean | `true` | 승인 뒤 대상·비용 변화 감지 |

`[permissions.actions]`에는 최소한 다음 key가 모두 존재한다.

| 행동 ID | 의미 | `safe_default` | `personal_auto` |
|---|---|---|---|
| `local_read` | 목표 범위의 로컬 읽기 | `auto` | `auto` |
| `local_write` | 목표 범위의 되돌릴 수 있는 쓰기 | `auto` | `auto` |
| `local_delete` | 파일 또는 상태 삭제 | `prompt` | `auto` |
| `local_mass_move` | 여러 파일의 대량 이동·이름 변경 | `prompt` | `auto` |
| `process_run` | 명시된 로컬 명령 실행 | `auto` | `auto` |
| `dependency_change` | 의존 항목 추가·update | `prompt` | `auto` |
| `system_change` | 시스템 설정·전역 설치 변경 | `prompt` | `auto` |
| `secret_access` | secret reference 해석 | `prompt` | `auto` |
| `network_read` | 외부 읽기 요청 | `auto` | `auto` |
| `network_download` | 파일·도구·자료 다운로드 | `prompt` | `auto` |
| `external_write` | 외부 서비스 상태 변경 | `prompt` | `prompt` 또는 explicit remote opt-in 범위에서만 `auto` |
| `account_change` | 외부 계정·권한·resource 변경 | `prompt` | `prompt` |
| `plan_execute` | 현재 계획의 실행 시작 | `prompt` | `auto` |
| `git_commit` | 로컬 commit 생성 | `prompt` | `auto` |
| `git_merge` | branch·worktree 결과 통합 | `prompt` | `auto` |
| `git_push` | 원격 push | `prompt` | `prompt` 또는 explicit remote opt-in 범위에서만 `auto` |
| `pull_request` | PR 생성·수정 | `prompt` | `prompt` 또는 explicit remote opt-in 범위에서만 `auto` |
| `release_publish` | 공개 release·배포 | `prompt` | `prompt` |
| `paid_action` | 비용이 발생하거나 유료 한도를 쓰는 동작 | `prompt` | `prompt` |

`personal_auto`도 목표 밖 경로, 제품의 deny, Codex approval·sandbox, 관리자 제한을 넘지 않는다. 비용 발생 여부를 판정할 근거가 없으면 `paid_action=prompt`로 취급한다. 공개 `safe_default`는 remote write를 opt-in으로 바꿀 수 없다. `personal_auto` remote write는 아래 user-only opt-in과 action permission을 모두 만족해야 하며 release publish와 account change는 항상 별도 prompt다.

9단계 [CrossRepo ChangeBundle](cross-repo-change-bundle.md)은 더 강한 remote floor를 적용한다. `personal_auto`와 `RemoteWriteScope`가 있어도 `git_push`, `pull_request`, remote merge·protected ref update와 `release_publish`는 bundle·Project·commit·target·action별 current `ApprovalRequest decision=approved`가 필요하다. standing scope는 승인 후보 범위를 좁힐 뿐 approval decision을 합성하지 않는다.

11단계 `rust_style_auto_fix`도 local write의 Profile-specific floor를 적용한다. `safe_default`는 generic `local_write=auto`여도 exact PatchSet 사용자 승인을 요구한다. `personal_auto`는 user-owned standing grant 자체를 승인으로 쓰지 않고, prepare 뒤 exact Project/Profile/pipeline/policy/scope/PatchSet/evidence를 재평가해 current `ApprovalRequest decision=approved`, `resolved_by=policy_evaluator`를 만들 수 있다. 이 예외는 [Rust style 정본](../features/rust-code-style-auto-fix.md#14-personal_auto-자동-적용)의 명시적 terminal workflow에만 적용하며 remote write, background watcher와 다른 mutation Profile로 확장하지 않는다.

7단계 `debug_recovery`, `security_supply_chain`, `dependency_upgrade` Profile은 더 강한 built-in floor를 적용한다. 외부 advisory/version/license refresh의 `network_read`, package·tool 자료의 `network_download`, dependency 추가·update·lockfile 변경의 `dependency_change`, debugger attach·민감 dump capture와 dependency PatchSet apply는 `personal_auto`에서도 `prompt`다. Profile은 이 floor를 낮추거나 action을 획득하지 못한다. 정확한 scope·source·candidate·PatchSet hash가 바뀌면 이전 승인을 재사용하지 않는다.

### `[budgets]`

| key | type | 기본값 | 병합 |
|---|---|---|---|
| `goal_wall_time_ms` | optional integer | 없음 | `minimum_limit` |
| `stage_wall_time_ms` | optional integer | 없음 | `minimum_limit` |
| `goal_paid_action_limit` | optional integer | 없음 | `minimum_limit` |
| `stage_attempt_limit` | integer | `2` | `minimum_limit` |
| `max_artifact_bytes` | integer | `1073741824` | `minimum_limit` |
| `monetary_limit` | optional Money | 없음 | `minimum_limit` |

Money는 `amount`, ISO currency와 검증된 `price_source`를 함께 가져야 한다. 실제 가격 자료가 없으면 금액을 추정하지 않고 paid action 횟수와 측정 가능한 usage만 기록한다.

### `[validation]`

아래 기존 key는 P1/P0 compatibility를 유지한다. `baseline_mode`부터 `max_parallel_checks`까지는 3단계 M3 **목표 설정 계약**이며 현재 StarConfig parser·Schema·제품 code에 구현됐다는 뜻이 아니다. 구현 시 config Schema version, unknown/invalid fixture와 EffectiveConfig fingerprint golden을 함께 추가한다.

| key | type | 기본값 | 병합 |
|---|---|---|---|
| `required_phases` | enum set | `stage,goal` | `union` |
| `fail_on` | severity | `error` | 더 엄격한 severity |
| `command_timeout_ms` | integer | `600000` | `minimum_limit` |
| `allow_manual_evidence` | boolean | `true` | 제한은 `false` 우선 |
| `require_independent_review_for` | risk set | `high,critical` | `union`; review 필요 위험이며 Codex 강제 의미가 아님 |
| `max_log_bytes` | integer | `10485760` | `minimum_limit` |
| `checks_add` | Catalog ID set | 빈 값 | `union` |
| `checks_remove` | Catalog ID set | 빈 값 | 필수 검사는 제거 불가 |
| `baseline_mode` | enum | `ratchet_new_and_worsened` | `most_restrictive` |
| `require_current_evidence` | boolean | `true` | `true_wins` |
| `allow_ratchet_satisfaction` | boolean | `true` | `false_wins` |
| `suppression_requires_expiry` | boolean | `true` | `true_wins` |
| `allow_permanent_suppressions` | boolean | `false` | `policy_allow_then_false_wins` |
| `required_flaky_action` | enum | `human_review` | `block > human_review` |
| `cli_only_semantic_review` | enum | `human_review` | immutable |
| `max_parallel_checks` | integer | `4` | `minimum_limit` |

`baseline_mode`은 `off|report_only|ratchet_new|ratchet_new_and_worsened|clean_only` 순서로 강하다. project·Goal·Profile은 더 강하게 만들 수 있지만 built-in risk floor가 요구한 ratchet을 끌 수 없다. `allow_ratchet_satisfaction=false`는 모든 required Check에 clean pass를 요구한다. true여도 CheckDescriptor가 `ratchet_eligible=true`이고 M3 불변식을 만족해야 한다.

- `off`: Baseline을 읽지 않고 current issue를 `unbaselined`로 평가해 일반 fail threshold를 적용한다.
- `report_only`: relation은 계산하지만 raw failure나 Gate effect를 완화하지 않는다. baseline candidate onboarding에 사용한다.
- `ratchet_new`: compatible baseline의 new issue를 차단하고 worsened는 최소 human review다.
- `ratchet_new_and_worsened`: new와 worsened를 모두 차단한다.
- `clean_only`: existing unchanged를 포함해 fail threshold 이상의 모든 unsuppressed current issue를 차단한다.

`required_flaky_action=human_review`는 flaky를 pass로 허용한다는 뜻이 아니다. security, validator guard, migration invariant, regression pair와 release identity의 built-in floor는 `block`이며 하위 설정이 낮출 수 없다.

`suppression_requires_expiry=true`에서는 일반 suppression에 finite `expires_at`이 필수다. `allow_permanent_suppressions=true`를 더 상위 정책이 명시하고 exact justification·사용자 approval이 있을 때만 permanent candidate를 만들 수 있다. validator guard, secret critical, redaction, stale evidence와 out-of-scope change Rule은 permanent suppression 불가 floor다.

`cli_only_semantic_review`의 v1 허용값은 `human_review` 하나다. `codex`, `ai`, provider/model ID는 validation config 값이 아니며 CLI-only composition에서 발견되면 unknown/constraint error다.

### `[contract_management]`, `[docs_validation]`, `[doctor]`

아래 key는 6단계 **목표 설정 계약**이며 현재 parser·Schema·제품 code에 구현됐다는 뜻이 아니다. project의 비교 대상·baseline ref·docs target·environment constraint 자체는 [6단계 정본](contract-compatibility-and-environment.md)의 `.star-control/contracts.toml`이 소유한다. StarConfig는 이를 약화할 수 없는 실행 정책만 계산한다.

| section.key | type | 기본값 | 병합·불변식 |
|---|---|---|---|
| `contract_management.require_explicit_baseline` | boolean | `true` | `true_wins`; current를 baseline으로 자동 채택 금지 |
| `contract_management.require_complete_consumers` | boolean | `true` | `true_wins`; removal/breaking은 false override 불가 |
| `contract_management.require_companion_changes` | boolean | `true` | `true_wins` |
| `contract_management.breaking_requires_migration_guide` | boolean | `true` | `true_wins` |
| `contract_management.deprecation_window` | enum | `finite_required` | immutable |
| `contract_management.unknown_semantic_action` | enum | `human_review` | immutable; evidence missing은 별도 block |
| `contract_management.public_surface_expansion` | enum | `declared_only` | immutable |
| `docs_validation.require_local_links` | boolean | `true` | `true_wins` |
| `docs_validation.require_registered_commands` | boolean | `true` | `true_wins` |
| `docs_validation.require_config_schema` | boolean | `true` | `true_wins` |
| `docs_validation.require_generated_provenance` | boolean | `true` | `true_wins` |
| `docs_validation.allow_safe_example_execution` | boolean | `false` | `false_wins`; true여도 M2 selected Check·disposable fixture·read-only ToolDescriptor 필요 |
| `doctor.read_only` | boolean | `true` | immutable |
| `doctor.network_action` | enum | `diagnose_only` | immutable |
| `doctor.package_action` | enum | `diagnose_only` | immutable |
| `doctor.system_setting_action` | enum | `diagnose_only` | immutable |
| `doctor.collect_environment_values` | boolean | `false` | immutable |
| `doctor.probe_timeout_ms` | integer | `30000` | `minimum_limit` |
| `doctor.max_output_bytes` | integer | `1048576` | `minimum_limit` |

`diagnose_only`는 필요한 download·install·설정 변경을 Diagnostic의 수동 remediation으로 설명할 수 있다는 뜻이며 action permission을 만들지 않는다. `doctor.read_only=false`, AI/provider key, raw command와 package source credential은 invalid config다.

### `[failure_reproduction]`, `[security_supply_chain]`, `[dependency_maintenance]`, `[maintenance_radar]`

아래 key는 [7단계 실패 재현·보안·의존성 유지보수](failure-security-and-dependency-maintenance.md)의 **목표 설정 계약**이며 현재 parser·Schema·제품 code에 구현됐다는 뜻이 아니다. Project별 package identity, 외부 source, package manager와 failure tool의 선언은 Catalog가 소유하고 StarConfig는 실행·보안 floor만 소유한다.

| section.key | type | 기본값 | 병합·불변식 |
|---|---|---|---|
| `failure_reproduction.max_rerun_attempts` | integer | `3` | `minimum_limit`, 1 이상 |
| `failure_reproduction.require_structured_args` | boolean | `true` | immutable |
| `failure_reproduction.require_before_after` | boolean | `true` | `true_wins` |
| `failure_reproduction.external_condition` | enum | `record_unverified` | immutable |
| `failure_reproduction.default_artifact_role` | enum | `general_log` | immutable; pack이 명시적으로 `reproduction_required` 선택 |
| `failure_reproduction.unsafe_artifact` | enum | `quarantine_or_drop` | immutable |
| `failure_reproduction.debugger_action` | action policy | `prompt` | `deny > prompt`, auto 금지 |
| `security_supply_chain.require_source_provenance` | boolean | `true` | immutable |
| `security_supply_chain.require_freshness` | boolean | `true` | immutable |
| `security_supply_chain.unknown_freshness_action` | enum | `block_required_check` | immutable |
| `security_supply_chain.network_refresh` | action policy | `prompt` | `deny > prompt`, auto 금지 |
| `security_supply_chain.default_max_age_hours` | integer | `168` | source descriptor가 더 짧게 제한 가능, `minimum_limit` |
| `security_supply_chain.default_report_artifacts` | enum | `redacted_only` | immutable |
| `dependency_maintenance.default_stop` | enum | `awaiting_apply_approval` | immutable |
| `dependency_maintenance.lockfile_owner` | enum | `package_manager` | immutable |
| `dependency_maintenance.preview_workspace` | enum | `isolated` | immutable |
| `dependency_maintenance.network_action` | action policy | `prompt` | `deny > prompt`, auto 금지 |
| `dependency_maintenance.download_action` | action policy | `prompt` | `deny > prompt`, auto 금지 |
| `dependency_maintenance.change_action` | action policy | `prompt` | `deny > prompt`, auto 금지 |
| `dependency_maintenance.preserve_before_lockfile` | boolean | `true` | immutable |
| `dependency_maintenance.require_actual_diff_replan` | boolean | `true` | immutable |
| `maintenance_radar.sort_policy` | enum | `risk_freshness_evidence_v1` | immutable |
| `maintenance_radar.include_expiring_suppressions` | boolean | `true` | `true_wins` |
| `maintenance_radar.allow_ai_priority` | boolean | `false` | immutable |

`default_max_age_hours`는 모든 provider가 실제로 7일 주기라는 주장이 아니라 descriptor가 값을 제공하지 못할 때 쓰는 보수적 upper bound다. source `modified_at|published_at`이 없으면 최근 fetch만으로 current를 확정하지 않고 `unknown`을 허용한다.

`max_rerun_attempts`는 자동 retry 횟수를 늘릴 권한이 아니다. ToolDescriptor가 idempotent·retryable을 선언하고 PermissionDecision·budget·stability contract가 허용하는 범위에서만 실행한다. `quarantine_or_drop`은 민감 bytes를 default report에 내보내지 않는다는 뜻이며 보관을 강제하지 않는다. 확인된 secret·token·PII를 안전하게 가릴 수 없으면 quarantine이 아니라 `dropped_sensitive`가 우선한다.

### `[migration]`, `[performance_build]`, `[language_platform_migration]`

아래 key는 [8단계 Migration·성능·언어·플랫폼 계약](migration-performance-and-platform.md)의 **목표 설정 계약**이며 현재 parser·Schema·제품 code에 구현됐다는 뜻이 아니다. Project별 target/version/chain/invariant, workload와 behavior/consumer 선언은 Project Git manifest가 소유하고 StarConfig는 실행 안전 floor와 bounded default만 소유한다.

| section.key | type | 기본값 | 병합·불변식 |
|---|---|---|---|
| `migration.default_strategy` | enum | `side_by_side` | `replace` 뒤 manifest capability·policy 검증; 지원되지 않거나 더 위험한 전략 자동 승격 금지 |
| `migration.require_dry_run` | boolean | `true` | immutable |
| `migration.require_consistent_backup` | boolean | `true` | immutable |
| `migration.require_restore_rehearsal` | boolean | `true` | `true_wins`; destructive/live step에서 false 금지 |
| `migration.require_migration_rehearsal` | boolean | `true` | `true_wins`; destructive/live step에서 false 금지 |
| `migration.unknown_field_action` | enum | `block_unless_preserved` | immutable |
| `migration.live_execute_action` | action policy | `prompt` | `deny > prompt > auto`; project/Goal은 확대 불가 |
| `migration.destructive_action` | action policy | `prompt` | `deny > prompt`; `auto` 금지 |
| `migration.rollback_action` | action policy | `prompt` | `deny > prompt`; emergency라는 이유로 scope 생략 금지 |
| `migration.max_resume_attempts` | integer | `3` | `minimum_limit`, 1 이상 |
| `migration.max_additional_rehearsals` | integer | `2` | `minimum_limit`, 무한 retry 금지 |
| `performance_build.enabled_by_default` | boolean | `false` | immutable; explicit workload activation만 |
| `performance_build.require_declared_workload` | boolean | `true` | immutable |
| `performance_build.default_warmup_runs` | integer | `1` | `replace`, 0 이상; workload spec 값이 있으면 그 값을 사용 |
| `performance_build.default_measurement_runs` | integer | `5` | `replace`, 최소 3; workload spec 값이 있으면 그 값을 사용 |
| `performance_build.minimum_measurement_runs` | integer | `3` | immutable lower bound |
| `performance_build.max_additional_runs` | integer | `5` | `minimum_limit`, noise 때문에 무한 반복 금지 |
| `performance_build.outlier_policy` | enum | `predeclared_report_both` | immutable; raw sample 삭제 금지 |
| `performance_build.missing_measurement_action` | enum | `inconclusive` | immutable; 0·추정치 생성 금지 |
| `performance_build.require_exact_environment` | boolean | `true` | compatible class는 workload descriptor가 더 구체적으로 선언 |
| `performance_build.profiler_action` | action policy | `prompt` | `deny > prompt`; attach/elevated effect는 별도 action |
| `language_platform_migration.require_behavior_contract` | boolean | `true` | immutable |
| `language_platform_migration.compile_only_equivalence` | boolean | `false` | immutable |
| `language_platform_migration.unknown_semantics_action` | enum | `human_review` | immutable |
| `language_platform_migration.compatibility_window` | enum | `finite_required` | immutable |
| `language_platform_migration.cutover_action` | action policy | `prompt` | `deny > prompt`; auto 금지 |
| `language_platform_migration.unsupported_platform_action` | enum | `record_unverified` | immutable |
| `language_platform_migration.allow_full_auto_translation_claim` | boolean | `false` | immutable |

`migration.live_execute_action=auto`는 user-level `personal_auto`가 lossless·replay-safe·single-Project·verified backup/restore/rehearsal·current pre-Gate를 모두 요구하는 exact scope를 별도로 허용한 경우에만 계산할 수 있다. project, Goal, Profile, ToolDescriptor와 CLI/MCP override는 `auto`를 만들 수 없다. `live_destructive`, unknown field loss, irreversible writer cutover와 cross-project effect는 항상 `prompt|deny`다.

`migration.*`은 범용 대상 Project Profile 설정이다. 0단계 Star-Control 자체 store의 source-derived rebuildable projection에 적용하는 `management.auto_migrate_rebuildable`과 합치지 않는다. internal store는 [Version과 Migration 계약](versioning-and-migrations.md)의 별도 lifecycle을 따른다.

성능 측정의 noise threshold, aggregation, metric, unit, cache protocol과 budget은 workload마다 달라야 하므로 전역 default로 만들지 않는다. 값이 없으면 `PerformanceComparison`이 threshold pass/regression을 발명하지 않는다. `require_exact_environment=true`에서 의도하지 않은 environment 차이는 comparable이 아니다. baseline/candidate code revision은 `comparison_intent=source_change|migration_before_after`의 exact ChangeSet에 따라 다를 수 있지만 각 cohort 안 exact revision과 `allowed_delta_axes`가 필수다. toolchain/config 비교는 양쪽 revision이 같아야 한다.

workload가 warmup/measurement run 수를 명시하면 StarConfig default와 병합하지 않고 workload 값을 사용한다. runtime budget이 그 수를 실행할 수 없으면 sample 수를 몰래 줄이지 않고 `not_run|inconclusive`와 필요한 budget을 보고한다.

`compile_only_equivalence=false`는 compiler를 실행하지 말라는 뜻이 아니라 compile 성공만으로 기능 동등성을 만들 수 없다는 뜻이다. 다른 OS/architecture의 authenticated remote evidence가 없으면 `record_unverified`이고 local Windows 결과로 대체하지 않는다.

### `[release]`, `[evaluation]`

아래 key는 [10단계 CI·Release·평가·최종 제품 완성 계약](ci-release-evaluation-and-product-completion.md)의 **목표 설정 계약**이며 현재 parser·Schema·제품 code에 구현됐다는 뜻이 아니다. package file list, supported Windows baseline, channel, version/changelog/license source, supply-chain applicability와 release Gate는 Git의 `packaging/release.toml`과 built-in Catalog가 소유한다. 평가 corpus·case·threshold·recommendation policy는 `evals/`의 versioned source가 소유한다. StarConfig는 이를 다시 선언하지 않고 실행 한도와 더 강한 사용자 제한만 표현한다.

| section.key | type | 기본값 | 병합·불변식 |
|---|---|---|---|
| `release.promotion_mode` | enum | `build_once` | immutable; 검증·승격·publish를 위한 rebuild 금지 |
| `release.require_clean_windows` | boolean | `true` | immutable |
| `release.require_native_arm64_runtime` | boolean | `true` | immutable; cross-build evidence로 대체 금지 |
| `release.require_explicit_remote_action_approval` | boolean | `true` | immutable; publish·deploy·withdraw·rollback 모두 적용 |
| `release.publish_action` | action policy | `prompt` | `deny > prompt`; `auto` 금지 |
| `release.deploy_action` | action policy | `prompt` | `deny > prompt`; target별 `auto` 금지 |
| `release.withdraw_action` | action policy | `prompt` | `deny > prompt`; publish 승인 재사용 금지 |
| `release.rollback_action` | action policy | `prompt` | `deny > prompt`; exact failed deployment와 destination binding 필요 |
| `release.max_parallel_target_jobs` | positive integer | `1` | 상위·provider·resource limit과 `minimum_limit` |
| `evaluation.default_mode` | enum | `shadow` | `offline\|replay\|shadow`; 실제 route·Gate·source·release effect 금지 |
| `evaluation.separate_cli_codex_contexts` | boolean | `true` | immutable |
| `evaluation.provider_verified_cost_only` | boolean | `true` | immutable; 없는 값은 0으로 기록하지 않음 |
| `evaluation.max_attempts_per_case` | positive integer | `3` | policy·budget·Tool stability limit과 `minimum_limit` |
| `evaluation.incomparable_action` | enum | `needs_review` | immutable; 승격·accept로 변환 금지 |

`release.publish_action=prompt`, `release.deploy_action=prompt`, `release.withdraw_action=prompt`와 `release.rollback_action=prompt`는 ApprovalRequest를 자동 생성했다는 뜻이 아니다. Controller가 exact action kind, ReleaseManifest revision, artifact set digest, channel, provider, destination, before RemoteStateSnapshot과 expiry를 묶은 별도 요청을 만들어야 한다. `personal_auto`, Project config, Goal, Profile과 CLI/MCP override는 이를 `auto`로 낮추지 못한다.

`evaluation.default_mode=shadow`는 candidate를 실행할 수 있다는 뜻이지 현재 Rule·Check·Profile·Recipe를 변경할 권한이 아니다. `accept`, deprecation과 migration은 comparable EvaluationRun, validator guard, 별도 Catalog source 변경과 M3 Gate를 요구한다. sample floor·ground truth·threshold가 없는 경우 StarConfig 기본값으로 결과를 발명하지 않고 `needs_review`다.

### `[rust_style]`

아래는 [11단계 Rust 코드 스타일 자동 교정](../features/rust-code-style-auto-fix.md)의 **목표 설정 계약**이며 현재 parser·Schema·Catalog TOML·제품 code에 구현됐다는 뜻이 아니다. StarConfig는 Rust formatting/lint 값을 저장하는 곳이 아니다.

| source | 소유하는 값 | 소유하지 않는 값 |
|---|---|---|
| Git source | `rustfmt.toml`/`.rustfmt.toml`, Cargo `[lints]`/`[workspace.lints]`, source lint attribute, `clippy.toml`/`.clippy.toml`, `rust-toolchain.toml` | 실행 결과·DB projection |
| built-in/user/project Catalog Profile metadata | `rust_style_v1` version, fixed Tool/Check ref, exact Clippy fix allowlist, required package/target/feature/triple/cfg coverage, generated/vendor classifier, diff/Gate floor | 실제 rustfmt option·lint level·Clippy parameter의 복제 |
| user StarConfig | `personal_auto` standing grant ref와 더 강한 실행/retention limit | project가 선택한 style·lint 값, tool version을 대신하는 문자열 |
| project StarConfig | required Rust style Profile/Catalog ref와 더 강한 scope/coverage requirement | user auto grant·permission 확대 |
| CLI/Goal | 한 run의 typed package/workspace scope와 Profile ref | raw cargo/rustfmt/Clippy argv나 shell pipeline |
| management DB | resolved snapshot/fingerprint·coverage·Diagnostic·run/Patch/Evidence projection | source/config/Catalog truth |

`[rust_style]` 목표 field는 다음과 같다.

| section.key | type | 기본값 | 병합·불변식 |
|---|---|---|---|
| `rust_style.required_profile_ref` | optional exact Catalog ref | 없음 | project가 더 강한 Profile variant를 요구할 수 있음; user/Goal이 약화 불가 |
| `rust_style.auto_apply_grant_refs` | user-only exact grant ref set | 빈 값 | project/Goal/CLI에서 선언 금지; Profile·Project별 exact match |
| `rust_style.max_preview_retention` | duration/size limit | 제품 retention policy | `minimum_limit`; evidence floor보다 짧으면 conflict |
| `rust_style.network_action` | action policy | `deny` | immutable `deny`; dependency/tool/component/target download 없음 |
| `rust_style.unpinned_apply_action` | action policy | `deny` | immutable `deny`; inspect/check limitation만 허용 |
| `rust_style.partial_coverage_apply_action` | action policy | `deny` | immutable `deny`; `AUTO_PASS` 합성 금지 |

standing grant는 exact `project_id`, `profile_ref`/definition hash, `pipeline_ref`, `style_policy_fingerprint`, package/workspace와 path `scope_ceiling`, allowed action set, diff limits, required Gate phase, expiry와 grant fingerprint를 가진 user-owned versioned source다. raw command, wildcard Project, lint group/wildcard, open-ended path와 network/dependency/system action은 grant validation에서 거부한다. grant가 바뀌면 이미 준비된 candidate는 자동 승인 대상으로 재사용하지 않는다.

resolved `RustStylePolicySnapshot`은 위 Catalog/Profile metadata와 actual Git config/tool discovery를 결합한 derived evidence다. StarConfig나 DB에 rustfmt option, Cargo lint level, source attribute와 Clippy parameter를 복사해 override하지 않는다. config source가 ambiguous하거나 Catalog coverage가 required Cargo inventory를 설명하지 못하면 `partial|ambiguous`이고 auto apply를 차단한다.

### `[vcs]`, `[remote]`, `[state]`

`vcs.max_parallel_projects`부터 `vcs.worktree_disk_limit_bytes`와 `remote.max_parallel_writes`까지의 ChangeBundle resource key는 9단계 목표 설정 계약이다. 현재 parser·Schema·제품 code에 구현됐다는 뜻이 아니며, 구현 전에는 unknown key로 거부하고 version·invalid/future fixture와 EffectiveConfig golden을 함께 추가한다.

| section.key | type | 기본값 | 설명 |
|---|---|---|---|
| `vcs.use_worktree` | boolean | `true` | 병렬 변경을 별도 작업 복사본에서 수행 |
| `vcs.merge_strategy` | enum | `review_then_merge` | `review_then_merge`, `manual`, `never` |
| `vcs.protected_branches` | string set | repository에서 탐지 | 보호 대상은 `union` |
| `vcs.worktree_root` | path | Controller data 아래 | source 기준으로 해석 |
| `vcs.max_parallel_projects` | positive integer | `2` | ChangeBundle 동시 project 작업 상한, 상위와 `minimum_limit` |
| `vcs.max_active_worktrees` | positive integer | `4` | ready·dirty·validating·retained worktree 총 상한, `minimum_limit` |
| `vcs.max_parallel_mutations_per_repository` | positive integer | `1` | 한 Git common repository의 source effect 상한, `minimum_limit` |
| `vcs.max_parallel_local_merges` | positive integer | `1` | repository별 queue는 항상 직렬, global도 `minimum_limit` |
| `vcs.max_merge_queue_entries` | positive integer | `64` | 한 project queue entry 상한, `minimum_limit` |
| `vcs.worktree_disk_limit_bytes` | optional positive integer | 없음 | 없으면 unlimited가 아니라 BudgetSnapshot `unknown` 허용; adapter hard cap과 `minimum_limit` |
| `remote.allowed_hosts` | host set | 빈 값 | 상위 제한과 `intersection` |
| `remote.require_clean_target` | boolean | `true` | 원격 변경 전 상태 검사 |
| `remote.personal_auto_write_scopes` | RemoteWriteScope array | 빈 값 | 사용자 설정에서만 추가; project·Goal·MCP override 금지 |
| `remote.max_parallel_writes` | positive integer | `1` | ChangeBundle remote effect 동시성 상한; Project별 항상 직렬 |
| `state.artifact_root` | project-relative path | `.ai-runs/star-control` | project root의 `.ai-runs/` 아래에만 허용하는 증거 위치 |
| `state.checkpoint_interval_ms` | integer | `300000` | 긴 실행의 최대 checkpoint 간격 |
| `state.completed_retention_days` | integer | `90` | 완료 run의 큰 원문·중간 artifact 보관 기간 |
| `state.failed_retention_days` | integer | `180` | 해결된 실패의 재현 자료 보관 기간 |
| `state.redaction_rules_add` | rule ID set | built-in rules | `union` |
| `state.cleanup_trigger` | enum | `startup_and_manual` | `manual`, `startup_and_manual`. 자체 예약 실행은 없음 |

보관 정책은 실행 중 자료, 최종 요약·manifest, 보존 hold와 미해결 실패 자료를 삭제 대상으로 만들 수 없다. 실제 삭제는 별도 permission과 audit event를 필요로 한다.

`state.artifact_root`는 normalize 뒤 `.ai-runs/`를 벗어나거나 absolute·UNC·device path가 되면 오류다. DB에는 이 project-relative anchor와 ArtifactRef relative path만 저장한다.

`RemoteWriteScope`는 `host`, canonical `repository_id`, 허용 action set(`external_write`, `git_push`, `pull_request`), optional protected branch set과 optional `expires_at`을 가진다. wildcard host·repository, credential이 포함된 URL, `release_publish`, `account_change`는 허용하지 않는다. scope는 사용자가 직접 관리하는 user config에서만 만들 수 있고 Project config·Catalog·CLI/MCP 일회성 override가 확대하지 못한다. 실제 remote·branch·action이 exact match하고 approval scope hash가 같을 때만 `personal_auto`의 해당 action을 `auto`로 계산한다. 그 외에는 `prompt`다.

위 일반 계산 뒤에도 9단계 ChangeBundle built-in floor가 remote action을 `prompt`로 승격한다. `RemoteWriteScope` 또는 CLI `--yes`를 action별 ApprovalRequest로 변환하지 않는다.

ChangeBundle `resource_budget`은 위 VCS/remote limit, `planning.max_parallel_codex`, `validation.max_parallel_checks`, Goal budget, ToolDescriptor와 OS adapter hard cap의 가장 강한 값을 materialize한다. process·memory·artifact·wall-time dimension도 함께 `BudgetSnapshot`에 예약하며 측정할 수 없는 값을 0 또는 무제한으로 추측하지 않는다.

### `[management]`, `[project_discovery]`, `[scan]`, `[index]`, `[index_cache]`

0단계의 정확한 type·fingerprint·repository 경계는 [공통 개발 관리와 로컬 관리 DB 계약](development-management.md), 1단계의 Project discovery·source 분류·index tier·freshness 의미는 [Project Catalog·Code Index 계약](project-catalog-and-code-index.md)이 소유한다. 설정은 backend나 특정 parser/indexer가 아니라 동작, 분석 범위와 resource 한도만 표현한다.

`project_discovery.*`, `index.*`, `index_cache.*`와 아래 확장된 `scan.*` key는 1단계 **목표 설정 계약**이다. 현재 제품에서 이미 parse·적용된다고 해석하지 않으며 구현 시 Schema version·invalid/unknown fixture와 EffectiveConfig golden을 함께 추가한다.

| section.key | type | 기본값 | 병합 | 의미 |
|---|---|---|---|---|
| `management.integrity_check_on_unclean_start` | enum | `full` | `most_restrictive` | `quick`, `full`; unclean shutdown 뒤 검사 강도 |
| `management.allow_read_only_recovery` | boolean | `true` | `immutable` | future·suspect store의 비변경 진단 허용 |
| `management.auto_migrate_rebuildable` | boolean | `true` | `false_wins` | source-derived index만 바꾸는 non-destructive migration |
| `management.backup_before_migration` | boolean | `true` | `immutable` | migration·repair 전 consistent backup |
| `management.keep_latest_successful_scans` | integer | `2` | `maximum_floor` | project별 complete generation 최소 보존 수 |
| `management.incomplete_staging_retention_days` | integer | `7` | `maximum_floor` | 열린 run이 아닌 staging generation |
| `management.scan_detail_retention_days` | integer | `90` | `maximum_floor` | hold 없는 과거 ScanRun·Occurrence detail |
| `management.resolved_finding_retention_days` | integer | `180` | `maximum_floor` | reference 없는 resolved Finding summary |
| `management.local_decision_retention_days` | integer | `180` | `maximum_floor` | expired·revoked local decision의 최소 보존 |
| `management.migration_backup_min_count` | integer | `2` | `maximum_floor` | latest known-good 외 pre-migration backup 최소 수 |
| `management.suppression_default_expiry_days` | integer | `90` | `minimum_limit` | expires_at 없는 non-permanent Suppression에 적용 |
| `management.baseline_activation` | enum | `explicit_review` | `immutable` | complete ScanRun review 뒤 명시적 생성만 허용 |
| `project_discovery.roots_add` | RootBindingRef set | 빈 값 | `union_local_roots` | CLI 또는 user config가 제공한 여러 discovery root; raw path는 설정 해석 뒤 persisted snapshot에 남기지 않음 |
| `project_discovery.detect_nested_repositories` | boolean | `true` | `false_wins` | parent project 아래의 별도 repository 후보 발견 |
| `project_discovery.detect_linked_worktrees` | boolean | `true` | `false_wins` | Git common directory가 공유되는 linked worktree 관찰 |
| `project_discovery.detect_workspaces` | boolean | `true` | `false_wins` | manifest가 선언한 workspace와 member 관계 발견 |
| `project_discovery.detect_non_git` | boolean | `true` | `false_wins` | manifest·toolchain marker가 있는 non-Git project 후보 발견 |
| `project_discovery.follow_symlinks` | boolean | `false` | `false_wins` | discovery recursion의 symlink·junction 추적; root escape와 cycle 검증은 항상 적용 |
| `project_discovery.max_depth` | integer | `16` | `minimum_limit` | 각 explicit root 아래 recursion 깊이 상한 |
| `project_discovery.max_directories` | integer | `100000` | `minimum_limit` | 한 discovery에서 방문할 directory 상한 |
| `project_discovery.exclude_paths_add` | project-relative glob set | built-in cache·vendor·output root set | `union` | nested root를 찾기 위해 내려가지 않을 subtree; explicit root 자체는 이 규칙보다 우선 |
| `project_discovery.search_ignored_subtrees` | boolean | `false` | `explicit_widening` | ignored subtree 안의 nested project 탐색; 기본은 limitation count만 남김 |
| `scan.incremental` | boolean | `true` | `false_wins` | content·input fingerprint가 같은 결과 재사용 |
| `scan.include_untracked` | boolean | `true` | `replace` | Git untracked file을 WorkspaceSnapshot에 포함 |
| `scan.include_ignored` | boolean | `false` | `explicit_widening` | ignored file 포함 |
| `scan.follow_symlinks` | boolean | `false` | `false_wins` | root escape·cycle 방지를 위한 기본값 |
| `scan.binary_mode` | enum | `metadata_only` | `most_restrictive` | `skip`, `metadata_only`; source byte를 DB에 저장하지 않음 |
| `scan.max_file_bytes` | integer | `16777216` | `minimum_limit` | 한 source file의 읽기 상한 |
| `scan.max_files` | integer | `200000` | `minimum_limit` | 한 ScanRun source entry 상한 |
| `scan.max_total_bytes` | integer | `8589934592` | `minimum_limit` | 한 ScanRun에서 hash·parse할 총 byte 상한 |
| `scan.max_parallel_files` | integer | `4` | `minimum_limit` | file 분석 동시성 |
| `scan.require_complete_for_gate` | boolean | `true` | `true_wins` | incomplete scan의 auto pass 금지 |
| `scan.rule_error_policy` | enum | `mark_incomplete` | `most_restrictive` | `mark_incomplete`, `fail_scan` |
| `scan.include_paths` | project-relative glob array | 빈 값 | `intersection_scope` | 빈 값은 project 전체 |
| `scan.exclude_paths_add` | project-relative glob set | `.git/**`, `.ai-runs/**` | `union` | scan recursion과 VCS 내부 자료 제외 |
| `scan.classification_rules_add` | SourceClassificationRule array | 빈 값 | `append_unique_by_id` | source class·facet·생성 주체 override; 같은 ID의 다른 정의는 오류 |
| `scan.rule_sets_add` | Catalog ID set | built-in required set | `union` | 실행할 Rule set 추가 |
| `scan.rule_sets_remove` | Catalog ID set | 빈 값 | `subtract_optional` | 선택 Rule만 제거하며 required Rule 제거는 오류 |
| `scan.hardcoding_rules_enabled` | boolean | `true` | `false_wins` | 절대 경로·endpoint·수치 한도·raw command·error string·config 중복 후보 Rule 실행 |
| `scan.hardcoding_include_tests` | boolean | `false` | `explicit_widening` | test primary class를 hardcoding Rule에 포함 |
| `scan.hardcoding_include_fixtures` | boolean | `false` | `explicit_widening` | fixture facet을 hardcoding Rule에 포함 |
| `scan.hardcoding_include_docs_examples` | boolean | `false` | `explicit_widening` | docs example facet을 hardcoding Rule에 포함 |
| `scan.hardcoding_include_generated` | boolean | `false` | `explicit_widening` | generated source를 hardcoding Rule에 포함 |
| `scan.hardcoding_include_vendor` | boolean | `false` | `explicit_widening` | vendor·third-party source를 hardcoding Rule에 포함 |
| `index.required_tier` | enum | `text` | `maximum_requirement` | ScanRun complete에 필요한 최소 tier; `text < syntax < semantic` |
| `index.max_tier` | enum | `semantic` | `most_restrictive` | 요청 가능한 최고 tier; `text < syntax < semantic` |
| `index.fallback_to_lower_tier` | boolean | `true` | `immutable` | 상위 tier unavailable·partial이면 실제 lower tier와 limitation을 반환 |
| `index.max_symbols` | integer | `5000000` | `minimum_limit` | 한 CodeIndexSnapshot의 symbol 상한 |
| `index.max_references` | integer | `20000000` | `minimum_limit` | 한 CodeIndexSnapshot의 definition·reference edge 상한 |
| `index.max_graph_edges` | integer | `25000000` | `minimum_limit` | project·package·contract·dependency graph 전체 edge 상한 |
| `index.cross_project_edges` | boolean | `true` | `false_wins` | 같은 ProjectCatalogSnapshot 안에서 증거가 있는 cross-project edge 해석 |
| `index_cache.enabled` | boolean | `true` | `false_wins` | content-addressed derived cache 사용; cache는 current truth가 아님 |
| `index_cache.max_total_bytes` | integer | `2147483648` | `minimum_limit` | 모든 project index cache의 byte 상한 |
| `index_cache.retention_days` | integer | `30` | `minimum_limit` | reference 없는 cache entry의 최대 보관 기간 |
| `index_cache.reuse_partial` | boolean | `false` | `false_wins` | partial partition을 current 결과로 재사용하지 않음 |
| `index_cache.store_source_bytes` | boolean | `false` | `immutable` | source file 전체 복사본을 cache에 저장하지 않음 |

이 section의 추가 merge 전략은 다음처럼 고정한다.

- `maximum_floor`: 보존 최소값 중 가장 큰 값을 선택한다.
- `false_wins`: 하나라도 false면 false다.
- `true_wins`: 하나라도 true면 true다.
- `explicit_widening`: false→true는 새 Permission scope와 expected config fingerprint가 있을 때만 허용한다.
- `intersection_scope`: 각 source가 허용한 project-relative scope의 교집합이며 빈 교집합은 오류다.
- `union_local_roots`: user config와 해당 CLI 호출의 root binding 합집합이다. Project config·Catalog·MCP 입력은 새 local root를 추가할 수 없다.
- `append_unique_by_id`: source 우선순위 순으로 rule을 이어 붙이되 같은 ID·같은 fingerprint만 중복 제거하고 다른 fingerprint는 `CONFIG_CLASSIFICATION_RULE_CONFLICT`다.
- `subtract_optional`: 선택 항목에서만 빼며 built-in required ID가 들어오면 `CONFIG_REQUIRED_RULE_REMOVAL`이다.
- ordered `most_restrictive`: integrity는 `full > quick`, binary는 `skip > metadata_only`, Rule 오류는 `fail_scan > mark_incomplete` 순서의 강한 제한을, `index.max_tier`는 `text < syntax < semantic` 중 가장 낮은 허용 상한을 선택한다.
- `maximum_requirement`: 각 source가 요구한 index tier 중 가장 높은 값을 선택한다.

하위 source가 보존 기간·개수를 줄여 상위 hold 정책을 약화할 수 없다.

discovery·scan·index·cache 수치 기본값은 무제한 실행을 막는 초기 safety cap이지 처리 성능 SLO나 지원 규모 보장이 아니다. 실제 대형 Git/non-Git corpus에서 peak memory·disk·duration을 측정해 release 전에 조정한다. cap에 도달하면 범위를 임의 절단해 complete로 만들지 않고 해당 snapshot·partition을 `partial` 또는 ScanRun을 `incomplete`로 표시한다.

`management.suppression_default_expiry_days`는 1~365 범위다. `permanent=true`는 이 값을 우회하는 암묵적 무기한이 아니라 별도 justification과 `local_write` 또는 source PatchSet permission을 요구한다. `management.baseline_activation`에 자동 생성 모드는 존재하지 않는다.

store topology는 설정이 아니다. v1은 global store와 ProjectId별 project store를 사용하는 `hybrid`로 고정하며 `management.store_topology`, global/project DB path와 root-locator protection을 사용자 key로 노출하지 않는다. root locator는 Windows current-user protection을 사용하고 DB backup·export에서 제외한다.

`project_discovery.exclude_paths_add`의 built-in set v1은 slash-normalized `**/.git/**`, `**/.ai-runs/**`, `**/node_modules/**`, `**/target/**`, `**/.venv/**`, `**/vendor/**`, `**/dist/**`, `**/build/**`, `**/.cache/**`, `**/coverage/**`다. 이 set은 **nested project discovery recursion만** 막는다. explicit discovery root로 직접 지정하면 그 root 자체는 검사하며, project source inventory는 `scan.exclude_paths_add`와 class 정책으로 별도 결정한다. resolved set과 built-in set version은 discovery fingerprint에 들어간다.

`SourceClassificationRule`은 `rule_id`, project-relative `path_glob`, optional marker·VCS 상태 조건, 하나의 primary class, facet set, optional `generated_by`, optional `analysis_eligibility` map(`inventory|text|syntax|semantic|hardcoding`), 근거와 declaration fingerprint를 가진다. 절대 경로·shell expression·source literal은 허용하지 않는다. `generated_by`는 확인 가능한 manifest·generator ID이지 자유 형식 command가 아니다. 기본 class matrix보다 분석을 넓히는 `analysis_eligibility=true`는 explicit widening permission과 expected config fingerprint가 필요하다. immutable deny, `sensitive_candidate`, cache/output의 content 분석과 vendor의 hardcoding warning은 override할 수 없다. 서로 같은 우선순위 근거가 다른 primary class나 eligibility를 요구하면 임의로 하나를 고르지 않고 source를 `classification_conflict`로 표시한다.

effective analysis eligibility는 전용 계약의 class matrix에서 시작한다. `scan.hardcoding_include_*`는 해당 class/facet 전체를 넓히고, SourceClassificationRule의 `analysis_eligibility`는 matching path만 좁히거나 명시적으로 넓힌다. 상위 설정의 `false` 제한과 immutable deny가 가장 우선하며, 같은 우선순위의 true/false 충돌은 `CONFIG_CLASSIFICATION_RULE_CONFLICT`다. 최종 class·facet·eligibility와 각 근거를 SourceEntry에 저장한다.

`project_discovery.exclude_paths_add`는 discovery recursion만 제한하고 explicit root 자체를 무효화하지 않는다. `scan.exclude_paths_add`는 source inventory 범위다. Git tracked file은 Git ignore만으로 제거하지 않으며, ignored untracked file은 `scan.include_ignored=false`일 때 count와 provenance만 남긴다. `generated`, `vendor`, `cache`, `output`은 단순 path glob만으로 확정하지 않고 manifest·VCS 상태·adapter marker를 함께 기록한다. hardcoding widening key를 켜도 해당 class·facet이 normal source로 바뀌지는 않는다.

최초 attached checkout에 complete snapshot이 없으면 `scan.incremental=true`여도 `full` plan을 만든다. 그 뒤에는 Git revision, current status와 file content hash로 incremental 후보를 계산하며 manifest·lockfile·toolchain·AGENTS·canonical docs·classification·adapter fingerprint가 넓게 바뀌어 안전한 영향 범위를 계산할 수 없으면 full로 승격한다. 이 결정은 command 결과에 `requested_mode`, `effective_mode`, `promotion_reason`으로 남긴다.

`index.required_tier`는 `index.max_tier`보다 높을 수 없으며 위반은 `CONFIG_INDEX_TIER_RANGE`다. 기본 `required_tier=text`, `max_tier=semantic`은 eligible source의 text가 complete 조건이고 available syntax·semantic은 optional partition으로 시도한다는 뜻이다. required tier adapter·coverage가 실패하면 ScanRun은 `incomplete`; required보다 높은 tier가 unavailable이면 ScanRun은 성공할 수 있지만 그 partition·query에는 limitation과 fallback actual tier가 남는다. `max_tier=semantic` 자체는 semantic 결과를 보장하지 않는다. 해당 언어 adapter·toolchain·workspace 조건이 없으면 `semantic_unavailable`을 남기고 syntax, 다시 text로 내려간다. 1단계 index adapter는 generic process payload를 받지 않으며 package install, dependency resolution, build output 생성과 network access를 요구할 수 없다.

다음 이름은 v1 StarConfig key가 아니며 나타나면 unknown key 오류다.

- `management.backend`
- `management.database_path`
- `management.connection_string`
- `management.journal_mode`
- `management.store_topology`
- `management.global_database_path`
- `management.project_database_path`
- backend별 pragma·pool·vacuum 설정
- `project_discovery.schedule`, `project_discovery.cron`, `project_discovery.interval`
- `scan.schedule`, `scan.cron`, `scan.interval`, `scan.on_change`, `scan.watcher`
- `scan.auto_fix`, `scan.write_source`, `scan.apply_finding`
- `index.backend`, `index.database_path`, `index.parser`, `index.indexer`
- `index.ai`, `index.semantic_ai`, `index.model`

P0에서 선택한 embedded relational backend와 그 build option은 `star-state` private adapter와 release build 설정에만 속한다. CLI, project config와 MCP 입력으로 backend를 선택하거나 SQL·pragma를 전달하지 않는다. concrete 선택 근거는 [ADR-0008](../decisions/ADR-0008-P0-embedded-relational-backend.md)에만 둔다.

fingerprint는 다음처럼 분리한다.

| fingerprint | 포함 | 제외 |
|---|---|---|
| `discovery_config_fingerprint` | protected root binding ID set, nested·worktree·workspace·non-Git policy, recursion·ignore·symlink limit, marker contract version | raw root path, 표시 이름, timestamp |
| `classification_fingerprint` | ordered SourceClassificationRule, built-in marker contract, VCS·manifest evidence policy | source byte 자체, cache hit, render option |
| `scan_config_fingerprint` | resolved path scope, ignored·binary·hardcoding 대상 policy, byte·file limit, completeness, redaction contract, Rule ID·version·definition·parameter fingerprint | retention, terminal render, cache eviction |
| `index_config_fingerprint` | required/max tier, fallback contract version, symbol·reference·graph limit, cross-project policy와 adapter set fingerprint | cache location·hit, timestamp, backend·table |

모든 값은 JCS로 hash한다. ScanRun에는 전체 `EffectiveConfig.fingerprint`, scan·index·classification fingerprint를, ProjectCatalogSnapshot에는 discovery fingerprint를 기록한다. persisted fingerprint payload에는 raw root path를 넣지 않는다. cache key는 snapshot과 index fingerprint를 사용하지만 cache budget·retention 변화만으로 semantic snapshot을 stale로 만들지 않는다.

`management.*`와 scan resource·scope key는 사용자·project 설정에서 더 제한할 수 있다. Goal·CLI·MCP override가 scope나 resource를 넓히려면 일반 override가 아니라 새 Permission scope와 expected config fingerprint를 요구한다.

### `[change_planning]`

이 section은 2단계 [변경 계획·영향 분석](change-planning-and-impact.md)의 **목표 설정 계약**이다. 현재 StarConfig parser·Schema·제품 code에 구현됐다는 뜻이 아니다. 구현 전에는 unknown key로 거부하고, 구현 시 version·invalid/unknown fixture와 EffectiveConfig golden을 함께 추가한다.

| key | type | 기본값 | 병합 | 의미 |
|---|---|---|---|---|
| `require_current_inputs` | boolean | `true` | `true_wins` | stale·unverified Catalog/Index를 ready plan의 confirmed 근거로 금지 |
| `max_graph_depth` | integer | `8` | `minimum_limit` | seed별 허용 relation 수 상한 |
| `max_graph_nodes` | integer | `100000` | `minimum_limit` | 한 ImpactAnalysis의 unique node 상한 |
| `max_graph_edges` | integer | `500000` | `minimum_limit` | 한 ImpactAnalysis의 evidence edge 상한 |
| `max_downstream_projects` | integer | `64` | `minimum_limit` | cross-project read-only closure 상한 |
| `max_check_candidates` | integer | `2048` | `minimum_limit` | 한 ValidationPlan에서 조사할 candidate 상한 |
| `allow_cross_project_read` | boolean | `true` | `false_wins` | 등록된 target relation의 read-only 영향 조회 허용 |
| `allow_previous_success_reuse` | boolean | `true` | `false_wins` | descriptor가 허용한 exact compatible evidence 재사용 |
| `require_user_acceptance_for_change_scope_expansion` | boolean | `true` | `immutable` | planned change scope 자동 확대 금지 |

package→workspace→project full promotion order, `not_found != not_applicable`, user override provenance와 **2단계 change-planning 계층의 cross-repo source write 금지**는 설정으로 약화할 수 없는 product invariant다. 실제 여러 Project effect는 이 설정을 켜서 우회하지 않고 [9단계 ChangeBundle](cross-repo-change-bundle.md)이 project-local PatchSet·승인·Gate를 조정한다. resource limit에 도달하면 결과를 complete로 자르지 않고 frontier limitation과 fallback을 만든다.

`planning.ai`, `planning.model`, `change_planning.ai`, `change_planning.prompt`, `change_planning.auto_apply`, `change_planning.cross_repo_write`, `change_planning.auto_merge`는 v1 key가 아니며 나타나면 unknown key 오류다.

`planning_config_fingerprint`는 위 resolved 값, relation policy contract version, affected fallback contract version과 required RiskPath descriptor set fingerprint를 포함한다. TaskSpec·ScopeRevision·source snapshot은 config가 아니므로 제외하고 ImpactAnalysis input fingerprint에서 별도로 결합한다.

`validation_config_fingerprint`는 resolved `validation.*`, 적용 Profile의 gate/rule/check/baseline/suppression/stability/review metadata fingerprint, GatePolicyDescriptor set과 validation contract version을 포함한다. source·WorkspaceSnapshot·ValidationPlan·Tool executable identity는 config가 아니므로 [EvidenceSubjectBinding](validation-and-evidence.md#evidence-subject-binding)에서 별도로 결합한다. log render option, terminal color와 evidence retention 기간만 바뀐 경우 validation 의미를 stale로 만들지 않는다.

### `[catalog]`, `[tool_registry]`, `[mcp_gateway]`, `[logging]`, `[ipc]`

| section.key | type | 기본값 | 설명 |
|---|---|---|---|
| `catalog.user_roots` | path array | 사용자 Catalog 폴더 | 신뢰한 local descriptor 위치 |
| `catalog.project_enabled` | boolean | `true` | `.star-control\catalog` 읽기 |
| `catalog.require_trust` | boolean | `true` | 새 실행 descriptor 첫 사용 확인 |
| `tool_registry.enabled` | boolean | `true` | 외부 Tool Registry 사용 |
| `tool_registry.user_root` | path | `%APPDATA%\Star-Control\tools.d` | 사용자가 관리하는 manifest 위치 |
| `tool_registry.locations` | ID→path map | `{}` | 사용자 설정 전용, 최대 64개. 공유 manifest의 stable location_ref를 현재 PC absolute path로 해석 |
| `tool_registry.project_enabled` | boolean | `true` | 프로젝트 `tools.d` 발견. trust 전에는 비활성 |
| `tool_registry.project_trust` | enum | `explicit` | project manifest는 사용자 trust store 필요 |
| `tool_registry.user_trust` | enum | `policy_profile` | safe_default는 첫 trust, personal_auto는 관리 root 저장을 등록 의도로 사용 |
| `tool_registry.allow_path_lookup` | boolean | `false` | PATH의 첫 EXE를 자동 선택하지 않음 |
| `tool_registry.allowed_process_protocols` | enum set | `star_json_stdio_v1,argv_v1` | 상위 제한과 `intersection` |
| `tool_registry.allowed_isolation_profiles` | enum set | `appcontainer_adapter,trusted_desktop` | 운영체제 capability와 정책의 교집합 |
| `tool_registry.default_isolation` | enum | `policy_profile` | compatible broker adapter는 `appcontainer_adapter`, 나머지는 `trusted_desktop` |
| `tool_registry.require_trusted_desktop_code_trust` | boolean | `true` | 현재 사용자 token 실행 전 code trust 확인 |
| `tool_registry.live_reload` | boolean | `true` | MCP 연결을 유지한 채 Registry 갱신 |
| `tool_registry.watch_files` | boolean | `true` | manifest·Schema·EXE 경로 변경 감시 |
| `tool_registry.demand_scan` | boolean | `true` | search·describe·invoke 직전에 변경 누락 보완 |
| `tool_registry.reload_debounce_ms` | integer | `250` | 여러 저장 event를 한 candidate로 묶는 시간 |
| `tool_registry.stable_file_window_ms` | integer | `250` | 쓰는 중인 파일을 읽지 않기 위한 안정 구간 |
| `tool_registry.stable_file_timeout_ms` | integer | `5000` | candidate가 안정되기를 기다리는 최대 시간 |
| `tool_registry.persist_last_known_good` | boolean | `true` | invalid 편집·Controller 재시작 뒤 package 정상본 복구 |
| `tool_registry.user_default_update_policy` | enum | `pinned_hash` | 안전 기본값. 서명된 compatible 또는 명시적 follow_path만 opt-in |
| `tool_registry.allow_follow_path_user` | boolean | `true` | user source에서만 `follow_path` 선택 허용 |
| `tool_registry.project_update_policy` | enum | `pinned_hash` | project source는 실행 파일 hash 고정 필수 |
| `tool_registry.verify_executable_identity_each_call` | boolean | `true` | 실행 직전 path·file identity·hash 재확인 |
| `tool_registry.max_packages` | integer | `128` | resource 사용 상한 |
| `tool_registry.max_tools` | integer | `512` | active·probe pending·unavailable을 합친 검색 가능한 action 수 상한 |
| `tool_registry.max_actions_per_package` | integer | `64` | 한 package action 상한 |
| `tool_registry.max_watch_roots` | integer | `128` | unique final directory watcher 상한 |
| `tool_registry.max_manifest_bytes` | integer | `1048576` | package TOML 크기 상한 |
| `tool_registry.max_schema_bytes` | integer | `4194304` | package가 참조하는 Schema 총크기 상한 |
| `tool_registry.max_action_schema_bytes` | integer | `1048576` | action 하나의 fully resolved input+output Schema 상한 |
| `tool_registry.max_schema_depth` | integer | `64` | 중첩·local reference 해석 깊이 상한 |
| `tool_registry.invalid_optional_package` | enum | `keep_last_known_good` | 오류 package만 이전 정상본 유지하고 진단 |
| `mcp_gateway.contract_version` | integer | `1` | 고정 surface·hash·상태기계 version |
| `mcp_gateway.max_message_bytes` | integer | `8388608` | MCP JSON-RPC physical line 상한 |
| `mcp_gateway.sync_budget_ms` | integer | `30000` | tools/call이 결과를 기다리는 최대 시간 |
| `mcp_gateway.accepted_dispatch_ms` | integer | `5000` | OperationId를 반환할 접수 한도 |
| `mcp_gateway.progress_per_second` | integer | `4` | request progress rate limit |
| `logging.level` | enum | `info` | `error`, `warning`, `info`, `debug`, `trace` |
| `logging.include_raw_output` | boolean | `false` | 원문은 ArtifactRef로 분리 |
| `ipc.connect_timeout_ms` | integer | `5000` | named pipe 연결 한도 |
| `ipc.max_frame_bytes` | integer | `8388608` | IPC frame 상한 |
| `ipc.auth_required` | boolean | `true` | DPAPI per-user key HMAC handshake 필수 |

MCP·Tool Registry 설정의 source·병합 규칙은 다음으로 동결한다.

- `tool_registry.user_root`, `tool_registry.locations`, `tool_registry.user_trust`, `tool_registry.allow_follow_path_user`는 사용자 설정에서만 선언할 수 있다. project·Goal·MCP·CLI override에서 나타나면 오류다.
- `tool_registry.allow_path_lookup=false`, `tool_registry.live_reload=true`, `tool_registry.demand_scan=true`, `tool_registry.verify_executable_identity_each_call=true`, `tool_registry.project_update_policy=pinned_hash`, `mcp_gateway.contract_version=1`, `mcp_gateway.max_message_bytes=8388608`, `mcp_gateway.sync_budget_ms=30000`, `mcp_gateway.accepted_dispatch_ms=5000`, `mcp_gateway.progress_per_second=4`, `ipc.connect_timeout_ms=5000`, `ipc.max_frame_bytes=8388608`, `ipc.auth_required=true`는 v1 불변값이다. Gateway는 TOML을 읽지 않고 IPC v1에는 이 값을 협상하는 payload가 없으므로 다른 값은 받아 놓고 무시하지 않고 설정 오류로 거부한다.
- `tool_registry.watch_files`만 진단 목적으로 false로 낮출 수 있다. 이 경우에도 request 전 demand scan은 유지된다.
- demand scan은 이미 관찰 중인 package TOML과 같은-path 교체를 우선 보존하고, 그 밖의 새 TOML은 source 순서와 정렬된 path 순서로 전역 `tool_registry.max_packages` 범위 안에서만 읽는다. 초과 파일은 실행 후보로 읽지 않고 해당 root에 `TOOL_REGISTRY_LIMIT` 진단을 남긴다. 따라서 invalid·미신뢰 파일을 대량 배치해도 기존 active·candidate·last-known-good 확인과 요청 처리가 고갈되지 않는다.
- v1 release catalog allowlist는 Controller build에 포함된 `catalog/tool-packages/star-control-core.toml`의 파일명과 raw SHA-256 하나다. checksum이 다르거나 allowlist에 없는 release TOML은 `TOOL_INTEGRITY_INVALID`로 거부하고 기존 last-known-good를 유지한다. release package set 변경은 검증된 Controller release와 함께 이루어지며 user·project package의 live 등록에는 영향을 주지 않는다.
- `allowed_process_protocols`와 `allowed_isolation_profiles`는 `intersection`, 각 `max_*`와 timeout·byte limit은 `minimum_limit`이다. 하위 source는 범위를 넓힐 수 없다.
- `tool_registry.locations` key는 package-local ID 형식이고 path는 local fixed volume의 absolute directory다. 값은 trust scope에 들어가며 project가 같은 이름으로 바꿀 수 없다.

## SecretRef

설정과 Catalog에는 secret 원문을 넣지 않는다. 허용 형식은 다음 두 가지다.

- `env:NAME`
- `windows-credential:TARGET_NAME`

SecretRef는 값의 존재와 사용 결과만 기록한다. 진단, fingerprint, event, evidence와 debug log에 실제 값을 넣지 않는다. 프로젝트 설정이 사용자 credential 이름을 바꾸는 것은 가능하지만 secret을 더 낮은 신뢰 source에 복사하지 않는다.

## Catalog 구조

Catalog는 실행 logic이 아니라 기계가 읽는 선언이다. built-in Catalog는 release와 함께 읽기 전용으로 설치하고, 사용자와 프로젝트 Catalog는 명시적으로 신뢰한 뒤 합친다.

공통 descriptor 필드는 다음과 같다.

| 필드 | 의미 |
|---|---|
| `catalog_id` | namespace를 포함한 stable ID. 예: `star.task.rust-test` |
| `format_version` | descriptor 형식의 positive integer version |
| `item_version` | 항목 자체의 SemVer |
| `display_name` | 사용자 표시 이름 |
| `description` | 해결하는 문제와 사용 조건 |
| `platforms` | 현재는 `windows`만 허용 |
| `requires` | 다른 descriptor와 capability reference |
| `replaces` | 의도적으로 대체하는 ID와 호환 범위 |
| `source` | built-in, user, project와 origin |
| `extensions` | 허용 namespace의 추가 metadata |
| `lifecycle` | Rule·Check·Profile·ChangeRecipe에만 쓰는 versioned lifecycle object |

같은 ID와 version의 내용이 다르면 우선순위로 덮지 않고 충돌로 중단한다. 대체는 `replaces`가 명시되고 참조 compatibility가 검증될 때만 허용한다.

Rule·Check·Profile·ChangeRecipe의 `lifecycle`은 `state=active|deprecated|retired|rejected`, optional `replaced_by`, `deprecated_at`, `support_until`, `migration_guide_ref`, `last_evaluation_ref`와 사유 code를 가진다. `deprecated`는 기존 exact reference를 읽고 bounded migration할 수 있지만 새 기본 선택에서 제외한다. `retired`는 historical evidence·migration 입력으로만 읽으며 새 plan에서 해석하지 않는다. `rejected` candidate는 active Catalog에 publish하지 않는다. `replaced_by`는 compatible descriptor와 migration evidence가 없으면 설정할 수 없고 ID·version을 재사용하지 않는다. 새 descriptor의 공통 `replaces`와 이전 descriptor의 `lifecycle.replaced_by`는 같은 ID/version pair를 가리켜야 하며 한쪽만 있으면 load를 거부한다. lifecycle source를 EvaluationRun이나 DB projection이 직접 수정하지 않으며 검토된 Catalog source change만 writer다.

### TaskDescriptor

TaskDescriptor는 사용자가 선택한 task kind를 stage·impact seed·검사 family로 연결하는 `star.task-descriptor` 선언이다. 실행 logic이나 자연어 prompt를 넣지 않는다.

| 필드 | 의미 |
|---|---|
| `task_kind` | stable change/task 분류 |
| `input_schema_ref`, `output_schema_refs` | typed TaskSpec input과 기대 결과 계약 |
| `applicability` | Project/source class·language·manifest capability 조건 |
| `default_stage_mode`, `executor_kind`, `completion_criteria_template` | 기본 Stage 성격, `deterministic_local` 또는 `codex` 실행자와 완료 조건 |
| `required_context_kinds` | 필요한 Catalog·Index·guidance·source snapshot |
| `impact_seed_rules` | TaskSpec selector를 entity seed로 바꾸는 선언 |
| `default_risk_path_refs` | 항상 평가할 RiskPathDescriptor ID/version |
| `required_check_families`, `optional_check_families` | affected selector의 초기 family |
| `no_result_policy` | seed·test·descriptor 0건의 `block`, `review`, `allow_confirmed_empty` 처리 |
| `route_hints`, `permission_actions` | 이후 stage route·action 요구. 권한을 부여하지는 않음 |
| `required_evidence_kinds` | 실행·완료 뒤 필요한 evidence |
| `retry_contract`, `idempotency_contract` | 재시도 가능한 실패와 중복 처리 |

`impact_seed_rules`는 entity kind·selector field·required resolution과 fallback reason만 선언한다. raw query, regex script, SQL과 AI prompt를 넣지 않는다. TaskSpec의 사용자 include/exclude를 descriptor가 확대·삭제할 수 없다.

### ToolDescriptor

- stable ToolId, 검색용 이름·설명·tag·capability와 input·output Schema
- 고정 MCP risk lane과 read·destructive·open-world·idempotency 성격
- executable identity, `pinned_hash | version_compatible | follow_path` update policy와 지원 protocol
- 구조화된 argument binding, cwd·환경·timeout·출력 상한
- stdin·stdout·stderr 형식과 exit code 의미
- progress·취소·동시성·lock과 retryable failure 표시. 외부 EXE 자동 retry는 v1에서 하지 않음
- secret 요구, redaction, side effect·비용 성격과 Permission ActionId set
- `effect_class=none|derived_state_write|target_write|system_write|external_write`, `probe_class=none|read_only_introspection`, network/cache/package mutation 선언

외부 EXE package의 정확한 manifest, protocol, trust와 reload 규칙은 [외부 Tool Registry](external-tool-registry.md)가 소유한다. shell 한 줄 문자열, 임의 `cmd /c`, PowerShell script text를 persisted 실행 계약으로 저장하지 않는다. 복잡한 도구는 별도 adapter EXE로 `star_json_stdio_v1`을 구현한다.

문서 command와 doctor probe는 `probe_class=read_only_introspection`, `effect_class=none|derived_state_write`, `network=false`, `package_mutation=false`, `system_mutation=false`인 exact descriptor만 사용할 수 있다. executable identity·argument binding·cwd·environment allowlist·exit/output Schema가 모두 일치하지 않으면 실행하지 않는다. help command라는 이름이나 `--version` option만으로 read-only라고 추정하지 않는다.

### CheckDescriptor

CheckDescriptor는 affected selector가 검사를 발견·범위 bind·재사용·승격할 수 있게 하는 `star.check-descriptor` 선언이다.

| 필드 | 의미 |
|---|---|
| `check_family`, `tags` | test, build, lint, docs, contract, security 등 stable 분류 |
| `applicability` | task kind, source class/facet, entity·ImpactEdge relation, risk path selector의 typed expression |
| `tool_ref`, `invocation_template` | trusted ToolDescriptor와 shell 재해석 없는 argument binding |
| `coverage_unit` | `file`, `package`, `workspace`, `project`, `multi_project` |
| `scope_binding` | package/workspace/project key를 cwd·argument에 bind하는 typed rule |
| `soundness_preconditions` | 좁은 scope가 충분하려면 필요한 graph relation·freshness·coverage |
| `promotion_chain` | package용 Check에서 workspace·project full Check ref로 가는 순서 |
| `check_dependencies` | 선행 Check family/ID와 `requires`, `provides_input`, `must_run_after` relation |
| `invalidates_on` | manifest·lockfile·toolchain·config·source class·risk 변화 selector |
| `always_run_for` | cache/reuse로 생략할 수 없는 risk path·phase |
| `result_parser`, `diagnostic_mapping` | exit/output을 공통 결과로 정규화 |
| `diagnostic_rule_refs` | 생산 가능한 stable Rule ID·version·definition/fingerprint contract |
| `timeout_ms`, `retry_contract` | 실행 한도와 허용 retry |
| `stability_contract` | `single_attempt\|repeat_on_failure\|sampled`, minimum comparable attempts와 max attempts |
| `cache_contract` | deterministic 여부, key input, previous success reuse 조건 |
| `result_freshness_contract` | external DB/advisory/tool data identity, timestamp source, maximum age와 missing timestamp 처리 |
| `ratchet_eligible` | raw fail을 existing unchanged debt로 Gate 만족시킬 수 있는지. 기본 false |
| `protected_invariants` | baseline·suppression·waiver로 완화할 수 없는 failure class |
| `fixture_manifest_refs` | Rule/parser 변경에 필요한 positive·negative·edge·regression case manifest |
| `test_trust_policy_ref` | test family일 때 assertion/skip/focus adapter와 snapshot mass threshold contract |
| `side_effects`, `permission_actions` | source·external effect와 필요한 action |
| `gate_default`, `evidence_kinds` | required/optional 기본값과 결과 evidence |

`applicability=false`를 계산하려면 expression의 모든 required input이 current·complete해야 한다. descriptor가 없거나 expression input이 unknown이면 `not_applicable`이 아니라 `not_found|unknown`이다.

package Check가 `promotion_chain` 또는 soundness precondition을 선언하지 않으면 package affected 선택에 사용할 수 없고 workspace 또는 project full descriptor를 찾는다. full descriptor도 없으면 실행 command를 추측하지 않고 ValidationPlan을 unresolved로 둔다.

`ratchet_eligible=true`는 diagnostics-based static Check에만 허용한다. functional test, build·compile, regression pair, validator guard, secret critical, migration invariant와 release artifact identity family는 built-in false floor이며 project replacement가 true로 바꿀 수 없다. launch error, timeout, parser failure, output truncation, partial·stale·flaky result도 ratchet 대상이 아니다.

`diagnostic_mapping`은 external code→RuleRef, severity·confidence, location parser, stable fingerprint key와 remediation mapping을 typed data로 선언한다. unknown external error를 drop하거나 success로 매핑할 수 없다. mapping 실패는 `VALIDATION_DIAGNOSTIC_MAPPING_FAILED`와 unverified completeness다.

`TestTrustPolicy`는 framework adapter ref, test/case/assertion identity contract, skip·ignore·focus marker mapping, timeout/retry field mapping과 snapshot classification을 가진다. built-in snapshot review 기본 threshold는 `changed_files >= 5`, `changed_items >= 100`, `changed_after_bytes >= 1048576`, `changed_ratio >= 0.25` 중 하나다. project/Profile은 threshold를 낮출 수 있고, 올리거나 classifier scope를 줄이는 변경은 validator guard의 policy weakening 대상이다.

`stability_contract=single_attempt`은 complete/current한 started attempt 하나를 stable로 볼 수 있는 기본값이지만 같은 attempt group에 반대 pass/fail history가 있으면 항상 flaky다. `repeat_on_failure`는 첫 실패 뒤 descriptor가 선언한 idempotent retry만 비교하고, `sampled`는 `minimum_comparable_attempts >= 2`를 요구한다. minimum을 낮추거나 max를 올려 마지막 성공 가능성만 키우는 변경, 비교 group key field를 제거하는 변경은 validator guard의 `star.validation.guard.failure-masked` 대상이다.

### ExternalDataSourceDescriptor

`star.external-data-source-descriptor`는 vulnerability·license·available-version 자료의 provenance와 freshness policy를 선언한다. 외부 DB 내용을 복제하거나 network client를 정의하는 계약이 아니다.

| 필드 | 의미 |
|---|---|
| `source_kind` | `vulnerability\|license\|package_version\|workflow_action\|release_provenance` |
| `provider`, `official_url` | 공급자 identity와 공식 문서/endpoint |
| `schema_or_api_version` | adapter가 해석하는 source version |
| `ecosystems` | 지원 package ecosystem set |
| `query_identity_contract` | package/version/advisory query를 canonicalize하는 typed field |
| `published_time_field`, `modified_time_field` | source가 제공하는 시간 위치; 없으면 명시적 `none` |
| `maximum_age` | Gate에서 허용하는 최대 나이 |
| `missing_time_policy` | 기본 `unknown`; fetch 시각만으로 current 확정 금지 |
| `coverage_contract` | pagination, aliases, withdrawn record, affected range와 누락 표현 |
| `adapter_tool_ref` | trusted ToolDescriptor. process 실행은 기존 Tool executor만 수행 |
| `network_action` | offline cache 또는 승인된 `network_read` |
| `redaction_policy` | credential·query private data·response의 persisted 경계 |

Catalog load는 `maximum_age` 누락, unknown을 current로 만드는 policy, untrusted ToolDescriptor 또는 network effect 누락을 거부한다. runtime `ExternalDataSnapshot`은 descriptor ID/version/fingerprint를 evidence로 가진다.

### PackageManagerAdapterDescriptor

`star.package-manager-adapter-descriptor`는 ecosystem별 manifest·lockfile 소유권과 typed operation을 선언한다. resolver나 updater를 Star-Control core에 넣는 계약이 아니다.

| 필드 | 의미 |
|---|---|
| `ecosystem`, `manifest_kinds`, `lockfile_kinds` | manager가 소유하는 file format |
| `tool_ref` | trusted package manager ToolDescriptor |
| `operations` | `inspect\|locked_verify\|prepare_update\|prepare_add\|prepare_remove\|restore_previous` typed operation |
| `argument_schema_refs` | string shell이 아닌 operation별 structured args |
| `offline_capability` | offline/locked/frozen 기능과 실제 effect declaration |
| `network_and_cache_effects` | operation별 network, download, cache write |
| `expected_write_scope` | isolated worktree에서 허용하는 manifest·lockfile·generated metadata |
| `lockfile_generation_contract` | manager가 생성·갱신하고 core 직접 편집 금지 |
| `before_artifact_requirement` | 변경 전 manifest·lockfile 보존 |
| `rollback_operation` | reverse PatchSet 뒤 manager 검증 또는 restore operation |
| `diagnostic_mapping` | manager exit/output을 common Rule로 정규화 |

`prepare_update`라는 이름은 자동 승인이 아니다. operation effect에 `network_read|network_download|dependency_change`가 있으면 각각 PermissionDecision이 필요하며, 출력은 isolated actual diff와 PatchSet으로만 승격된다.

### Validator Registry

Validator Registry는 Rule·CheckDescriptor·GatePolicy·Diagnostic mapping을 실행 시점에 해석한 read-only index다. **새로운 세 번째 executor descriptor가 아니다.** source 관찰은 Rule의 `analyzer_ref`, process 검사는 CheckDescriptor의 ToolDescriptor를 사용하고 normalizer·Gate meta Diagnostic은 등록된 built-in producer ref를 사용한다.

`ValidatorRegistrySnapshot`은 다음을 가진다.

| 필드 | 의미 |
|---|---|
| `validator_registry_snapshot_id` | immutable snapshot ID |
| `catalog_snapshot_ref` | 해석한 Catalog ID·revision·hash |
| `entries` | Rule ID byte-order로 정렬한 ValidatorEntry |
| `gate_policy_refs` | phase/Profile별 resolved GatePolicyDescriptor |
| `required_builtin_set_fingerprint` | 제거·약화할 수 없는 built-in Rule/Check floor |
| `guard_minimum_manifest_ref` | release에 포함된 protected ID·floor·fixture manifest와 fingerprint |
| `trusted_predecessor_ref` | validator protected change 시 필수인 pre-change 또는 last-known-good Registry/producer identity. candidate 자신 금지 |
| `resolution_diagnostics` | conflict·invalid fixture·missing Tool·mapping 문제 |
| `snapshot_fingerprint` | 모든 실행 의미와 resolution 결과의 JCS SHA-256 |
| `created_at` | 표시·audit 시각, identity에서는 제외 |

`ValidatorEntry`는 다음 의미를 결합한다.

- RuleRef: Rule ID·SemVer·definition fingerprint·fingerprint contract version
- owner 기능 `B01`~`B09`와 owner module
- producer kind `built_in_analyzer|external_check_mapping|normalizer|gate_evaluator`와 exact producer ref
- 정렬된 `producer_bindings`: analyzer 또는 CheckDescriptor·ToolDescriptor·normalizer/Gate producer ref와 qualified external code namespace
- applicability·required evidence tier·source class/facet
- default severity·confidence와 protected minimum floor
- Diagnostic location/evidence/remediation mapping contract
- `ratchet_eligible`, baseline comparison key와 suppression selector capability
- fixture manifest ref와 required `positive|negative|edge|regression|adversarial` set
- deterministic flag, input/output limit, source·trust provenance

Registry resolution 순서는 다음과 같다.

1. release built-in required Rule·Check·Gate floor를 읽는다.
2. user/project Catalog의 additive validator를 trust·Schema·reference 검증한다.
3. explicit `replaces`가 있으면 compatibility와 floor 강화를 검사한다.
4. 같은 Rule ID/version의 다른 definition·fingerprint contract, 또는 같은 `(ToolDescriptor, external code)`가 여러 Rule로 가는 mapping은 conflict다. 같은 Rule을 여러 producer가 관찰하는 것은 허용한다.
5. required built-in 삭제·disable, severity/confidence floor 하향, applicability 축소, allowlist 확대, fixture 제거와 ratchet 금지 family 완화는 candidate 전체를 거부한다.
6. ToolDescriptor가 unavailable이어도 entry는 searchable 상태로 남고 required Check는 `not_found`가 아니라 `unavailable`로 구분한다.
7. resolution diagnostic까지 포함한 snapshot fingerprint를 publish한다.

pre-change trusted snapshot과 current candidate snapshot은 validator guard가 함께 사용한다. current 변경 validator의 self-test만으로 candidate를 active publish하지 않는다. last-known-good는 invalid 편집 뒤 조회·비교 근거일 뿐 current source와 다른 결과를 자동 Gate positive evidence로 만들지 않는다.

`GuardMinimumManifest`는 protected Rule/Check ID, minimum version·severity/confidence, applicability floor, required fixture kind, forbidden ratchet family와 manifest fingerprint만 가진 release-owned nested descriptor다. project/user Catalog는 이를 replace할 수 없다. `trusted_predecessor_ref`가 candidate Registry나 이번 변경에서 만든 producer를 가리키면 resolution을 거부한다.

Validator Registry 선언에는 raw shell, script text, source replacement, SQL, AI prompt와 외부 scanner DB를 넣지 않는다.

### GatePolicyDescriptor

GatePolicyDescriptor는 `catalog/validators/gates.toml`의 data-driven threshold와 review floor다. Gate 결정 알고리즘 자체를 script로 저장하지 않는다.

| 필드 | 의미 |
|---|---|
| `gate_policy_id`, `item_version`, `definition_fingerprint` | stable identity와 의미 version |
| `applies_to_phases` | `during_stage\|stage_exit\|goal_exit\|patch_pre_apply\|patch_post_apply\|merge\|release` |
| `applies_to_profiles`, `risk_floor` | Profile/risk selector |
| `fail_on` | new·worsened Diagnostic severity threshold |
| `baseline_mode` | `off\|report_only\|ratchet_new\|ratchet_new_and_worsened\|clean_only` |
| `allowed_run_satisfactions` | `clean_pass`와 허용 시 `ratchet_satisfied` |
| `suppression_policy` | selector precision·expiry·permanent approval floor |
| `flaky_policy` | family/risk별 `human_review\|block` |
| `review_policy_by_context` | CLI-only human, Codex-managed optional independent review |
| `protected_invariants` | 어떤 baseline·suppression·waiver로도 완화할 수 없는 reason code |
| `required_decision_evidence_kinds` | GateDecision 전에 commit돼야 할 current binding·ChangeSet·run·Diagnostic 요구 |
| `required_completion_artifacts` | decision 뒤 자동 완료 전에 만들 EvidenceBundle·ReviewPack·Handoff 요구 |

여러 Policy/Profile이 적용되면 required evidence·protected invariant·Check set은 union, severity/risk/scope floor는 가장 엄격한 값을 사용한다. project Catalog는 built-in `validator_guard`, secret redaction, out-of-scope change와 stale evidence floor를 낮출 수 없다.

`required_completion_artifacts`는 GateDecision input이 아니다. GateDecision → EvidenceBundle → ReviewPack 순서로 packaging하고, required completion artifact가 complete로 commit되지 않으면 `auto_pass` decision이 있어도 Run·Stage 자동 완료를 만들지 않는다.

### RiskPathDescriptor

RiskPathDescriptor는 영향 graph의 위험 경로와 최소 검사 범위를 선언하는 `star.risk-path-descriptor` Catalog item이다.

| 필드 | 의미 |
|---|---|
| `risk_path_id`, `item_version`, `definition_fingerprint` | `risk_path_id`는 공통 `catalog_id`와 같은 stable identity, 의미 version |
| `seed_selectors` | typed source class/facet·entity·contract·descriptor 기반 시작 조건. raw path/literal 단독 selector는 금지 |
| `edge_patterns` | 허용 relation sequence와 방향·최대 길이 |
| `required_evidence` | confirmed 판정에 필요한 tier·resolution·freshness |
| `severity_floor` | match 시 최소 risk level |
| `required_check_families` | ValidationPlan candidate로 만들 family |
| `fallback_scope_floor` | `package`, `workspace`, `project_full`, `affected_projects` |
| `always_run_check_families` | previous success로 생략할 수 없는 family |
| `exclusions` | generated/vendor/test 등 typed 예외와 필요한 evidence |
| `redaction_policy` | secret·개인 path·민감 literal 저장 금지 |
| `no_result_policy` | metadata 부재·partial·confirmed empty 처리 |

built-in required set은 다음 ID를 가진다.

| ID | 위험 경로 | 기본 floor |
|---|---|---|
| `star.risk.auth-secret` | auth·secret·permission·redaction | `workspace`; global policy면 `project_full` |
| `star.risk.public-api-schema` | public API·Contract·Schema·consumer | provider `workspace` + affected consumer |
| `star.risk.dependency-lockfile` | dependency manifest·lockfile·build graph | `workspace`; root lockfile이면 `project_full` |
| `star.risk.validator-policy` | Rule·Check·validator·gate policy | `workspace`; shared gate면 `project_full` |
| `star.risk.migration` | schema/config/store migration·rollback | 기본 `project_full` |
| `star.risk.workflow-release` | CI workflow·packaging·release | `project_full` |
| `star.risk.generated-source` | generator input·output·provenance | generator owner `workspace` |
| `star.risk.failure-regression` | 재발 failure·before/after·recovery | owning package + compatible regression Check |
| `star.risk.external-security-freshness` | advisory/license/version coverage·freshness | affected dependency graph + required source |

user/project Catalog는 새 risk path를 추가하거나 더 강한 check·scope floor로 대체할 수 있다. built-in required descriptor를 제거하거나 severity·fallback floor를 낮추는 replacement는 conflict로 거부한다. literal·path glob만으로 critical risk를 확정하지 않고 entity·ownership·relation evidence를 요구한다.

descriptor source mapping은 다음과 같다.

| descriptor | built-in source | project source |
|---|---|---|
| TaskDescriptor | `catalog/tools/task-kinds.toml` | `.star-control/tasks.toml`의 `[[tasks]]` |
| CheckDescriptor | `catalog/validators/registry.toml` | `.star-control/tasks.toml`의 `[[checks]]` |
| RiskPathDescriptor | `catalog/policies/risk-paths.toml` | `.star-control/risk-paths.toml`의 `[[risk_paths]]` |
| ExternalDataSourceDescriptor | `catalog/maintenance/external-data-sources.toml` | `.star-control/maintenance.toml`의 `[[external_data_sources]]` |
| PackageManagerAdapterDescriptor | `catalog/maintenance/package-managers.toml` | `.star-control/maintenance.toml`의 `[[package_managers]]` |

각 TOML item은 공통 descriptor field와 해당 Schema ID를 사용한다. project Check는 trusted ToolDescriptor를 reference해야 하며 executable path·shell command text를 직접 넣지 않는다. Project scan에서 발견한 manifest script·문서 command는 provenance가 있는 candidate일 뿐, 이 선언과 trust 검증을 통과하기 전에는 실행 가능한 CheckDescriptor가 아니다.

### Rule

Rule은 Finding과 공통 Diagnostic의 stable identity·version·fingerprint 의미를 소유하는 versioned 선언이다. P0 source Finding field와 M3 conditional Diagnostic field의 exact wire 계약은 [공통 개발 관리 계약의 M3 Rule v2](development-management.md#m3-rule-v2-target)가 소유한다.

- stable RuleId, SemVer와 definition fingerprint
- `rule_domain=scan_finding|validation_diagnostic|both`, 적용 language·source kind·Check family와 producer reference
- typed parameter Schema와 기본 severity·confidence
- line 이동에 흔들리지 않는 identity anchor와 fingerprint contract version
- message code, typed redaction parameter와 민감 값 저장 금지
- 적용 가능한 ChangeRecipe reference와 lifecycle
- Gate 기본 영향, ratchet 가능 여부와 완화 불가 protected invariant
- positive·negative·edge·regression, 필요 시 adversarial fixture manifest reference

Rule 실행 code, raw source literal과 DB query를 Catalog에 넣지 않는다. built-in Rule은 `catalog/validators`, project Rule은 신뢰한 `.star-control` 선언이 정본이고 DB에는 resolved snapshot만 저장한다.

### ChangeRecipe

ChangeRecipe는 Finding 또는 사용자가 지정한 typed target에서 immutable PatchSet preview를 만드는 반복 가능한 shared 선언이다. 4단계에서는 Finding이 없는 `user_planned` ChangePlan도 Recipe를 사용할 수 있다.

- stable Recipe ID, SemVer와 definition fingerprint
- Finding selector 또는 `managed_declaration|contract|symbol|path_range|finding_occurrence|generator_input` target selector contract
- source·revision·dirty·Index precondition과 typed input Schema
- 대상 language·rewrite kind·assurance·expected postcondition
- built-in transformer, private language adapter 또는 trusted ToolDescriptor reference와 typed input binding
- allowed project-relative path scope, replay idempotency와 reverse PatchSet·격리 worktree rollback 계약
- Permission action, risk class와 required validation floor

Recipe에는 raw shell, 동적 script, AI prompt와 backend SQL을 넣지 않는다. raw literal은 target identity가 아니며 이미 resolve된 bounded target 안의 before predicate로만 쓸 수 있다. 0단계 공통 field는 [공통 개발 관리 계약](development-management.md), M4 exact field·version·resolution·idempotence는 [안전한 Patch·Refactor·codemod 엔진 계약](safe-patch-and-codemod.md#changerecipe-m4-target)을 따른다.

### ProfileDescriptor

ProfileDescriptor는 최종 16개 개발 작업 유형을 표현한다.

- 적용 대상 작업 성격과 선택 조건
- 단계 template와 단계 사이 의존 관계
- Context 수집 규칙
- Route hint·Validation 기본값과 필요한 Permission action 종류
- Review Pack, checkpoint, merge와 완료 증거 요구
- 선택적으로 하나의 부모 Profile ID

M3를 사용하는 ProfileDescriptor는 다음 metadata를 typed field로 추가한다.

| 필드 | 의미 |
|---|---|
| `triggers` | Task kind, ChangeSet class/facet, ImpactEdge·RiskPath selector |
| `gate_phases` | 공통 `during_stage\|stage_exit\|goal_exit\|patch_pre_apply\|patch_post_apply\|merge\|release`와 등록된 M8/M10 상세 phase 중 적용 위치 |
| `required_rule_families`, `required_check_families` | Validator Registry와 M2 candidate에 union할 floor |
| `optional_check_families` | confidence 보강용 candidate |
| `always_run_for` | previous success·cache로 생략할 수 없는 risk·change selector |
| `baseline_policy` | `off\|report_only\|ratchet_new\|ratchet_new_and_worsened\|clean_only` 중 minimum floor |
| `suppression_policy` | exact/bounded selector, expiry·approval 요구 |
| `claim_policy` | change·check·regression·compatibility 등 평가할 CompletionClaim kind |
| `stability_policy` | flaky family/risk의 review/block 처리 |
| `review_policy_by_context` | `cli_only`, `codex_managed`별 human/Codex review 경계 |
| `evidence_requirements` | ChangeSet·Run·DiagnosticEvaluation·Gate·ReviewPack 요구 |
| `corpus_requirements` | Profile/Rule 변경 시 필요한 fixture kind |
| `approval_checkpoints` | network·download·debug attach·dependency change·PatchSet apply처럼 Profile이 강화하는 exact prompt 경계 |
| `default_stop_state` | mutation Profile의 자동 진행 종료점. `dependency_upgrade`는 `awaiting_apply_approval` |

작업 Profile은 실행 코드를 포함하지 않고 Task·Tool·Check ID를 조합한다. action을 요구할 수는 있지만 `auto` 승인이나 더 넓은 권한을 부여할 수 없다. 부모 순환과 존재하지 않는 reference는 Catalog load 단계에서 거부한다.

`debug_recovery`의 debugger attach·민감 dump, `security_supply_chain`의 external refresh, `dependency_upgrade`의 network/download/dependency change와 PatchSet apply checkpoint는 built-in minimum `prompt`다. M8의 destructive migration, unknown-field loss, cross-project effect와 language writer cutover도 `prompt` floor이며 project replacement와 `personal_auto`가 `auto`로 낮출 수 없다.

M8 ProfileDescriptor는 공통 metadata 외에 다음 typed 의미를 materialize한다.

| Profile | 필수 metadata |
|---|---|
| `data_config_db_migration` | target kind, version/chain/invariant family, strategy floor, unknown/backup/restore/rehearsal/resume/destructive policy, default `awaiting_approval` + `pending_action=execute` |
| `performance_build` | explicit activation, workload ref, sample floor, comparability/noise/outlier/missing policy, metric/correctness family |
| `language_platform_migration` | behavior/equivalence dimension, coexistence·consumer order, codegen/codemod assurance, platform evidence, window/cutover/rollback policy |

Profile은 manifest의 raw command, SQL, compiler option 또는 benchmark script를 포함하지 않고 stable Task·Tool·Check ref만 조합한다. resolved M8 metadata·parent closure·activation evidence는 `profile_resolution_fingerprint`에 참여한다.

M10 `ci_release_deploy` ProfileDescriptor는 공통 metadata 외에 다음 typed 의미를 materialize한다.

| 필드 | 의미 |
|---|---|
| `validation_layer_refs` | `local_quick\|target\|full\|release`의 GatePolicy·Check promotion chain |
| `release_policy_ref` | versioned `packaging/release.toml`과 built-in minimum policy fingerprint |
| `target_environment_refs` | clean Windows x64·native ARM64 build/runtime/install environment requirement |
| `package_lifecycle_refs` | package dry-run·file list, install·safe_default·update·rollback·uninstall Check |
| `supply_chain_applicability_ref` | SBOM·provenance·signing의 required/not-required 근거와 evidence floor |
| `approval_state_policy` | `draft\|candidate\|blocked\|ready\|approved\|publishing\|publish_outcome_unknown\|published\|rollback_required\|withdrawn` 전이, role별 remote action과 exact approval binding |
| `evaluation_policy_refs` | Rule·Check·Profile·Recipe evaluation, validator guard, Radar·lifecycle policy |
| `evaluation_contexts` | `cli_only`와 `codex_integrated`를 합산하지 않는 cohort contract |

이 Profile은 별도 release engine을 포함하지 않는다. 기존 TaskSpec·ValidationPlan·M3 Gate·EvidenceBundle·ArtifactRef·RemoteOperationRecord와 공통 application command를 조합하며, CI·installer·signer·registry·deploy provider는 Catalog adapter로만 연결한다. Profile resolution에 참여한 release/evaluation policy와 lifecycle closure는 `profile_resolution_fingerprint`에 포함한다.

M11 `rust_style_auto_fix` ProfileDescriptor는 공통 metadata 외에 다음 typed 의미를 materialize한다.

| 필드 | 의미 |
|---|---|
| `profile_kind` | `rust_style_auto_fix`; 다른 Profile variant도 이 stable kind를 유지 |
| `pipeline_ref` | `rust_style_v1@1`과 ordered fixed adapter definition fingerprint |
| `tool_role_refs` | rustfmt check/rewrite, Clippy check/allowlisted fix의 exact ToolDescriptor ref |
| `formatting_policy` | stable cargo fmt, style edition/config resolution과 formatting-only classification floor |
| `clippy_fix_allowlist` | exact lint ID entry. group·wildcard 금지, built-in v1 기본은 빈 list |
| `suggestion_policy` | `MachineApplicable`과 span/replacement/hunk mapping required |
| `coverage_policy` | package/target/feature/triple/cfg/ownership required cell, 호환 feature set와 conflict 처리 |
| `scope_policy` | inspect/check/prepare/auto-apply별 selector, handwritten `.rs` modify ceiling |
| `side_effect_policy` | forbidden operation, source root complete manifest와 build script/proc macro write 검사 |
| `auto_apply_policy` | `safe_default|personal_auto` checkpoint, diff/public/dirty/stale ceiling과 required Gate phase |
| `corpus_requirements` | toolchain/style edition/feature/target/conflict/side-effect/idempotence/Windows fixture floor |

project/user Catalog는 exact lint ID allowlist와 coverage를 Profile variant의 versioned metadata로 제공할 수 있다. 같은 ID/version 내용이 다르면 충돌이고, child Profile은 built-in forbidden operation·stable-only·complete coverage·pre/post Gate floor를 약화할 수 없다. exact Clippy identity와 Corpus evidence가 없는 built-in allowlist entry는 load 단계에서 거부한다. 사용자 정의 shell command나 raw cargo/rustfmt/Clippy argv는 이 metadata에 허용하지 않는다.

등록된 상세 phase는 M8의 `migration_pre_execute`, `migration_post_execute`, `migration_post_rollback`, `performance_compare`, `language_cutover`와 M10의 `release_preflight`, `release_build`, `release_verify`, `release_package`, `release_install_lifecycle`, `release_ready`, `release_publish_preflight`, `release_publish_verify`다. 임의 문자열 phase를 허용하지 않으며 새 phase는 validation/evidence schema version과 fixture를 함께 바꾼다. `review_policy_by_context.codex_managed` execution에서 얻은 평가 case는 EvaluationRun의 `codex_integrated` context로 정규화하고 `cli_only`와 합치지 않는다.

Profile 결합에서 required Rule·Check·evidence는 union, baseline·suppression·stability·review floor는 가장 엄격한 값을 선택한다. 이 결합은 M2가 candidate를 확정하기 전에 수행하고 resolved ID/version/hash·activation evidence·parent closure·merged floor를 ValidationPlan에 materialize한다. M3 runner는 selected required Check를 추가·제거하거나 package→workspace→project full floor를 낮출 수 없으며 Profile closure가 달라지면 재계획한다.

`review_policy_by_context.cli_only`는 `none|human_semantic`만 허용한다. 의미 검토가 필요하면 GateDecision `human_review`를 만들며 Codex·AI ToolDescriptor를 required Check로 합성하지 않는다. `codex_managed`의 independent Codex review도 결정적 Check와 current evidence를 대체하지 않는다.

### PolicyProfileDescriptor

PolicyProfileDescriptor는 작업 유형이 아니라 사용자의 자동 진행 경계를 표현한다.

- ActionId별 `auto`, `prompt`, `deny`
- 허용 모델 역할·실행 방식과 동시 실행 상한
- 비용·시간·attempt 상한
- 반드시 실행할 validation·review 최소값
- 보호 경로·redaction·retention 제한
- 선택적으로 하나의 부모 정책 Profile

`personal_auto`의 M11 자동 승인은 일반 `local_write=auto`만으로 활성화되지 않는다. user-owned Rust style standing grant와 `rust_style_auto_fix` Profile의 `auto_apply_policy`, exact candidate `AUTO_PASS`, M3 `patch_pre_apply`와 single-use M4 permit이 모두 필요하다. `safe_default`와 다른 Profile은 이 grant를 참조하거나 policy evaluator resolution을 재사용할 수 없다.

프로젝트의 `required_policy_profile`은 현재 사용자 정책과 field별 `most_restrictive`, `minimum_limit`, `intersection`, `union`으로 합친다. 정책 Profile을 바꾼다는 이유로 값을 일반 `replace`하지 않는다.

## CatalogSnapshot 계약

실행 재현을 위해 최종 Catalog를 통째로 복사하지 않고 다음을 저장한다.

- 사용한 descriptor의 ID, format version, item version과 내용 SHA-256
- source와 trust 상태
- reference graph와 resolution 결과
- 계획에 참조한 ToolId·capability 조건과 당시 ToolRegistrySnapshot ID·hash
- scan·change 계획에 참조한 RuleId·ChangeRecipeId·RiskPathDescriptor, version과 definition fingerprint. M4에서는 Recipe transformer binding·target language/capability·input Schema hash도 포함
- affected 선택에 조사한 TaskDescriptor·CheckDescriptor 전체와 applicability·promotion **declaration** reference graph
- M3 ValidatorRegistrySnapshot, GatePolicyDescriptor, Profile validation metadata, Rule fingerprint contract와 fixture manifest reference graph
- M8 migration Tool/Check binding, version-chain/invariant Profile metadata, PerformanceWorkloadSpec·metric collector와 language equivalence/platform evidence requirement reference graph
- M10 release validation layer·package/install lifecycle·supply-chain applicability·approval state policy와 Rule·Check·Profile·Recipe evaluation/lifecycle reference graph
- M11 Rust style Profile/pipeline, Tool role, exact allowlist·coverage·auto policy, Corpus evidence와 fixed adapter/parser/side-effect validator reference graph
- 무시되거나 충돌한 항목과 Diagnostic
- snapshot 생성 시각과 제품 version

실행 중 일반 Catalog 파일이 바뀌어도 이미 시작한 stage의 계획 snapshot은 유지한다. 다만 live Tool Registry는 MCP connection이나 Stage에 고정하지 않는다. 각 tool invoke는 describe에서 확인한 `descriptor_hash`를 검증하고 실제 사용한 descriptor·EXE identity를 evidence에 남긴다. 계획과 호환되지 않는 변경이면 이전 계약을 추측 실행하지 않고 재설명·재계획을 요구한다.

## Managed Registry와 설정·Catalog 경계

Managed Registry는 Catalog의 다른 이름도, EffectiveConfig의 저장소도 아니다.

| 대상 | 정본과 책임 |
|---|---|
| config key의 stable ID·type·description·언어별 symbol | Git Managed Registry manifest의 `managed_declaration` |
| shipped default가 공유 계약인 경우 | `value_role=config_default` ManagedDeclaration과 source binding |
| build protocol·wire tag 같은 compile-time 값 | `value_role=compile_time_contract`; config override로 바꾸지 않음 |
| 사용자·Project·Goal override와 provenance | StarConfig → EffectiveConfig |
| Task·Tool·Check·Profile·Rule·Recipe 실행 metadata | Catalog descriptor → CatalogSnapshot |
| scanner가 찾은 아직 미승인 key/literal | Registry `candidate`; Catalog나 config key로 자동 승격하지 않음 |
| 한 module만 소유하는 구현 상수 | `local_implementation_constant`; 검색은 가능하지만 Registry·config가 소유하지 않음 |

Registry source는 `<project>/.star-control/managed-registry/manifest.toml`과 그 root가 명시적으로 나열한 fragment다. 이 위치를 이번 문서의 `[management]` 설정이나 DB row로 재지정하지 않는다. 5단계 첫 error-code Slice는 새 StarConfig key를 요구하지 않는다.

`CatalogSnapshot`과 `ManagedRegistrySnapshot`은 서로 독립된 snapshot이다. Registry 변경 계획은 사용한 CatalogSnapshot의 Recipe·Rule·Check fingerprint와 ManagedRegistrySnapshot의 source·binding·consumer fingerprint를 EvidenceSubjectBinding에서 함께 고정한다. DB 관리 surface는 config나 Registry source를 직접 저장하지 않고 [2단계 ChangePlan](change-planning-and-impact.md), [4단계 PatchSet](safe-patch-and-codemod.md)과 [3단계 Gate](../features/common-validation-gate.md)를 사용한다.

6단계 `ProjectContractManifest`도 별도 Catalog나 Registry가 아니다. public surface ID가 managed 대상이면 Registry declaration을 참조하고, command 실행은 Catalog descriptor를 참조하며, config actual value와 provenance는 EffectiveConfig를 참조한다. `DocumentationSnapshot`, `EnvironmentSnapshot`, `ProjectDoctorReport`는 모두 derived evidence이고 `.star-control/contracts.toml`, Registry manifest, Catalog source 또는 StarConfig에 역으로 쓰지 않는다.

config key의 상태는 declaration 존재 `declared`, Registry lifecycle `active|deprecated|removed`와 current 관찰 `documented|read|overridden`을 분리한다. `ConfigKeyTrace`는 declaration ref, Schema, docs, semantic reader binding, EffectiveConfig provenance와 consumer를 연결하지만 secret 또는 실제 override 값을 저장하지 않는다. complete semantic reader coverage가 없으면 unused key를 확정하거나 자동 삭제 후보로 만들지 않는다.

## 예시

```toml
schema_version = 1
policy_profile = "star.policy-profile.personal-auto"

[routing]
default_model_role = "terra"
default_reasoning_effort = "medium"
allowed_execution_modes = ["single", "max", "ultra"]
max_parallel_codex = 3

[permissions]
default_action = "prompt"

[permissions.actions]
local_read = "auto"
local_write = "auto"
local_delete = "auto"
external_write = "prompt"
paid_action = "prompt"

[validation]
fail_on = "error"
required_phases = ["stage", "goal"]
```

이 예시는 사용자의 개인 선호를 표현한다. 배포본의 제품 기본 정책 Profile은 `star.policy-profile.safe-default`다.

## Codex 설정과의 관계

Star-Control 설정은 Codex 설정을 대체하지 않는다.

- `reasoning_effort`는 Codex가 모델별로 지원하는 `minimal | low | medium | high | xhigh` 중에서 선택한다.
- Plan 단계의 생각 깊이는 일반 실행과 분리한다.
- Codex의 `approval_policy`와 `sandbox_mode`를 완화하지 않고 현재 허용 범위 안에서 PermissionPlan을 만든다.
- MCP server는 설치 시 `required=true`로 구성해 준비 실패를 닫힌 상태로 다룬다.
- MCP·app tool approval의 `auto`, `prompt`, `writes`, `approve` 같은 외부 모드는 adapter가 Star-Control action policy와 현재 capability에 맞춰 변환한다.

외부 값을 단순한 한 줄 우선순위로 비교하지 않는다.

| Codex 경계 | Star-Control 해석 |
|---|---|
| `approval_policy=untrusted \| on-request \| never \| granular` | 질문 가능 여부와 command별 추가 승인 제약으로 정규화. `never`를 Star-Control `auto`로 해석하지 않음 |
| `sandbox_mode=read-only` | 파일 write action을 실행 불가로 제한 |
| `sandbox_mode=workspace-write` | Codex가 허용한 root와 Star-Control ProjectPathRef의 교집합만 사용 |
| `sandbox_mode=danger-full-access` | Codex sandbox가 넓어도 Star-Control 목표·permission 범위는 유지 |
| MCP·app tool approval | 외부 tool별 prompt·write 제한을 추가 제약으로 반영 |

공식 기준은 [Codex 설정 Reference](https://developers.openai.com/codex/config-reference/)와 [MCP 지원 기능](https://learn.chatgpt.com/docs/extend/mcp#supported-mcp-features)을 실행 구현 직전에 다시 확인한다.

## 설정 오류와 미래 version

- 알 수 없는 key는 원문 보존 대상으로 격리하지만 현재 실행에는 적용하지 않고 오류를 낸다.
- 현재 reader보다 새 `schema_version`은 조회와 export만 허용하고 자동 저장·migration·실행을 거부한다.
- 낮은 version은 [version·migration 계약](versioning-and-migrations.md)에 따라 dry-run과 backup 뒤 올린다.
- 오류 보고에는 source, key 위치, 기대 type, 실제 type과 안전하게 고친 예시를 포함한다.
- 유효한 EffectiveConfig가 만들어지기 전에는 Controller가 Goal 실행을 시작하지 않는다.
