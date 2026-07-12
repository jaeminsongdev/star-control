# ToolPackageManifest TOML Reference

## 상태와 정본

이 문서는 `ToolPackageManifest format_version = 1`의 완전한 사람이 읽는 문법 정본이다. Rust type이 기계 정본이고 generated JSON Schema `star.tool-package-manifest`가 이 문서와 일치해야 한다. 구현자가 이 문서에 없는 key·기본값·암묵적 변환을 추가하면 안 된다.

파일 규칙:

- TOML 1.0, UTF-8 without BOM
- 파일 최대 1 MiB
- unknown·duplicate key 거부
- NUL, invalid Unicode와 remote include 거부
- field 이름은 snake_case
- path separator는 TOML 값에서 Windows `\` 또는 `/`를 받을 수 있지만 normalized form은 `/`
- 모든 relative reference는 manifest 파일의 실제 parent directory 기준
- symlink·junction을 따라간 최종 경로가 허용 root 밖이면 거부

`executable_id`, parameter name과 다른 package-local ID는 `^[a-z][a-z0-9_-]{0,63}$`다. PackageId·ToolId·ActionId는 [MCP 구현 동결 계약](mcp-implementation-contract.md#공통-lexical-제약)의 공통 형식을 사용한다. Windows 환경 변수 이름 비교는 대소문자를 구분하지 않고, 그 밖의 ID 비교는 byte-exact다.

## Root table

| key | 형식 | 필수 | 기본값·제약 |
|---|---|---:|---|
| `format_version` | integer | 예 | 정확히 `1` |
| `package_id` | PackageId | 예 | 전역 stable ID |
| `package_version` | SemVer string | 예 | build metadata 허용, range 아님 |
| `display_name` | string | 예 | 1~80자 |
| `description` | string | 예 | 1~2000자 |
| `enabled` | boolean | 아니요 | `true` |
| `required` | boolean | 아니요 | `false`, release source만 true 허용 |
| `publisher` | string | 아니요 | 최대 200자 |
| `homepage` | HTTPS URL | 아니요 | fragment 금지 |
| `license` | SPDX expression string | 아니요 | 없으면 `NOASSERTION` |
| `backend_kinds` | enum set | 예 | `process`, `controller_command` |
| `replaces` | inline table array | 아니요 | 기본 `[]`, 최대 16 ReplacementDescriptor |

`controller_command`는 checksum이 검증된 release source만 선언할 수 있다. user·project source에서 발견하면 package 전체를 거부한다.

`enabled=false`는 parse·unknown key·root·ID·locator 형식까지만 검증하고 trust·probe·process를 실행하지 않는다. complete action을 선언했다면 그 action의 정적 Schema·binding도 검증한다. disabled package는 active·last-known-good snapshot에 들어가지 않는다.

## ReplacementDescriptor

각 항목은 `{package_id, version_req}`다. package ID는 자기 자신일 수 없고 version requirement는 SemVer range다.

- replacement graph는 cycle이 없어야 하고 최대 깊이는 8이다.
- required release package는 replacement 대상이 될 수 없다.
- target package가 존재하고 version range에 맞으며 replacer가 ready·trusted일 때만 활성화한다.
- 같은 target을 둘 이상의 active package가 replace하면 우선순위를 추측하지 않고 모두 conflict로 둔다.
- target action은 search index에서 빠지지만 status와 provenance에는 남는다.
- 같은 PackageId가 여러 source origin에 있으면 version이 달라도 conflict다. source 순서로 덮지 않는다.
- `ToolId`는 effective Registry에서 전역 단일 owner를 가져야 한다. trusted replacement 적용 뒤에도 unrelated package가 같은 `ToolId`를 선언하면 해당 action은 fail-closed하며 PackageId 정렬 순서로 owner를 고르지 않는다. required release action의 frozen `ToolId`는 user·project package가 shadow할 수 없다.

## ExecutableDescriptor

`backend_kinds`에 `process`가 있으면 `[[executables]]`가 하나 이상 필요하다. package당 최대 16개다.

| key | 형식 | 필수 | 기본값·제약 |
|---|---|---:|---|
| `executable_id` | local ID | 예 | package 안에서 unique, 1~64자 |
| `locator_kind` | enum | 예 | `absolute`, `anchor_relative`, `location_ref` |
| `path` | string | 조건부 | absolute 또는 anchor-relative path |
| `anchor` | enum | 조건부 | `program_files`, `local_app_data`, `user_tools`, `package_dir` |
| `location_ref` | stable ID | 조건부 | user config의 absolute path mapping |
| `update_policy` | enum | 예 | `pinned_hash`, `version_compatible`, `follow_path` |
| `sha256` | SHA-256 | 조건부 | pinned_hash에서 필수 |
| `protocol` | enum | 예 | `argv_v1`, `star_json_stdio_v1` |
| `interface_version_req` | SemVer requirement | 아니요 | 기본 `*`, compatible에서 필수 |
| `product_version_req` | SemVer requirement | 아니요 | 기본 `*`; `*`가 아니면 probe 필수 |
| `architectures` | enum set | 아니요 | `x86_64,aarch64` |
| `minimum_windows_build` | integer | 아니요 | 기본 `26100` |
| `working_directory` | enum | 아니요 | 기본 `stage_worktree`; `project_root`, `stage_worktree`, `artifact_root`, `fixed` |
| `fixed_working_directory` | absolute path | 조건부 | working_directory=fixed에서 필수 |
| `environment_mode` | enum | 아니요 | 정확히 `core` |
| `environment_allow` | env-name array | 아니요 | 기본 `[]`, 최대 64 |
| `startup_args` | string array | 아니요 | literal만, 최대 32개 |
| `timeout_ms` | integer | 아니요 | `60000`, 100~86400000 |
| `max_stdout_bytes` | integer | 아니요 | `8388608`, 최대 67108864 |
| `max_stderr_bytes` | integer | 아니요 | `1048576`, 최대 8388608 |
| `max_memory_bytes` | integer 또는 null | 아니요 | null, 최소 16 MiB |
| `max_processes` | integer | 아니요 | `16`, 1~128 |
| `isolation_compatibility` | enum set | 아니요 | `trusted_desktop` |
| `authenticode_policy` | enum | 아니요 | `record` |
| `authenticode_subject` | string | 조건부 | `require_subject`에서 필수 |

locator 조건:

| `locator_kind` | 필수 | 금지 |
|---|---|---|
| `absolute` | absolute `path` | `anchor`, `location_ref` |
| `anchor_relative` | relative `path`, `anchor` | `location_ref` |
| `location_ref` | `location_ref` | `path`, `anchor` |

`location_ref`는 사용자 설정의 `tool_registry.locations`에서만 해석한다. project·Goal·일회성 설정은 이 map을 만들거나 바꿀 수 없다. `working_directory=fixed`는 release·user source에서만 허용하고 final directory를 trust scope에 포함한다. project source는 `project_root`, `stage_worktree`, `artifact_root`만 사용할 수 있다.

location mapping의 normalized ID·final path hash·user config revision은 descriptor와 trust scope에 들어간다. mapping 변경은 새 candidate다. `safe_default`는 새 path code trust를 요구하고 `personal_auto`는 사용자 설정 저장을 등록 의도로 기록해 자동 갱신할 수 있으며 project package가 이를 대신할 수 없다.

환경 변수 이름은 `^[A-Za-z_][A-Za-z0-9_]{0,127}$`다. `PATH`, `PATHEXT`, `COMSPEC`, `PSModulePath`, `PROMPT`는 `environment_allow`로 상속할 수 없다. 공통 process adapter가 최소 SystemRoot·TEMP·locale 환경을 만든다.

isolation 값:

- `trusted_desktop`: current user token + Job Object. 코드 신뢰 필요.
- `appcontainer_adapter`: Star-Control 호환 adapter만. brokered path와 capability가 필요.

`restricted_token`은 지원 enum이 아니다. 실제 project path·network 경계를 보장하지 못하면서 안전한 것처럼 보일 수 있어 제외한다.

working_directory는 `project_root`, `stage_worktree`, `artifact_root`, `fixed` 중 하나다. `appcontainer_adapter`는 `star_json_stdio_v1`과 `artifact_root`만 허용하며 network·external·system·Git remote·paid ActionId를 선언할 수 없다. project path input은 broker artifact로 materialize한 뒤 전달한다.

`product_version_req != "*"` 또는 `interface_version_req != "*"`이면 probe가 필수다. version 비교에는 probe가 반환한 SemVer만 사용하며 PE `ProductVersion` 문자열을 임의로 잘라 SemVer로 바꾸지 않는다.

`version_compatible`은 probe뿐 아니라 `authenticode_policy=require_subject`와 `authenticode_subject`가 필수다. byte가 바뀐 새 EXE를 자동 채택하는 근거는 valid publisher chain, subject, version·interface 범위의 교집합이다. unsigned tool은 `pinned_hash` 또는 명시적 user `follow_path`를 사용한다.

Authenticode 값:

- `ignore`: 검사하지 않음
- `record`: 서명 유무·chain 결과만 evidence 기록
- `require_valid`: 현재 Windows trust policy에서 valid해야 함
- `require_subject`: valid chain + leaf subject exact match

`require_subject`의 비교값은 `CertGetNameStringW(CERT_NAME_SIMPLE_DISPLAY_TYPE)` 결과를 Unicode NFKC한 뒤 invariant case-fold한 문자열이다. 원문과 normalized 값 모두 evidence에 남기되 MCP에는 원문 certificate detail을 기본 노출하지 않는다.

## IntegrityFile

선택 `[[executables.integrity_files]]`, executable당 최대 128개다.

| key | 형식 | 필수 | 규칙 |
|---|---|---:|---|
| `path` | relative path | 예 | executable install root 안 |
| `sha256` | SHA-256 | 예 | 선언한 sidecar의 exact byte |
| `required` | boolean | 아니요 | true |

path는 DLL·runtime·sidecar file만 가리킨다. glob, directory 전체 hash와 실행 중 download는 허용하지 않는다.

## ProbeDescriptor

`version_compatible`은 `[executables.probe]`가 필수다. `follow_path`는 선택, `pinned_hash`는 readiness 확인용으로 선택할 수 있다.

| key | 형식 | 필수 | 규칙 |
|---|---|---:|---|
| `kind` | enum | 예 | `argv`, `json_stdio` |
| `args` | string array | argv에서 필수 | literal 최대 16개 |
| `output_format` | enum | argv에서 필수 | `json`, `semver_line` |
| `version_pattern` | string | 조건부 | semver_line에서 필수, 최대 256자 |
| `timeout_ms` | integer | 아니요 | 5000, 최대 30000 |

`version_pattern`은 Rust regex 문법이며 named capture `product`와 optional `interface`만 허용한다. look-around, backreference와 256자 초과를 거부한다. compile size limit은 1 MiB다.

`json` probe stdout 형식:

```json
{
  "product_version": "14.1.0",
  "interface_version": "1.0.0",
  "capabilities": ["progress"]
}
```

`json_stdio` probe는 [Windows Tool Runtime](../architecture/windows-tool-runtime.md)의 `probe` frame을 사용하고 args·output format·pattern을 금지한다.

probe도 EXE 실행이므로 release trust, existing compatible trust 또는 사용자의 새 code-trust 결정 뒤에만 실행한다. probe 결과는 version·capability를 줄일 수만 있고 permission·isolation·paid 분류를 넓힐 수 없다.

probe의 `product_version`은 SemVer, `interface_version`은 SemVer 또는 null이다. `interface_version_req != "*"`이면 null을 허용하지 않는다. 인식하는 capability는 `progress`, `stdin_cancel`, `artifact_output`이다. unknown capability는 evidence에만 보존하고 동작을 활성화하지 않는다. `version_compatible`은 `interface_version_req`와 `product_version_req`를 모두 만족해야 한다.

argv probe는 exit code 0, stdout 최대 64 KiB와 선언 encoding UTF-8을 요구한다. `semver_line`은 stdout 첫 non-empty line 전체에 pattern이 한 번 match해야 하고, `json`은 위 object 외 unknown field를 거부한다. stderr는 진단 artifact일 뿐 version source가 아니다. 실패한 probe를 자동 재시도하거나 다른 흔한 flag로 추측하지 않는다.

## EnvironmentValue

선택 `[[executables.environment_values]]`, 최대 64개다.

| key | 형식 | 필수 | 규칙 |
|---|---|---:|---|
| `name` | env name | 예 | executable 안에서 unique |
| `value` | string | 조건부 | 고정 비민감 값 |
| `secret_ref` | SecretRef | 조건부 | secret 값 |

`value`와 `secret_ref` 중 정확히 하나다. project source는 고정 value를 지정할 수 있지만 user secret_ref 이름을 덮어쓰지 못한다.

환경 이름은 Windows 규칙대로 case-insensitive unique다. `SystemRoot`, `WINDIR`, `TEMP`, `TMP`, `USERNAME`, `USERDOMAIN`, `PATH`, `PATHEXT`, `COMSPEC`, `PSModulePath`는 manifest 값으로 덮어쓸 수 없다. `secret_ref`는 `env:NAME` 또는 `windows-credential:TARGET_NAME`만 허용하며 실제 값은 hash·log·MCP 결과에 들어가지 않는다.

## StateDirectory

선택 `[[executables.state_directories]]`, 최대 16개다.

| key | 형식 | 필수 | 규칙 |
|---|---|---:|---|
| `kind` | enum | 예 | `config`, `cache`, `data` |
| `scope` | enum | 예 | `operation`, `project`, `user` |
| `location` | enum | 예 | `controller_temp`, `controller_data`, `tool_default` |
| `environment_name` | env name | 조건부 | controller 위치를 child에 알릴 이름 |
| `retention` | enum | 아니요 | `delete_on_success`, `keep_on_failure`, `policy` |

`tool_default`는 `trusted_desktop`과 명시적 code trust에서만 허용한다. AppContainer adapter는 brokered Controller root만 사용한다.

`controller_temp`와 `controller_data`는 `environment_name`이 필수이고 `tool_default`에서는 금지한다. Controller는 scope별 final directory를 만든 뒤 해당 환경 변수에 넣는다. 이름은 EnvironmentValue·core environment와 충돌할 수 없다.

## ToolActionDescriptor

`[[actions]]`는 package당 최대 64개다.

| key | 형식 | 필수 | 기본값·제약 |
|---|---|---:|---|
| `tool_id` | ToolId | 예 | Registry 전역 unique |
| `backend_kind` | enum | 예 | `process`, `controller_command` |
| `backend_ref` | string | 예 | executable ID 또는 typed command ID |
| `display_name` | string | 예 | 1~80자 |
| `summary` | string | 예 | 1~240자 |
| `description` | string | 예 | 1~4000자 |
| `aliases` | string set | 아니요 | 최대 16, 각 1~80자 |
| `tags` | tag set | 아니요 | 최대 32 |
| `task_kinds` | tag set | 아니요 | 최대 16 |
| `when_to_use` | string array | 아니요 | 최대 8, 각 240자 |
| `when_not_to_use` | string array | 아니요 | 최대 8, 각 240자 |
| `permission_actions` | ActionId set | 예 | process는 `process_run` 필수 |
| `paid_action` | enum | 예 | `yes`, `no`, `unknown` |
| `idempotency` | enum | 예 | `read_only`, `idempotent`, `non_idempotent` |
| `execution_mode` | enum | 아니요 | 기본 `waitable`, 선택 `detachable` |
| `expected_duration_ms` | integer | 아니요 | 기본 1000, 0~86400000 |
| `cancel_mode` | enum | 아니요 | argv 기본 `terminate_job`, JSON-STDIO 기본 `stdin_frame` |
| `input_schema_file` | relative JSON path | 조건부 | parameters와 상호 배타 |
| `output_schema_file` | relative JSON path | 조건부 | JSON·JSONL·JSON-STDIO data에서 필수 |
| `examples` | inline table array | 아니요 | `{name, arguments}` valid 예시 최대 3개 |

process backend_ref는 같은 package의 executable ID여야 한다. controller command는 release core package에 등록된 allowlist command여야 한다.

`backend_kinds`는 실제 action backend kind의 중복 없는 정확한 집합이어야 한다. process action이 참조하지 않는 executable과 중복 ToolId·executable ID는 거부한다. 단, `enabled=false` draft는 executables만 있고 action이 0개일 수 있으며 Registry에 tool을 만들지 않고 `disabled`로만 표시한다.

각 example의 `name`은 1~80자, `arguments`는 action input Schema를 통과하는 object다. invalid example은 manifest가 선언하지 않는다. `star_tool_describe`가 보여 주는 invalid example은 generated Schema fixture가 존재할 때만 최대 3개 제공하며, 없으면 빈 array다.

permission ActionId는 다음 19개만 허용한다.

`local_read`, `local_write`, `local_delete`, `local_mass_move`, `process_run`, `dependency_change`, `system_change`, `secret_access`, `network_read`, `network_download`, `external_write`, `account_change`, `plan_execute`, `git_commit`, `git_merge`, `git_push`, `pull_request`, `release_publish`, `paid_action`

`paid_action=yes|unknown`이면 permission set에도 `paid_action`이 있어야 한다. `no`인데 permission set에 paid_action이 있으면 invalid다.

`idempotency=read_only`인데 [MCP 구현 동결 계약](mcp-implementation-contract.md#risk-lane-계산)의 write·destructive set ActionId가 있으면 invalid다. `execution_mode=detachable`은 MCP response를 Operation으로 분리한다는 뜻이며 process가 Job Object를 벗어난다는 뜻이 아니다. durable Operation을 지원하는 Controller command 또는 process action에서만 허용한다.

process action은 `[actions.output]`이 필수다. `argv_v1`은 `[actions.exit_codes]`도 필수이고, `star_json_stdio_v1`은 exit-code table을 금지하며 final protocol frame을 결과 정본으로 사용한다. `output.format=json|jsonl`과 모든 `star_json_stdio_v1` action은 `output_schema_file`이 필수다. JSONL에서는 Schema가 각 line item에 적용된다.

## ParameterDescriptor

`input_schema_file`을 쓰지 않을 때 `[[actions.parameters]]`로 object Schema를 생성한다. action당 최대 128개다.

| key | 형식 | 필수 | 규칙 |
|---|---|---:|---|
| `name` | snake_case ID | 예 | action 안 unique |
| `type` | enum | 예 | 아래 type |
| `description` | string | 예 | 1~500자 |
| `required` | boolean | 아니요 | false |
| `default` | typed value | 아니요 | type과 일치 |
| `enum_values` | typed array | 조건부 | enum에서 필수, 최대 128 |
| `minimum`, `maximum` | integer | 아니요 | integer type |
| `min_length`, `max_length` | integer | 아니요 | string·array |
| `pattern` | Rust regex | 아니요 | string, 최대 256자 |
| `path_kind` | enum | 조건부 | path type에서 필수 |
| `must_exist` | boolean | 아니요 | read path는 true 기본 |
| `mutually_exclusive_group` | local ID | 아니요 | 같은 group 최대 하나 |
| `requires` | parameter-name array | 아니요 | 최대 16 |
| `conflicts_with` | parameter-name array | 아니요 | 최대 16 |

지원 type:

- `string`, `integer`, `decimal_string`, `boolean`, `enum`
- `string_array`, `integer_array`
- `project_path`, `project_path_array`
- `artifact_ref`, `secret_ref`

`path_kind`는 `file`, `directory`, `file_or_directory`, `glob` 중 하나다. glob은 project-relative pattern만 허용하고 Controller가 match를 ProjectPathRef 목록으로 고정한 뒤 process에 전달한다.

object·union·nested array가 필요하면 local `input_schema_file`을 사용한다. input·output Schema는 Draft 2020-12 root object이고 root `additionalProperties`를 명시해야 한다. input root는 반드시 `additionalProperties=false`다. JSON Schema `type=number`, NaN·Infinity 의미는 금지하고 정수가 아니면 검증된 decimal string을 사용한다. remote `$ref`, recursive `$ref`, code·format assertion extension은 금지한다. local `$ref` 깊이 최대 64, package Schema 총 4 MiB, action 하나의 fully resolved input+output Schema 합계는 1 MiB다.

## ArgumentBinding

`argv_v1` action은 순서가 의미 있는 `[[actions.argv]]`를 사용한다. 최대 256개다.

| `kind` | 필수 key | 선택 key | 결과 |
|---|---|---|---|
| `literal` | `value` | 없음 | 고정 argument 하나 |
| `positional` | `input` | `when_present`, `when_equals` | 값 하나 |
| `option` | `flag`, `input` | 조건 | flag와 값 두 argument |
| `flag_if_true` | `flag`, `input` | 없음 | true면 flag |
| `flag_if_false` | `flag`, `input` | 없음 | false면 flag |
| `repeat` | `input` | `flag` | 원소마다 값 또는 flag+값 |
| `joined` | `flag`, `input`, `separator` | 조건 | 한 argument `flag+separator+value` |
| `terminator` | `value` | 없음 | 정확히 `--`만 허용 |
| `stdin_text` | `input` | `encoding` | stdin text 전체 |
| `stdin_json` | 없음 | `inputs` | selected arguments의 JSON object |
| `temp_file` | `input` | `suffix`, `encoding`, `content_kind` | Controller temp file path argument |

공통 조건 key는 `when_present` 또는 `{when_input,when_equals}` 중 하나만 쓴다. arbitrary expression, environment expansion, shell quote와 script는 금지한다. stdin binding은 action당 하나다.

- `input`, `when_input`과 `inputs`는 현재 action parameter 이름만 참조한다.
- `when_present=true`는 optional 값이 실제 arguments에 있을 때만 binding을 낸다. `when_equals`는 `when_input`의 scalar typed value와 type까지 같을 때만 참이다.
- `option.flag`, `flag_if_*.flag`와 `joined.flag`는 `-`로 시작하는 1~64자 literal이며 공백·quote·NUL을 금지한다.
- `joined.separator`는 `=` 또는 `:`만 허용한다.
- `repeat`는 array parameter만, `flag_if_*`는 boolean parameter만 사용한다.
- `stdin_json.inputs`를 생략하면 secret_ref를 제외한 전체 input을 parameter 선언 순서로 담고, 지정하면 해당 이름만 담는다. wire byte는 UTF-8 JCS JSON이다.
- `temp_file.content_kind`는 `text`, `json`, `base64`다. `text`는 string, `json`은 임의 Schema 값, `base64`는 base64 string input만 받는다. `encoding`은 text·json에서 `utf8` 또는 `utf16le`, base64에서는 금지한다. `suffix`는 `^\.[A-Za-z0-9][A-Za-z0-9._-]{0,15}$`다.
- project path type은 Controller가 검증한 final absolute path로 binding한다. path가 option처럼 보일 수 있는 positional 또는 repeat binding은 그 앞에 `terminator`가 없으면 manifest를 거부한다.

`star_json_stdio_v1` action은 `actions.argv`를 금지한다. executable의 literal `startup_args` 뒤 protocol frame만 사용한다.

## ExitCodeContract

`argv_v1`은 `[actions.exit_codes]`가 필수다.

| key | 형식 | 기본값 |
|---|---|---|
| `success` | integer set | `[0]` |
| `empty` | integer set | `[]` |
| `warning` | integer set | `[]` |
| `retryable` | integer set | `[]` |

각 code는 0~4294967295이고 집합은 서로 겹치면 안 된다. unlisted code는 non-retryable failure다. `empty`는 `status=ok`, 빈 data와 설명을 반환한다. `warning`은 `status=ok`와 Diagnostic을 반환한다. exit code만으로 gate 성공을 주장하지 않는다.

## OutputContract

`[actions.output]`:

| key | 형식 | 필수 | 기본값·규칙 |
|---|---|---:|---|
| `format` | enum | 예 | `text`, `json`, `jsonl`, `binary` |
| `encoding` | enum | 아니요 | 기본 `utf8`; `utf8`, `oem`, `utf16le`, binary format은 `binary`만 |
| `stderr_encoding` | enum | 아니요 | 기본 `utf8`; `utf8`, `oem`, `utf16le`, `binary` |
| `inline_limit_bytes` | integer | 아니요 | 65536, executable 상한 이하 |
| `max_items` | integer 또는 null | 아니요 | jsonl 기본 5000, 그 밖은 null |
| `overflow` | enum | 아니요 | `artifact`, `error`; 기본 `artifact` |
| `stdout_role` | enum | 아니요 | `data` 고정 |
| `stderr_role` | enum | 아니요 | `log` 고정 |
| `artifact_media_type` | string 또는 null | 아니요 | binary·artifact에서 사용 |

JSON·JSONL data는 output Schema가 있으면 반드시 검증한다. text는 summary와 optional artifact만 만들며 임의 regex로 Diagnostic을 추출하지 않는다. overflow에서 성공 data를 조용히 truncate하지 않는다.

JSON은 document 하나, JSONL은 빈 줄을 허용하지 않는 object line sequence다. `stderr_encoding=binary`면 stderr를 inline log로 해석하지 않고 quarantine artifact로만 저장한다. `retryable` exit code는 `retryable=true` ErrorEnvelope를 만들지만 v1 Controller는 자동 재시도하지 않는다.

## ConcurrencyContract

`[actions.concurrency]`:

| key | 형식 | 기본값 |
|---|---|---|
| `max_parallel` | integer | 1, 최대 64 |
| `exclusive_scope` | enum | `none`, 선택 `project,worktree,custom` |
| `lock_key_inputs` | parameter array | `[]`, custom에서 필수 |
| `queue_timeout_ms` | integer | 30000 |

custom lock key는 `{tool_id,selected_arguments}`의 arguments canonical hash다. lock 획득 순서는 scope kind, ProjectId, ToolId, lock hash 오름차순으로 고정해 deadlock을 막는다.

## CancelContract

- `argv_v1`: `cancel_mode = "terminate_job"` 또는 `none`
- `star_json_stdio_v1`: `cancel_mode = "stdin_frame"`, `terminate_job`, `none`
- `controller_command`: command capability에서 결정, manifest 지정 금지

`[actions.cancel]`의 `grace_ms` 기본 2000, 최대 30000이다. stdin cancel frame 뒤 grace가 지나면 Job Object를 종료한다. `none`은 cancel intent만 기록하고 timeout까지 기다리며 UI에 `not_cancellable`을 표시한다.

## Source·update policy 규칙

| source | 허용 update policy | trust |
|---|---|---|
| release | pinned_hash, version_compatible | installer checksum·서명 |
| user safe_default | 세 정책 | 첫 package·path trust |
| user personal_auto | 세 정책 | 관리 root 저장을 등록 의도로 사용 |
| project | pinned_hash만 | manifest·Schema·EXE exact trust |

`follow_path`는 같은 CLI·output 계약을 유지하는 byte 교체만 자동 채택한다. contract가 바뀌면 TOML·Schema를 함께 바꿔 descriptor hash를 갱신한다. `version_compatible`은 probe 통과, architecture, Authenticode와 integrity file 검사를 모두 만족해야 한다.

같은 source origin의 user 편집은 package_version을 유지해도 candidate로 읽지만 실행 의미가 바뀌었는데 version이 같으면 `PACKAGE_VERSION_NOT_BUMPED` warning을 남긴다. 서로 다른 source에서 같은 package ID·version인데 manifest hash가 다르면 conflict다. 공개 release와 공유 project package는 실행 의미 변경 때 SemVer를 반드시 올린다.

## Normalization과 validation 순서

1. UTF-8·TOML parse
2. root·unknown key·source rule
3. ID·SemVer·URL·SPDX lexical validation
4. reference graph·cycle·중복 검사
5. Schema parse·local ref·size·depth 검사
6. parameter→Schema 생성
7. binding type·순서·stdin·output·exit 검사
8. permission→risk lane 계산
9. locator resolve·path policy
10. trust lookup
11. executable identity·integrity·probe
12. normalized hash와 immutable PackageSnapshot 생성

앞 단계 실패에서 뒤 단계 process를 실행하지 않는다.

## `star tools` 관리 CLI 동결

모든 명령은 기본 human text, `--json`이면 해당 Controller result JSON을 stdout에 쓴다. mutation은 Controller IPC로만 수행한다.

`list`와 `status`는 CLI가 Controller cursor를 내부적으로 끝까지 소비한다. public CLI syntax에 cursor option을 추가하지 않으며 Registry 상한 안의 전체 결과를 한 번 출력한다.

| 명령 | exact syntax | 의미 |
|---|---|---|
| list | `star tools list [--source release|user|project] [--readiness <value>] [--json]` | active action 조회 |
| describe | `star tools describe <tool-id> [--json]` | effective descriptor·hash·provenance |
| status | `star tools status [<package-id>] [--json]` | watcher·candidate·LKG·diagnostic |
| validate | `star tools validate <manifest-path> --source user|project [--json]` | publish·trust·probe 없이 단계 1~9의 정적 검사 |
| probe | `star tools probe <package-id> [--executable <id>] [--json]` | 선언된 side-effect-free probe만 실행 |
| trust | `star tools trust <package-id> --manifest-hash <sha256> [--expires <rfc3339>] [--json]` | 현재 candidate scope trust |
| revoke | `star tools revoke <package-id> [--cancel-running] --reason <text> [--json]` | 새 invoke 차단, 선택 running cancel |
| scaffold | `star tools scaffold <exe-path> --output <toml-path>` | 최소 disabled user manifest 초안 생성 |

`scaffold` output은 `enabled=false`, `update_policy=pinned_hash`, 현재 file SHA-256·architecture·recorded signature와 action 0개인 disabled draft다. EXE를 실행하지 않고 PE·file metadata만 읽는다. `--help` text에서 action, permission·exit·Schema를 자동 확정하지 않는다. reload 명령은 만들지 않으며 저장 event와 demand scan이 기준이다.

`scaffold --output` 대상이 이미 있으면 덮어쓰지 않고 종료 code 2다. 새 파일은 같은 directory의 temporary file을 flush한 뒤 atomic rename으로 만든다.

종료 code는 조회·valid 0, 입력·manifest 오류 2, trust·approval 대기 3, Controller·probe·외부 process 실패 4, protocol·version 비호환 6이다. `--json`에서도 stderr에는 한 줄 human error summary만 쓰고 secret·absolute path를 출력하지 않는다.

## 완전한 `argv_v1` 예시

```toml
format_version = 1
package_id = "user.ripgrep"
package_version = "1.0.0"
display_name = "Ripgrep"
description = "프로젝트 파일에서 정규식으로 텍스트를 검색한다."
enabled = true
required = false
publisher = "BurntSushi"
homepage = "https://github.com/BurntSushi/ripgrep"
license = "MIT OR Unlicense"
backend_kinds = ["process"]

