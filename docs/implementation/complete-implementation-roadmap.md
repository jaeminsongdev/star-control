# Complete Implementation Roadmap

## 목적

이 문서는 Star-Control의 완전 구현 마일스톤을 고정한다. `codex-work-queue-current.md`의 E01~E11은 v0 fake flow를 완성하기 위한 현재 착수 큐이며, 이 문서는 그 이후 local/cloud provider, daemon/API/UI, 운영 안정화까지 이어지는 전체 경로를 설명한다.

작업 착수 순서는 항상 `codex-work-queue-current.md`를 우선한다. 이 문서는 목표 지점과 다음 확장 순서를 판단하는 기준이다.

## 공통 완료 기준

모든 마일스톤은 다음을 만족해야 한다.

- schema/example/manifest 계약을 약화하지 않는다.
- 실행 산출물은 대상 프로젝트 `.ai-runs/` 아래에 둔다.
- provider 제품명을 core crate 이름에 넣지 않는다.
- approval-required action은 자동 진행하지 않는다.
- `python scripts/ci/run_all.py`를 통과한다.
- Cargo workspace가 생긴 뒤에는 `cargo fmt --check`, `cargo check --workspace`, `cargo test --workspace`를 함께 통과한다.

## M0 문서와 결정 정렬

Entry condition:

- repository가 스캐폴드와 설계 문서 상태다.
- v0 runtime stack, fake provider instance, Star Sentinel P0 scope가 결정되어 있다.

Exit criteria:

- 완전 구현 기본값이 `docs/decisions/0005-full-implementation-defaults.md`에 기록되어 있다.
- `README.md`, `PLANS.md`, implementation README, repository layout, roadmap, runbook이 같은 package/provider/surface 순서를 가리킨다.
- v0 current queue는 유지되고, v0 이후 확장 경로가 별도 문서로 분리되어 있다.

Validation:

```text
python scripts/ci/run_all.py
git diff --check
```

## M1 Runtime Foundation

대응 current queue:

```text
E01 Schema / Runtime Validator
E02 File-based StateStore
E03 Artifact Layout Writer
```

Entry condition:

- `star-control-*` core crate naming을 따른다.
- Cargo workspace baseline 추가가 허용된 구현 PR이다.

Exit criteria:

- runtime schema validator가 canonical examples를 검증한다.
- StateStore가 `job.json`, `run-state.json`, `events.jsonl`을 읽고 쓴다.
- artifact path helper가 provider-output, tool-output, approvals, review-packs, tmp 경로를 job directory 내부로 제한한다.

Validation:

```text
python scripts/ci/run_all.py
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

## M2 Provider-neutral Execution

대응 current queue:

```text
E04 Provider Registry
E05 FakeProviderAdapter
E06 RouterEngine
E07 ExecutionEngine
E08 CLI read-only + fake run
```

Entry condition:

- M1 StateStore와 schema validator API가 안정화되어 있다.
- `fake-default` provider instance 기준을 따른다.

Exit criteria:

- provider registry가 manifest, instance, capability profile을 조회한다.
- FakeProviderAdapter가 deterministic `ProviderRunResult`를 생성한다.
- RouterEngine이 deterministic RouteSpec과 WorkSpec metadata를 만든다.
- ExecutionEngine이 WorkSpec을 FakeProviderAdapter와 연결하고 provider output을 저장한다.
- CLI `run`, `status`, `report`가 fake project에서 동작한다.

Validation:

```text
python scripts/ci/run_all.py
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

## M3 Validation / Gate

대응 current queue:

```text
E09 Star Sentinel P0
E10 ValidationEngine
```

Entry condition:

- fake provider output과 changed file 후보를 validation input으로 연결할 수 있다.
- Star Sentinel P0 scope는 5개 rule로 제한한다.

Exit criteria:

- Star Sentinel P0가 scope, test deletion, dependency approval, secret, validator self-bypass rule을 평가한다.
- diagnostics, gate decision, review pack, ledger/selfcheck가 E09 split 기준에 맞게 구현된다.
- ValidationEngine이 `AUTO_PASS`, `HUMAN_REVIEW`, `BLOCK`, invalid output을 RunState로 mapping한다.
- approval response 없이 `WAITING_APPROVAL`에서 다음 stage로 진행하지 않는다.

Validation:

```text
python scripts/ci/run_all.py
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

## M4 v0 Fake E2E

대응 current queue:

```text
E11 Integration Smoke
```

Entry condition:

- M1~M3가 통과했다.
- CLI fake run, Star Sentinel P0, ValidationEngine이 연결되어 있다.

Exit criteria:

- fake project에서 `route -> execute -> validate -> report` 흐름이 반복 가능하다.
- `AUTO_PASS`, `HUMAN_REVIEW`, `BLOCK` smoke가 통과한다.
- terminal state와 final report가 확인된다.
- local/cloud provider 확장 전 남은 approval 필요 항목과 위험이 보고된다.

Validation:

```text
python scripts/ci/run_all.py
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

