# Testing / CI / Release 구현 계약

## 목적

이 문서는 Star-Control 구현이 커질수록 어떤 검증 단계를 추가하고, 어떤 기준을 만족해야 release 가능한지 정의한다. 현재 repository는 설계와 계약 단계이므로 release automation은 구현하지 않는다. 다만 Codex 장시간 구현이 진행될 때 테스트와 CI를 약화하지 않도록 기준을 먼저 고정한다.

## 기본 원칙

- 실패한 검사를 삭제하거나 약화해서 통과시키지 않는다.
- 테스트 삭제, assertion 약화, skip-only-ignore 추가는 Star Sentinel 검증 대상이다.
- 새 package가 생기면 해당 언어의 기본 formatting, lint, test를 추가한다.
- CI workflow 변경은 high-risk이며 review와 approval 대상이다.
- release/deploy는 별도 승인 전까지 RESERVED다.

## local validation entrypoint

로컬에서 우선 실행할 기본 명령은 다음이다.

```text
python scripts/ci/run_all.py
```

이 명령은 현재 contract validator를 순서대로 실행한다. 개별 실패를 조사할 때만 job별 명령을 따로 실행한다.

## 현재 CI

현재 CI job:

```text
repository-policy-check
data-format-check
manifest-contract-check
naming-policy-check
schema-example-check
implementation-documentation-check
work-queue-consistency-check
```

이 job들은 설계/계약 repo 단계의 최소 안전선이다.

## complete implementation milestone validation

완전 구현 milestone은 `complete-implementation-roadmap.md`를 기준으로 하고, CI는 milestone이 진행될 때마다 작은 PR로 확장한다.

| milestone | validation baseline |
|---|---|
| M0 문서와 결정 정렬 | `python scripts/ci/run_all.py`, `git diff --check` |
| M1 Runtime Foundation | Rust fmt/check/test, schema validator tests, StateStore path/atomic write tests |
| M2 Provider-neutral Execution | provider registry tests, FakeProviderAdapter tests, RouterEngine deterministic tests, ExecutionEngine artifact tests |
| M3 Validation / Gate | Star Sentinel P0 fixture tests, gate decision tests, ValidationEngine state mapping tests |
| M4 v0 Fake E2E | fake project integration smoke with AUTO_PASS/HUMAN_REVIEW/BLOCK |
| M5 Local Provider | command policy, timeout/cancel, sandbox, stdout/stderr capture tests |
| M6 Cloud Provider | provider conformance tests, credential reference tests, budget/cost/privacy handoff tests |
| M7 Daemon / API | daemon queue smoke, API read-only contract tests, approval/cancel/resume mutation tests |
| M8 UI Shell | UI view model contract tests, read-only smoke, approval flow smoke |
| M9 Hardening / Release Readiness | redaction, audit, recovery, retention, release readiness checks |

Milestone validation은 누적된다. 뒤 단계로 갈수록 앞 단계 검증을 삭제하지 않고, 필요하면 quick/full profile로 분리한다.

검증 비용은 milestone에 맞춰 늘린다. 현재 구현 전 단계에서는 `python scripts/ci/run_all.py`와 필요한 경우 `git diff --check`를 기본선으로 두고, package build/test나 provider smoke는 해당 구현물이 생긴 뒤에만 CI에 추가한다.

## repository-policy-check

목적:

- 필수 파일/디렉터리 존재 확인
- Star-Control repo 내부 실행 산출물 금지 확인

확장 후보:

- `.ai-runs/` 금지 강화
- provider-output/tool-output 금지
- package boundary 검사
- docs/implementation 필수 문서 존재 검사

## data-format-check

목적:

- JSON/YAML/TOML 파싱 가능성 확인

초기 검사 범위:

```text
.github/workflows/
configs/
specs/
builtin-tools/
builtin-providers/
examples/
```

확장 후보:

- docs frontmatter를 도입할 경우 markdown metadata 확인
- package config file 확인

## manifest-contract-check

목적:

- Star Sentinel manifest 최소 계약 확인

확장 후보:

- provider manifest 검증
- tool command/output schema 연결
- builtin provider registry 검증

## naming-policy-check

목적:

- Star Sentinel 공식 명칭 정책 확인
- legacy alias 위치 제한
- core/tool 경계 혼동 방지

확장 후보:

- provider-neutral package naming 검사
- 특정 provider 제품명이 core package에 들어가는지 검사

## schema-example-check

목적:

- canonical example이 schema를 만족하는지 확인

확장 후보:

- provider manifest example 검증
- run artifact fixture 검증
- review pack / approval / ledger corpus 검증

## implementation-documentation-check