[[executables]]
executable_id = "rg"
locator_kind = "absolute"
path = 'C:\Tools\ripgrep\rg.exe'
update_policy = "follow_path"
protocol = "argv_v1"
interface_version_req = "*"
product_version_req = ">=14.0.0"
architectures = ["x86_64"]
minimum_windows_build = 26100
working_directory = "stage_worktree"
environment_mode = "core"
environment_allow = []
timeout_ms = 60000
max_stdout_bytes = 8388608
max_stderr_bytes = 1048576
max_processes = 4
isolation_compatibility = ["trusted_desktop"]
authenticode_policy = "record"

[executables.probe]
kind = "argv"
args = ["--version"]
output_format = "semver_line"
version_pattern = '^ripgrep (?P<product>[0-9]+\.[0-9]+\.[0-9]+)'
timeout_ms = 5000

[[actions]]
tool_id = "user.ripgrep.search"
backend_kind = "process"
backend_ref = "rg"
display_name = "프로젝트 텍스트 검색"
summary = "프로젝트 상대 경로에서 정규식을 검색한다."
description = "ripgrep JSONL 결과를 반환하며 파일을 수정하지 않는다."
aliases = ["rg", "text search"]
tags = ["search", "source", "docs"]
task_kinds = ["analyze"]
when_to_use = ["정확한 문자열이나 정규식으로 파일 위치를 찾을 때"]
when_not_to_use = ["의미 기반 검색이 필요할 때"]
permission_actions = ["local_read", "process_run"]
paid_action = "no"
idempotency = "read_only"
execution_mode = "waitable"
cancel_mode = "terminate_job"
output_schema_file = "schemas/ripgrep-match.schema.json"
examples = [{ name = "src에서 TODO 찾기", arguments = { pattern = "TODO", paths = ["src"] } }]

