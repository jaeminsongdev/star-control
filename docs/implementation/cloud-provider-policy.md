# Cloud Provider Policy

## 목적

M6 cloud provider 구현은 실제 외부 호출보다 먼저 credential, privacy handoff, cost metric 계약을 강제한다.

이 문서는 `cloud_cli_agent` + `cli`, `cloud_api_model` + `http` provider의 공통 안전 기준이다. 특정 provider 공식 API/CLI 세부 동작은 실제 transport 구현 PR에서 최신 문서를 다시 확인한다.

## 기본 원칙

- credential raw value는 provider instance, artifact, log, report에 저장하지 않는다.
- cloud API provider는 `credential_ref`로만 인증 정보를 참조한다.
- cloud CLI provider는 `credential_ref` 또는 `transport_config.auth_mode: login_session`을 명시해야 한다.
- cloud handoff는 실행 전에 `privacy-handoff.json`으로 context 전달 범위를 남긴다.
- cost/budget은 provider-neutral `cost-metric.json`으로 남긴다.
- 실제 외부 API 호출, 유료 사용, 외부 계정 변경은 사용자 승인 없이 실행하지 않는다.

## credential policy

허용되는 credential reference 후보:

```text
env:NAME
keychain:NAME
secret-manager:NAME
login-session:NAME
```

금지되는 raw field 후보:

```text
api_key
token
access_token
refresh_token
secret
password
credential
credentials
bearer_token
```

Adapter는 provider instance에서 위 raw field가 문자열 값으로 발견되면 `ProviderRunResult.status=blocked`로 정규화하고 raw 값을 response, stderr, event에 복사하지 않는다.

## privacy handoff

Cloud provider는 실행 전 다음 artifact를 provider output directory에 남긴다.

```text
provider-output/{provider_instance_id}/privacy-handoff.json
```

`privacy-handoff.json`은 `specs/schemas/privacy-handoff.schema.json`을 따른다.

초기 M6a preflight는 `transport_config.privacy_handoff_approved`가 true가 아니면 transport 실행을 차단한다. 이후 approval flow가 cloud handoff approval을 생성하면 이 값을 approval artifact에서 유도할 수 있다.

## cost metric

Cloud provider는 다음 artifact를 provider output directory에 남긴다.

```text
provider-output/{provider_instance_id}/cost-metric.json
```

`cost-metric.json`은 `specs/schemas/cost-metric.schema.json`을 따른다.

M6a preflight는 실제 provider 호출이 없으므로 token과 wall time을 0으로 기록한다. 실제 CLI/API transport 구현 PR은 provider가 반환한 usage, rate limit, quota 정보를 가능한 범위에서 이 계약에 매핑한다.

## M6a preflight scope

M6a는 다음만 구현한다.

- cloud manifest kind/transport를 ExecutionEngine provider selection에 연결한다.
- cloud provider instance raw credential field를 차단한다.
- cloud API `credential_ref` 누락을 차단한다.
- cloud CLI `credential_ref` 또는 login session 선언 누락을 차단한다.
- privacy handoff approval 누락을 차단한다.
- `privacy-handoff.json`, `cost-metric.json`, `response.json`, `stdout.txt`, `stderr.txt`를 `.ai-runs/` 아래에 쓴다.

M6a는 다음을 구현하지 않는다.

- 실제 cloud CLI process 실행
- 실제 HTTP API 호출
- provider별 parser
- paid usage
- external account 변경
- credential raw value 조회

Preflight가 통과해도 M6a에서는 `cloud_provider_transport_not_implemented`로 `BLOCKED` 처리한다. 실제 transport 실행은 다음 M6 slice에서 별도 conformance fixture와 함께 구현한다.

## M6b cloud CLI transport scope

M6b는 `cloud_cli_agent` + `cli` provider만 실행한다.

실행 규칙:

- M6a preflight를 통과하지 못하면 transport를 시작하지 않고 기존 `BLOCKED` response를 쓴다.
- command는 shell 없이 `command.executable`과 `command.args` vector로만 실행한다.
- `cmd`, `powershell`, `pwsh`, `sh`, `bash`, `zsh` 같은 shell wrapper executable은 거부한다.
- cwd는 대상 프로젝트 root다.
- stdout/stderr는 provider output directory의 `stdout.txt`, `stderr.txt`에 capture한다.
- timeout은 `limits.timeout_seconds`를 사용하며, 최대값은 adapter가 제한한다.
- `credential_ref: env:NAME`의 raw value를 `command_policy.env_allowlist`로 그대로 넘기지 않는다.
- `command.args`는 `{{request_path}}`, `{{job_id}}`, `{{stage}}`, `{{goal}}` placeholder를 지원할 수 있다.

M6b 검증은 local test executable fixture만 사용한다. 실제 Codex CLI, Claude Code, Gemini CLI 같은 외부 CLI 호출 검증은 사용자가 승인한 별도 환경에서만 수행한다.

Provider doc refresh:

- 2026-07-01 기준 OpenAI Codex CLI docs를 확인했다.
- 확인 URL: `https://developers.openai.com/codex/cli/reference`, `https://developers.openai.com/codex/cli/features`, `https://developers.openai.com/codex/concepts/sandboxing`
- Codex CLI는 prompt를 command line 인자로 받을 수 있고, sandbox/approval 경계가 별도 설정으로 존재한다.
- Star-Control은 provider-specific flag를 core에 하드코딩하지 않고 provider instance `command` 설정으로 격리한다.

## conformance 방향

M6 전체 exit criteria에는 다음 fixture가 필요하다.

- cloud CLI preflight success + transport execution fixture
- cloud API preflight success + parser fixture
- missing credential_ref -> `BLOCKED`
- raw credential field -> `BLOCKED` and no raw value echo
- unapproved privacy handoff -> `BLOCKED`
- cost/rate/budget metric report mapping