목적:

- 구현자가 반드시 읽어야 하는 문서와 결정 기록 존재 확인
- canonical example directory 존재 확인
- GitHub workflow와 local runner가 핵심 validator를 참조하는지 확인

## work-queue-consistency-check

목적:

- `codex-work-queue-current.md`가 현재 구현 착수 순서의 최상위 기준임을 확인
- E01~E11 EPIC heading과 handoff marker 확인
- E08/E09 split guidance와 RESERVED section 확인

## package별 CI

### Rust package 후보

도입 조건:

- Cargo workspace 도입 PR
- package 경계 확정

검사 후보:

```text
cargo fmt --check
cargo clippy --workspace --all-targets
cargo test --workspace
cargo check --workspace
```

### TypeScript package 후보

도입 조건:

- 별도 승인으로 TypeScript package manager 도입
- lockfile 정책 확정

검사 후보:

```text
format check
lint
typecheck
test
```

### Python package 후보

도입 조건:

- Python package 구조 확정
- dependency policy 확정

검사 후보:

```text
python -m compileall
lint
test
```

## test taxonomy

테스트는 다음으로 나눈다.

```text
unit tests
contract tests
schema/example tests
fixture tests
integration smoke tests
provider adapter tests
CLI tests
validation tests
security guard tests
```

## contract tests

Contract test는 schema와 문서 계약을 확인한다.

후보:

- JobSpec roundtrip
- RunState state enum
- RouteSpec assignment structure
- WorkSpec allowed_scope/forbidden_actions
- ReportSpec status mapping
- Star Sentinel output schema validation

## fixture tests

Fixture test는 policy 판단을 고정한다.

초기 fixture:

```text
scope-violation.case.yaml
dependency-approval.case.yaml
```

확장 fixture:

```text
secret-exposure.case.yaml
test-deletion.case.yaml
skip-only-ignore.case.yaml
validator-policy-change.case.yaml
workflow-permission-change.case.yaml
```

## integration smoke

초기 integration smoke 목표:

```text
1. fake project 준비
2. star-control run 실행
3. J-0001 생성
4. route.json 생성
5. fake provider output 생성
6. Star Sentinel P0 validation 실행
7. report 생성
8. terminal state 확인
```

초기에는 fake provider만 사용한다. cloud/local provider smoke는 뒤로 미룬다.

## CLI tests

CLI test 후보:

- `status` 없는 job -> error
- `run` fake provider -> J-0001 생성
- `report` -> final report 출력
- `approve` -> approval-response.json 생성
- `cancel` -> CANCELLED 전이
- `--json` output parse 가능

## CI 변경 policy

CI workflow 변경은 다음을 PR 본문에 명시해야 한다.

```text
왜 필요한가
어떤 job이 추가/변경되는가
permissions 변화가 있는가
외부 action이 추가되는가
runtime dependency가 추가되는가
실패 시 어떤 오류를 잡는가
```

금지:

- failing check 삭제
- required check 이름 바꿔 우회
- permissions 무단 상승
- secret 접근 범위 확대

## release policy

Release는 RESERVED다. release 구현 전에는 다음 문서와 계약이 필요하다.

```text
versioning policy
changelog policy
artifact signing policy
release checklist
rollback policy
package publishing policy
```

release 전 gate 후보:

```text
all required CI passed
Star Sentinel release profile passed
no open BLOCK diagnostics
no unreviewed HUMAN_REVIEW decision
changelog updated
version consistent
```

## branch protection 후보

초기 필수 check 후보:

```text
repository-policy-check
manifest-contract-check
data-format-check
naming-policy-check
schema-example-check
implementation-documentation-check
work-queue-consistency-check
```

Branch protection은 CI 안정화 후 GitHub settings에서 수동 적용한다. 이 작업은 외부 계정/저장소 설정 변경이므로 별도 승인 없이 자동으로 하지 않는다.

## performance / CI cost

CI가 무거워지면 다음을 도입한다.

- path filter
- docs-only PR에서는 package test 생략
- package별 test 분리
- quick/full profile 분리
- cache 정책 문서화

단, path filter 때문에 필수 검증이 누락되면 안 된다.

## Codex 구현 지시

Codex는 테스트 실패 시 다음 순서로 처리한다.

1. 실패 job 확인
2. 실패 step 확인
3. 실패 로그의 파일/줄/메시지 요약
4. 실제 계약 위반이면 코드 또는 문서 수정
5. 오탐이면 검사 범위를 문서화된 의도에 맞게 조정
6. 검사 삭제/약화 금지

Codex는 release/deploy/publish 관련 변경을 별도 승인 없이 구현하지 않는다.