## M5 Local Provider

Entry condition:

- M4 fake flow가 안정화되어 있다.
- command policy, sandbox policy, timeout/cancel behavior가 문서화되어 있다.
- 기준 문서: `docs/implementation/local-process-provider-policy.md`

Exit criteria:

- `local_process` provider가 허용된 command만 실행한다.
- stdout/stderr/log가 provider output directory에 저장된다.
- timeout, cancel, forbidden action guard가 동작한다.
- local OpenAI-compatible/local server adapter는 provider 공식 문서 refresh 후 구현된다.

Validation:

```text
python scripts/ci/run_all.py
cargo fmt --check
cargo check --workspace
cargo test --workspace
local provider contract tests
```

## M6 Cloud CLI / Cloud API Provider

Entry condition:

- M5 local provider와 provider conformance fixture가 안정화되어 있다.
- credential reference policy와 budget/cost metric 계약이 적용되어 있다.
- 기준 문서: `docs/implementation/cloud-provider-policy.md`

Exit criteria:

- cloud CLI provider는 process/stdio/file handoff 중 선택한 transport로 실행된다.
- cloud API provider는 credential raw value 없이 `credential_ref`로만 동작한다.
- provider별 parser와 conformance fixture가 있다.
- budget, cost, rate limit, privacy handoff가 report에 반영된다.

M6a preflight는 실제 외부 호출 전에 credential/privacy/cost artifact 계약을 적용한다. M6b cloud CLI transport는 provider instance command vector를 local fixture로 검증한다. M6c provider output conformance는 cloud provider artifact path/ref/file existence와 privacy/cost sidecar를 runtime fixture로 검증한다. M6d OpenAI-compatible parser는 Responses API와 Chat Completions response fixture를 live call 없이 정규화한다. M6e request builder는 OpenAI-compatible request URL/body fixture를 credential 없이 생성한다. M6f cloud API offline fixture integration은 prepared request와 raw response fixture parse를 같은 runtime path에서 검증한다. M6g cloud API transport boundary는 live call 없이 `http-transport-plan.json`으로 method/url/header policy/credential reference kind를 고정한다. Cloud API 실제 transport 실행은 별도 M6 slice에서 provider 공식 문서 refresh와 승인 조건을 확인한 뒤 구현한다.

Validation:

```text
python scripts/ci/run_all.py
cargo fmt --check
cargo check --workspace
cargo test --workspace
provider conformance tests
```

## M7 Daemon / API Control Plane

Entry condition:

- CLI run/status/report/approve/cancel/resume이 안정화되어 있다.
- StateStore resume/cancel precondition이 검증되어 있다.

Exit criteria:

- daemon이 장시간 queue, resume, cancel, provider session을 관리한다.
- API는 read-only endpoint부터 시작하고, mutation은 approval/cancel/resume 계약을 따른다.
- daemon state는 repository root가 아니라 user config/cache 영역에 둔다.

Validation:

```text
python scripts/ci/run_all.py
cargo fmt --check
cargo check --workspace
cargo test --workspace
daemon/API smoke tests
```

## M8 UI Shell

Entry condition:

- API read-only endpoint와 approval mutation 계약이 안정화되어 있다.
- UI view model schema가 StateStore artifact를 안전하게 표현한다.

Exit criteria:

- UI가 job list, job detail, run timeline, provider output, validation result, approval request, review pack을 표시한다.
- UI는 provider process나 Star Sentinel rule을 직접 실행하지 않는다.
- approval mutation은 API/CLI 계약을 통해서만 수행한다.

Validation:

```text
python scripts/ci/run_all.py
UI contract tests
read-only view smoke
approval flow smoke
```

## M9 Hardening / Conformance / Release Readiness

Entry condition:

- M1~M8이 통과하고 실제 provider 흐름이 반복 가능하다.

Exit criteria:

- provider conformance suite가 fake/local/cloud provider를 검증한다.
- secret redaction, audit, cost, privacy handoff, retention, recovery command가 안정화되어 있다.
- release readiness artifact가 생성된다.
- release/deploy/publish 자동화는 별도 approval 뒤에만 진행한다.

Validation:

```text
python scripts/ci/run_all.py
cargo fmt --check
cargo check --workspace
cargo test --workspace
provider conformance suite
security guard tests
release readiness checks
```

## 다음 작업 선택 규칙

- 현재 구현 착수는 E01부터 시작한다.
- E01~E11을 완료하기 전에는 M5 이후 작업을 앞당기지 않는다.
- 단, 문서나 schema가 M5 이후 boundary를 설명하는 것은 허용한다.
- provider 공식 문서가 최신성에 민감하면 adapter 구현 직전에 다시 확인한다.
- 외부 계정, release, deploy, package registry, GitHub settings 변경은 별도 승인 전까지 실행하지 않는다.
