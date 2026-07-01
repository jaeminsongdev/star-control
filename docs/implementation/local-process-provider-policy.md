# Local Process Provider Policy

## 목적

M5 `local_process` provider를 구현하기 전에 command policy, sandbox policy, timeout/cancel behavior를 고정한다.

이 문서는 `local_process_model` kind와 `process` transport의 기본 안전 계약이다. local OpenAI-compatible server, cloud CLI, cloud API provider는 이 문서의 범위가 아니다.

## 기본 원칙

- shell을 기본 transport로 사용하지 않는다.
- provider instance가 명시한 executable과 argument vector만 실행한다.
- command string을 shell로 재해석하지 않는다.
- 대상 프로젝트 source file은 직접 수정하지 않는다.
- provider output은 대상 프로젝트 `.ai-runs/{job_id}/provider-output/{provider_instance_id}/` 아래에만 쓴다.
- credential raw value는 command, args, env, artifact, log에 저장하지 않는다.
- approval-required action은 자동 실행하지 않는다.

## command policy

`local_process` command는 다음 조건을 모두 만족해야 한다.

```text
transport: process
shell: false
executable: provider instance allowlist에 포함
args: array 형태
cwd: 대상 프로젝트 root 또는 명시적으로 허용된 하위 directory
stdin: rendered input file 또는 empty
stdout/stderr: provider output directory capture
env: deny-by-default + allowlist
```

금지:

- `cmd /c`, `powershell -Command`, `sh -c`, `bash -c` 같은 shell wrapper
- command line string join 후 실행
- glob, pipe, redirection, command substitution을 shell에 위임
- package manager 실행
- release/deploy/publish command 실행
- credential 또는 external account 설정 command 실행
- destructive file operation command 실행

초기 M5 구현은 `std::process::Command`에 executable과 args vector를 직접 전달한다. shell이 필요한 provider는 M5 범위가 아니며 별도 approval과 더 강한 sandbox 계약이 필요하다.

## sandbox policy

`process` transport provider는 sandbox profile이 필요하다.

초기 profile:

```text
network: deny
workspace_write: deny
allowed_write_roots:
  - .ai-runs/{job_id}/provider-output/{provider_instance_id}/
env: allowlist only
stdin: file or empty
max_parallel_jobs: 1
```

실제 OS sandbox 구현 전에도 adapter는 다음을 강제한다.

- output path traversal 차단
- provider output directory 밖 artifact reference 거부
- source path write claim이 있으면 `BLOCKED`
- forbidden action evidence가 있으면 `BLOCKED` 또는 `FAILED`
- timeout/cancel/error는 success로 변환하지 않음

## timeout / cancel behavior

Timeout 값은 provider instance `limits.timeout_seconds`에서 읽는다.

기본값:

```text
timeout_seconds: 300
max_timeout_seconds: 600
cancel_grace_seconds: 5
retry: disabled
```

Timeout:

- process를 중단한다.
- `ProviderRunResult.status`를 `timeout`으로 기록한다.
- stdout/stderr는 capture된 범위까지만 저장한다.
- RunState는 `FAILED`로 전이한다.
- `events.jsonl`에 timeout evidence를 남긴다.

Cancel:

- 실행 전 cancel이면 provider를 시작하지 않는다.
- 실행 중 cancel이면 process termination을 시도한다.
- cancel 성공 또는 best-effort 종료 후 `ProviderRunResult.status`를 `cancelled`로 기록한다.
- RunState는 `CANCELLED`로 전이한다.
- approval response의 `cancelled`와 provider cancel은 별도 사건으로 기록한다.

## forbidden action guard

다음 action은 local process provider가 자동 수행할 수 없다.

```text
dependency_install
dependency_change
workflow_change
release_publish
deploy
credential_change
external_account_change
file_delete
bulk_move
test_delete
test_skip_only_ignore
assertion_weakening
validator_self_bypass
sensitive_data_output
```

Adapter는 WorkSpec `forbidden_actions`, provider result evidence, command policy를 함께 확인한다. 하나라도 위반하면 다음 stage로 자동 진행하지 않는다.

## output contract

필수 output:

```text
provider-output/{provider_instance_id}/request.json
provider-output/{provider_instance_id}/response.json
provider-output/{provider_instance_id}/stdout.txt
provider-output/{provider_instance_id}/stderr.txt
```

`response.json`은 `specs/schemas/provider-run-result.schema.json`에 맞춰야 한다.

stdout/stderr는 secret redaction 후보가 포함될 수 있으므로 report에 전문을 복사하지 않는다. report는 path와 요약만 참조한다.

## 구현 순서

1. local process manifest/instance policy validation
2. command allowlist validator
3. process execution adapter with stdout/stderr capture
4. timeout handling
5. cancel state model
6. forbidden action guard
7. local provider contract tests

## M5 exit check

M5 local process provider는 다음을 통과해야 한다.

- 허용 command 실행 성공
- allowlist 밖 executable 거부
- shell wrapper 거부
- stdout/stderr capture
- timeout -> `timeout` result + `FAILED` RunState
- cancel -> `cancelled` result + `CANCELLED` RunState
- forbidden action evidence -> `BLOCKED` 또는 `FAILED`
- provider output directory 밖 artifact path 거부
