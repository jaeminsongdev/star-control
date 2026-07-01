# CI 운영 로드맵

## 현재 적용된 CI

현재 `main`에는 구현 전 계약 고정 단계의 최소 안정 CI를 적용한다.

```text
repository-policy-check
data-format-check
manifest-contract-check
naming-policy-check
schema-example-check
implementation-documentation-check
work-queue-consistency-check
```

로컬에서는 다음 명령으로 동일한 계약 검사를 한 번에 실행한다.

```text
python scripts/ci/run_all.py
```

현재 단계의 목적은 AI 작업 PR에 대해 낮은 비용의 자동 검증선을 제공하면서, schema/example/doc/work queue drift를 조기에 잡는 것이다.

## 완전 구현 milestone별 CI 확장

완전 구현 milestone은 `docs/implementation/complete-implementation-roadmap.md`를 따른다.

| milestone | CI 확장 기준 |
|---|---|
| M0 문서와 결정 정렬 | 현재 contract checks와 `git diff --check`를 유지한다. |
| M1 Runtime Foundation | Cargo workspace가 생기면 Rust fmt/check/test를 추가한다. |
| M2 Provider-neutral Execution | provider registry, fake provider, router, execution unit/contract test를 추가한다. |
| M3 Validation / Gate | Star Sentinel P0 fixture와 ValidationEngine decision mapping test를 추가한다. |
| M4 v0 Fake E2E | fake provider integration smoke를 추가한다. |
| M5 Local Provider | command policy, timeout/cancel, sandbox, stdout/stderr capture test를 추가한다. |
| M6 Cloud Provider | provider conformance, artifact path/ref/file existence, provider request/response fixture, cloud API offline fixture runtime, transport plan artifact, live approval gate artifact/state, credential reference, budget/cost, privacy handoff test를 추가한다. |
| M7 Daemon / API | CLI approve/cancel/resume regression, daemon queue skeleton test, API read-only service test, in-process API approve/cancel/resume mutation test, daemon/API smoke와 resume/cancel/approval regression test를 추가한다. |
| M8 UI Shell | `star-control-ui` view model contract, read-only no-write smoke, approval path smoke, browser control shell smoke를 추가한다. |
| M9 Hardening / Release Readiness | redaction utility/report tests, audit event writer tests, cost metric budget guard tests, provider conformance hardening tests, state recovery inspection tests, release readiness writer tests, release readiness API read tests, release version consistency checker tests, release evidence file discovery tests, release profile readiness integration tests, release readiness UI read tests, security guard, provider conformance suite, release readiness checks를 추가한다. |

각 단계의 CI 추가는 실패 검사를 삭제하거나 약화하지 않는 별도 PR로 진행한다.

현재 M0/M1 진입 전 검증은 문서, schema, manifest, work queue drift를 잡는 낮은 비용의 계약 검사로 유지한다. Rust package, provider smoke, daemon/API/UI, 보안 guard처럼 시간이 커지는 검사는 해당 milestone의 구현물이 생긴 뒤 추가하고, 누적 비용이 커지면 quick/full profile로 분리한다.

## 1단계: 데이터 형식 검사

정본 경로 기준으로 데이터 파일 파싱 검사를 유지한다.

초기 검사 범위:

- `.github/workflows/`
- `configs/`
- `specs/`
- `builtin-tools/`
- `builtin-providers/`
- `examples/`

검사 대상은 JSON, YAML, TOML이다.

## 2단계: 문서 품질 검사

설계 문서가 늘어나면 문서 전용 검사를 추가한다.

- 빈 링크 검사
- 내부 상대 링크 검사
- 문서 제목 중복 검사
- 문서 읽는 순서 문서와 실제 파일 존재 여부 비교
- 정본 문서와 임시 작업 산출물의 혼동 방지 검사

현재 `implementation-documentation-check`는 필수 문서와 canonical example directory 존재 여부, workflow/local runner wiring을 확인한다.

## 3단계: 명칭 정책 검사

Star-Control은 명칭과 package 경계가 중요하므로 별도 검사를 둔다.

- Star Sentinel 정식 명칭 사용 여부
- legacy alias 사용 위치 제한
- provider-neutral package 경계 확인
- core package 이름에 특정 provider 제품명이 들어가지 않는지 확인
- builtin provider manifest와 core package의 책임 경계 확인

## 4단계: 스키마 검증

`specs/`와 canonical example이 안정되면 schema 기반 검사를 계속 확장한다.

- schema 파일 자체의 파싱 가능 여부
- manifest 예시가 schema를 만족하는지 확인
- provider manifest 검증
- tool manifest 검증
- capability registry 검증
- run ledger, approval, review pack 관련 산출물 schema 검증
- schema coverage 검사 후보

## 5단계: 구현 패키지 생성 후 언어별 검사

`packages/` 아래 실제 구현이 생기면 언어별 CI를 추가한다.

Rust + Cargo workspace가 생기면 다음 검사를 추가한다.

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets`
- `cargo test --workspace`
- `cargo check --workspace`

초기 Cargo workspace는 `star-control-*` core crate와 `packages/star-sentinel`을 기준으로 한다. `star-provider-*`, `star-transport-*`, `star-adapter-*` extension package 검사는 provider 확장 milestone에서 추가한다.

TypeScript 또는 Python package는 별도 승인으로 package manager와 dependency policy가 확정된 뒤 추가한다.

## 6단계: Star Sentinel selfcheck

Star Sentinel 구현이 생기면 자체 검증 명령을 CI에 연결한다.

- quick profile selfcheck
- quick profile check
- policy corpus 검사
- approval gate 판정 샘플 검사
- review pack 생성 샘플 검사

## 7단계: PR 보호 설정

초기 CI가 안정적으로 통과한 뒤 `main`에 보호 규칙을 건다.

- PR 없이 merge 금지
- 필수 status check 통과 전 merge 금지
- 대화 해결 전 merge 금지
- 강제 push 금지
- branch 삭제 금지

초기 필수 status check 후보는 다음과 같다.

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

## 8단계: 보안 및 운영 정책 검사

초기에는 오탐을 줄이기 위해 강한 정책 검사를 넣지 않는다. 이후 별도 PR로 다음 검사를 추가한다.

- 민감정보 포함 여부 검사
- 실행 산출물 위치 검사
- workflow 변경 위험도 검사
- 외부 action 사용 정책 검사
- 권한 상승 가능성이 있는 workflow 패턴 검사
- 의존성 추가 여부 검사

## 9단계: 비용과 시간 최적화

CI가 무거워지면 비용과 시간을 줄이는 정책을 추가한다.

- 변경 경로 기반 job 실행
- 문서만 바뀐 PR에서는 코드 테스트 생략
- 코드가 바뀐 PR에서는 관련 package 테스트 실행
- 캐시 사용 기준 문서화
- 긴 테스트와 빠른 테스트 분리

## 운영 원칙

- CI는 검증자이고 구현자가 아니다.
- CI workflow 기본 권한은 읽기 중심으로 유지한다.
- 초기 CI에서는 배포나 공개 작업을 하지 않는다.
- Codex 또는 다른 AI가 CI를 수정하는 PR은 고위험 변경으로 본다.
- 실패한 검사를 삭제하거나 약화해서 통과시키지 않는다.
- 단계별로 작은 PR을 만들어 안정화한 뒤 필수 status check로 승격한다.