[[actions.parameters]]
name = "pattern"
type = "string"
description = "검색할 정규식"
required = true
min_length = 1
max_length = 4096

[[actions.parameters]]
name = "paths"
type = "project_path_array"
description = "검색할 프로젝트 상대 경로"
required = false
max_length = 256
path_kind = "file_or_directory"
must_exist = true

[[actions.argv]]
kind = "literal"
value = "--json"

[[actions.argv]]
kind = "positional"
input = "pattern"

[[actions.argv]]
kind = "terminator"
value = "--"

[[actions.argv]]
kind = "repeat"
input = "paths"

[actions.exit_codes]
success = [0]
empty = [1]
warning = []
retryable = []

[actions.output]
format = "jsonl"
encoding = "utf8"
stderr_encoding = "utf8"
inline_limit_bytes = 65536
max_items = 5000
overflow = "artifact"
stdout_role = "data"
stderr_role = "log"

[actions.concurrency]
max_parallel = 4
exclusive_scope = "none"
lock_key_inputs = []
queue_timeout_ms = 30000

[actions.cancel]
grace_ms = 2000
```

예시가 참조하는 `schemas/ripgrep-match.schema.json`은 package directory 안의 다음 Draft 2020-12 Schema다. 실제 ripgrep JSONL 한 line을 검증하되 remote reference를 사용하지 않는다.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "urn:star-control:schema:user.ripgrep.match:v1",
  "type": "object",
  "required": ["type", "data"],
  "properties": {
    "type": {
      "type": "string",
      "enum": ["begin", "match", "context", "end", "summary"]
    },
    "data": {
      "type": "object"
    }
  },
  "additionalProperties": false
}
```

## 반드시 거부할 예

- `command = "cmd /c ..."`, PowerShell code 또는 shell interpolation
- PATH lookup과 extension 생략
- project source의 follow_path·version_compatible
- user·project controller_command
- remote URL Schema·include
- unknown permission ActionId
- 서로 겹치는 exit code
- JSON output인데 output Schema가 필요한 structured field를 검증하지 않는 선언
- `read_only`인데 write ActionId 포함
- paid_action metadata와 ActionId 불일치
- binding이 존재하지 않는 parameter를 참조
- stdin binding 두 개
- path traversal, UNC executable, device path, alternate data stream
- executable·integrity file reparse point

## 연결 문서

- [MCP 구현 동결 계약](mcp-implementation-contract.md)
- [무재시작 외부 Tool Registry](external-tool-registry.md)
- [Windows Tool Runtime](../architecture/windows-tool-runtime.md)
- [MCP 검증 행렬](../testing/mcp-verification-matrix.md)
