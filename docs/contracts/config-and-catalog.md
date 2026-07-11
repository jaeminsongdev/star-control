# 설정과 Catalog 계약

## 목표와 경계

설정은 사용자가 바꿀 수 있는 선택과 제품이 지켜야 하는 안전 제한을 분리한다. 개인 사용자는 `personal_auto` 정책 Profile로 유료 동작 외의 범위 내 작업을 자동 진행할 수 있고, 공개 배포본은 `safe_default` 정책 Profile로 시작한다. 어느 정책·작업 Profile도 Codex, 운영체제 또는 관리자가 강제한 제한을 약화하지 못한다.

이 문서는 설정의 형식, 병합, 출처 추적과 Catalog descriptor를 소유한다. 설치·update·개인정보와 공개 release 절차는 [설치와 공개 배포](../operations/installation.md)가 소유한다.

## 설정 파일과 형식

| 종류 | 위치 | 역할 |
|---|---|---|
| 사용자 설정 | `%APPDATA%\Star-Control\config.toml` | 모든 프로젝트의 사용자 선호 |
| 프로젝트 설정 | `<project>\.star-control\config.toml` | 저장소별 규칙 |
| 목표 설정 | Controller의 GoalSpec 부속 문서 | 한 목표에만 적용하는 값 |
| 일회성 설정 | `star` 명령 또는 MCP 입력 | 한 명령 또는 한 run에만 적용 |
| 실행 상태 | `%LOCALAPPDATA%\Star-Control\` | EffectiveConfig, snapshot과 상태 |

- 사람이 편집하는 설정은 UTF-8 TOML이다. UTF-8 BOM은 읽을 수 있지만 다시 쓸 때는 BOM 없이 정규화한다.
- 파일에는 최상위 `schema_version`과 `policy_profile`을 둔다. 15개 개발 작업 유형은 별도 `default_work_profile` 또는 StageSpec에서 선택한다.
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
| `fingerprint` | SHA-256 | secret 값을 제외한 canonical values와 제약의 hash |
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
| `external_write` | 외부 서비스 상태 변경 | `prompt` | `auto` |
| `account_change` | 외부 계정·권한·resource 변경 | `prompt` | `auto` |
| `plan_execute` | 현재 계획의 실행 시작 | `prompt` | `auto` |
| `git_commit` | 로컬 commit 생성 | `prompt` | `auto` |
| `git_merge` | branch·worktree 결과 통합 | `prompt` | `auto` |
| `git_push` | 원격 push | `prompt` | `auto` |
| `pull_request` | PR 생성·수정 | `prompt` | `auto` |
| `release_publish` | 공개 release·배포 | `prompt` | `auto` |
| `paid_action` | 비용이 발생하거나 유료 한도를 쓰는 동작 | `prompt` | `prompt` |

`personal_auto`도 목표 밖 경로, 제품의 deny, Codex approval·sandbox, 관리자 제한을 넘지 않는다. 비용 발생 여부를 판정할 근거가 없으면 `paid_action=prompt`로 취급한다.

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

| key | type | 기본값 | 병합 |
|---|---|---|---|
| `required_phases` | enum set | `stage,goal` | `union` |
| `fail_on` | severity | `error` | 더 엄격한 severity |
| `command_timeout_ms` | integer | `600000` | `minimum_limit` |
| `allow_manual_evidence` | boolean | `true` | 제한은 `false` 우선 |
| `require_independent_review_for` | risk set | `high,critical` | `union` |
| `max_log_bytes` | integer | `10485760` | `minimum_limit` |
| `checks_add` | Catalog ID set | 빈 값 | `union` |
| `checks_remove` | Catalog ID set | 빈 값 | 필수 검사는 제거 불가 |

### `[vcs]`, `[remote]`, `[state]`

| section.key | type | 기본값 | 설명 |
|---|---|---|---|
| `vcs.use_worktree` | boolean | `true` | 병렬 변경을 별도 작업 복사본에서 수행 |
| `vcs.merge_strategy` | enum | `review_then_merge` | `review_then_merge`, `manual`, `never` |
| `vcs.protected_branches` | string set | repository에서 탐지 | 보호 대상은 `union` |
| `vcs.worktree_root` | path | Controller data 아래 | source 기준으로 해석 |
| `remote.allowed_hosts` | host set | 빈 값 | 상위 제한과 `intersection` |
| `remote.require_clean_target` | boolean | `true` | 원격 변경 전 상태 검사 |
| `state.artifact_root` | path | `<project>\.ai-runs\star-control` | 프로젝트 증거 위치 |
| `state.checkpoint_interval_ms` | integer | `300000` | 긴 실행의 최대 checkpoint 간격 |
| `state.completed_retention_days` | integer | `90` | 완료 run의 큰 원문·중간 artifact 보관 기간 |
| `state.failed_retention_days` | integer | `180` | 해결된 실패의 재현 자료 보관 기간 |
| `state.redaction_rules_add` | rule ID set | built-in rules | `union` |
| `state.cleanup_trigger` | enum | `startup_and_manual` | `manual`, `startup_and_manual`. 자체 예약 실행은 없음 |

보관 정책은 실행 중 자료, 최종 요약·manifest, 보존 hold와 미해결 실패 자료를 삭제 대상으로 만들 수 없다. 실제 삭제는 별도 permission과 audit event를 필요로 한다.

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
| `tool_registry.max_tools` | integer | `512` | 검색 가능한 action 수 상한 |
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
- `tool_registry.allow_path_lookup=false`, `tool_registry.live_reload=true`, `tool_registry.demand_scan=true`, `tool_registry.verify_executable_identity_each_call=true`, `tool_registry.project_update_policy=pinned_hash`, `mcp_gateway.contract_version=1`, `ipc.auth_required=true`는 v1 불변값이다.
- `tool_registry.watch_files`만 진단 목적으로 false로 낮출 수 있다. 이 경우에도 request 전 demand scan은 유지된다.
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

같은 ID와 version의 내용이 다르면 우선순위로 덮지 않고 충돌로 중단한다. 대체는 `replaces`가 명시되고 참조 compatibility가 검증될 때만 허용한다.

### TaskDescriptor

- 목표 입력과 결과 type
- 기본 Stage 성격과 완료 조건 template
- 필요한 Context 항목과 capability
- 기본 Route hint와 Permission action
- 실행 뒤 요구할 Check와 Evidence 종류
- 재시도 가능 조건과 idempotency 성격

### ToolDescriptor

- stable ToolId, 검색용 이름·설명·tag·capability와 input·output Schema
- 고정 MCP risk lane과 read·destructive·open-world·idempotency 성격
- executable identity, `pinned_hash | version_compatible | follow_path` update policy와 지원 protocol
- 구조화된 argument binding, cwd·환경·timeout·출력 상한
- stdin·stdout·stderr 형식과 exit code 의미
- progress·취소·동시성·lock과 retryable failure 표시. 외부 EXE 자동 retry는 v1에서 하지 않음
- secret 요구, redaction, side effect·비용 성격과 Permission ActionId set

외부 EXE package의 정확한 manifest, protocol, trust와 reload 규칙은 [외부 Tool Registry](external-tool-registry.md)가 소유한다. shell 한 줄 문자열, 임의 `cmd /c`, PowerShell script text를 persisted 실행 계약으로 저장하지 않는다. 복잡한 도구는 별도 adapter EXE로 `star_json_stdio_v1`을 구현한다.

### CheckDescriptor

- 언제 선택하는지 나타내는 파일·변경·Profile 조건
- 참조할 ToolDescriptor와 typed invocation template
- 결과 parser와 Diagnostic severity mapping
- timeout, cache key input과 재실행 조건
- Gate 기본값과 생성할 evidence 종류

### ProfileDescriptor

ProfileDescriptor는 15개 개발 작업 유형을 표현한다.

- 적용 대상 작업 성격과 선택 조건
- 단계 template와 단계 사이 의존 관계
- Context 수집 규칙
- Route hint·Validation 기본값과 필요한 Permission action 종류
- Review Pack, checkpoint, merge와 완료 증거 요구
- 선택적으로 하나의 부모 Profile ID

작업 Profile은 실행 코드를 포함하지 않고 Task·Tool·Check ID를 조합한다. action을 요구할 수는 있지만 `auto` 승인이나 더 넓은 권한을 부여할 수 없다. 부모 순환과 존재하지 않는 reference는 Catalog load 단계에서 거부한다.

### PolicyProfileDescriptor

PolicyProfileDescriptor는 작업 유형이 아니라 사용자의 자동 진행 경계를 표현한다.

- ActionId별 `auto`, `prompt`, `deny`
- 허용 모델 역할·실행 방식과 동시 실행 상한
- 비용·시간·attempt 상한
- 반드시 실행할 validation·review 최소값
- 보호 경로·redaction·retention 제한
- 선택적으로 하나의 부모 정책 Profile

프로젝트의 `required_policy_profile`은 현재 사용자 정책과 field별 `most_restrictive`, `minimum_limit`, `intersection`, `union`으로 합친다. 정책 Profile을 바꾼다는 이유로 값을 일반 `replace`하지 않는다.

## CatalogSnapshot 계약

실행 재현을 위해 최종 Catalog를 통째로 복사하지 않고 다음을 저장한다.

- 사용한 descriptor의 ID, format version, item version과 내용 SHA-256
- source와 trust 상태
- reference graph와 resolution 결과
- 계획에 참조한 ToolId·capability 조건과 당시 ToolRegistrySnapshot ID·hash
- 무시되거나 충돌한 항목과 Diagnostic
- snapshot 생성 시각과 제품 version

실행 중 일반 Catalog 파일이 바뀌어도 이미 시작한 stage의 계획 snapshot은 유지한다. 다만 live Tool Registry는 MCP connection이나 Stage에 고정하지 않는다. 각 tool invoke는 describe에서 확인한 `descriptor_hash`를 검증하고 실제 사용한 descriptor·EXE identity를 evidence에 남긴다. 계획과 호환되지 않는 변경이면 이전 계약을 추측 실행하지 않고 재설명·재계획을 요구한다.

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
external_write = "auto"
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
| `approval_policy=untrusted | on-request | never | granular` | 질문 가능 여부와 command별 추가 승인 제약으로 정규화. `never`를 Star-Control `auto`로 해석하지 않음 |
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
