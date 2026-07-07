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

M6a preflight는 실제 외부 호출 전에 credential/privacy/cost artifact 계약을 적용한다. M6b cloud CLI transport는 provider instance command vector를 local fixture로 검증한다. M6c provider output conformance는 cloud provider artifact path/ref/file existence와 privacy/cost sidecar를 runtime fixture로 검증한다. M6d OpenAI-compatible parser는 Responses API와 Chat Completions response fixture를 live call 없이 정규화한다. M6e request builder는 OpenAI-compatible request URL/body fixture를 credential 없이 생성한다. M6f cloud API offline fixture integration은 prepared request와 raw response fixture parse를 같은 runtime path에서 검증한다. M6g cloud API transport boundary는 live call 없이 `http-transport-plan.json`으로 method/url/header policy/credential reference kind를 고정한다. M6h cloud API live approval gate는 explicit live request flag를 `BLOCKED` approval artifact로 정규화한다. Cloud API 실제 transport 실행은 provider 공식 문서 refresh와 별도 승인 조건을 확인한 뒤 구현한다.

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

M7a CLI control commands는 daemon/API 구현 전에 `approve`, `cancel`, `resume`의 file-based StateStore mutation과 schema-valid CLI output/error envelope을 고정한다. M7b daemon queue skeleton은 daemon process 없이 `{config_root}/daemon/state.json`과 StateStore job 참조 등록, terminal/approval guard를 고정한다. Productization daemon app slice는 `apps/star-daemon`의 `status`와 테스트 가능한 `serve --max-ticks`로 queue state를 실제 process surface에서 연다. Productization daemon scheduler tick slice는 `serve --max-ticks`가 queued `fake-default` job을 `ExecutionEngine`으로 실행하고 queue에서 제거하며, non-fake provider는 `DISABLED` scheduler result로 남겨 provider live call을 수행하지 않는다. Productization local-process scheduler slice는 queue entry의 `provider_instance_paths`를 사용해 builtin registry와 local-process instance를 로드하고 allowlisted process provider를 실행하되 Local/Cloud AI live connector는 disabled로 둔다. Productization HTTP API slice는 `star-daemon api`가 `ApiControlService` GET/POST를 loopback-only HTTP endpoint로 노출하되 remote exposure/cloud-live scheduler executor/Local·Cloud AI live connector를 disabled로 둔다. M7c API read-only service는 daemon state와 StateStore job/events/report를 `api-response` envelope으로 조회한다. M8a UI read-only view model은 이 read-only API를 소비한다. M7d API control mutation service는 `approve`, `cancel`, `resume` mutation을 `api-response` envelope과 StateStore `.ai-runs/` artifact로 고정한다.

Validation:

```text
python scripts/ci/run_all.py
cargo fmt --check
cargo check --workspace
cargo test --workspace
daemon/API smoke tests
API control mutation tests
```

## M8 UI Shell

Entry condition:

- API read-only endpoint와 approval mutation 계약이 안정화되어 있다.
- UI view model schema가 StateStore artifact를 안전하게 표현한다.

Exit criteria:

- UI가 job list, job detail, run timeline, provider output, validation result, approval request, review pack을 표시한다.
- UI는 provider process나 Star Sentinel rule을 직접 실행하지 않는다.
- approval mutation은 API/CLI 계약을 통해서만 수행한다.

M8a read-only view model은 `packages/star-control-ui`의 `UiReadOnlyShell`로 구현한다. 이 slice는 browser app이 아니라 API read-only service를 소비하는 library-level view model이며, job list/detail/timeline/provider output/validation/approval/review pack 데이터를 만들고 StateStore artifact를 직접 수정하지 않는다.

M8b browser UI shell은 `packages/star-control-ui`의 `UiBrowserShell`로 구현한다. 이 slice는 browser app이 아니라 browser-oriented library model이며 `ApiControlService`를 소비해 action panel, approve/cancel/resume result view, enable/disable reason을 만든다. Browser package manager, network server, remote exposure는 별도 승인 전까지 구현하지 않는다.

Productization static browser UI app slice는 `apps/star-control-ui`의 `index.html`, `styles.css`, `app.js`로 구현한다. 이 slice는 새 package manager 없이 `star-daemon api`를 소비해 daemon state, project jobs, job detail, timeline, release readiness, approve/cancel/resume action panel을 제공하되 provider process, Star Sentinel rule, StateStore file mutation, Local/Cloud AI live connector는 직접 실행하지 않는다.

Validation:

```text
python scripts/ci/run_all.py
cargo test -p star-control-ui -- --nocapture
UI contract tests
read-only view smoke
approval flow smoke
browser control shell smoke
```

## M9 Hardening / Conformance / Release Readiness

Entry condition:

- M1~M8이 통과하고 실제 provider 흐름이 반복 가능하다.

Exit criteria:

- provider conformance suite가 fake/local/cloud provider를 검증한다.
- secret redaction, audit, cost, privacy handoff, retention, recovery command가 안정화되어 있다.
- release readiness artifact가 생성된다.
- release/deploy/publish 자동화는 별도 approval 뒤에만 진행한다.

M9a redaction utility는 `packages/star-control-security`의 shared redaction utility와 schema-valid RedactionReport builder로 구현한다. 이 slice는 API/UI redaction helper를 통합한다. Productization E61은 `StateStore::write_redaction_report_json`으로 RedactionReport artifact 저장을 추가한다. Productization E64는 `star-control report --json` 출력 redaction과 `audit/redaction-report-<stage>.json` 자동 저장을 연결한다. Productization E65는 fake/local/cloud provider output artifact redaction과 `audit/provider-redaction-<provider>-<artifact>.json` 자동 저장을 연결한다. retention/recovery command 자동 연결, release readiness automation은 후속 slice로 남긴다.

M9b audit event writer는 `packages/star-control-observability`의 AuditEventWriter로 구현한다. 이 slice는 `.ai-runs/{job_id}/audit/audit-events.jsonl` append-only writer/readback helper, schema validation, 저장 전 redaction, job directory containment를 고정한다. API/CLI/daemon/provider event 자동 연결, cost/budget guard, retention/recovery command, release readiness automation은 후속 slice로 남긴다.

M9c cost metric budget guard는 `packages/star-control-observability`의 CostMetricWriter와 CostBudgetThresholds로 구현한다. 이 slice는 provider output sidecar `cost-metric.json` validation/write/readback, 저장 전 redaction, missing metric non-fatal path, warning-only budget evaluation을 고정한다. Productization E62는 fake/local-process provider execution path가 schema-valid cost metric sidecar를 직접 남기도록 연결한다. Productization E63은 cloud provider `budget.max_estimated_cost` hard limit 초과를 transport 실행 전 blocked result로 정규화한다. 외부 billing/quota 조회, retention/recovery command, release readiness automation은 후속 slice로 남긴다.

Productization daemon HTTP control audit integration slice는 `apps/star-daemon`의 loopback HTTP API wrapper가 approve/cancel/resume POST action을 처리한 뒤 `AuditEventWriter`에 schema-valid/redacted audit event를 append하도록 연결한다. 이 slice는 API response에 audit artifact ref 또는 누락 warning을 표시하되, provider execution, provider live call, credential raw value 접근, hard budget enforcement, retention/recovery action, release/deploy/publish automation은 수행하지 않는다.

M9d provider conformance hardening은 `packages/star-control-provider`의 ProviderConformanceChecker를 강화한다. 이 slice는 ArtifactRef path/kind/producer, stored `response.json` schema/value 일치, cloud privacy/cost sidecar schema와 job/provider/stage 일치를 검증한다. provider live call, schema field 변경, workflow/release/deploy/publish automation은 구현하지 않는다.

M9e state recovery inspection은 `packages/star-control-state`의 `StateStore::inspect_recovery`로 구현한다. 이 slice는 missing/invalid/schema/corrupt/tmp issue를 inspect-only report로 분류하되, tmp file 삭제, event log trim, recovered copy 생성, artifact 교체, retention cleanup은 수행하지 않는다.

Productization recovery action dry-run/approval slice는 `StateStore::plan_recovery_action`과 `star-control recover --action <name>`으로 구현한다. 이 slice는 tmp cleanup, recovered copy, event log trim, artifact replace, retention cleanup 계획을 dry-run으로 표시하고 dry-run 없는 action은 approval gate token과 `blocked` status로 막는다. 실제 destructive mutation executor는 action-specific 후속 slice로 남긴다.

M9f release readiness writer는 `packages/star-control-release`의 ReleaseReadinessWriter로 구현한다. 이 slice는 `.ai-runs/{job_id}/release/release-readiness.json` artifact를 생성/검증하되, `ready` status, signing, publish, deploy, repository settings 변경은 별도 승인 전까지 RESERVED로 둔다.

M9g release readiness API read는 `packages/star-control-api`의 `ApiReadOnlyService`에 `GET /projects/{project_id}/jobs/{job_id}/release-readiness` path를 추가한다. 이 slice는 existing readiness artifact를 schema-valid API envelope으로 읽어 반환하되, HTTP server, CLI command, UI app, signing, publish, deploy, repository settings 변경은 별도 승인 전까지 RESERVED로 둔다.

M9h release version consistency checker는 `packages/star-control-release`의 `ReleaseConsistencyChecker`로 구현한다. 이 slice는 caller-provided expected version, declared version text, changelog text를 평가해 `version-consistent`/`changelog-updated` checks와 blockers를 생성하되, filesystem discovery, changelog parser, release profile integration, signing, publish, deploy, repository settings 변경은 별도 승인 전까지 RESERVED로 둔다.

M9i release evidence file discovery는 `packages/star-control-release`의 `ReleaseEvidenceFileChecker`로 구현한다. 이 slice는 caller-provided project root와 relative evidence path에서 version/changelog text를 read-only로 읽어 `ReleaseConsistencyChecker`에 연결하되, automatic repository-wide scan, changelog parser, release profile integration, signing, publish, deploy, repository settings 변경은 별도 승인 전까지 RESERVED로 둔다.

M9j release profile readiness integration은 `packages/star-control-release`의 `ReleaseProfileValidation`과 `ReleaseProfileReadinessBuilder`로 구현한다. 이 slice는 caller-provided release profile pass/fail result를 `release-profile-passed` check로 만들고 version/changelog consistency checks와 병합하되, Star Sentinel profile evaluator, CLI/API/UI surface, schema field 변경, signing, publish, deploy, repository settings 변경은 별도 승인 전까지 RESERVED로 둔다.

M9k release readiness UI read는 `packages/star-control-ui`의 `UiReadOnlyShell`에 release readiness viewer를 추가한다. 이 slice는 existing ReleaseReadiness artifact를 API read-only endpoint를 통해 표시하고 missing artifact를 optional read-only error로 처리하되, browser app, HTTP server, CLI command, StateStore 직접 mutation, signing, publish, deploy, repository settings 변경은 별도 승인 전까지 RESERVED로 둔다.

M9l release readiness CLI read는 `packages/star-control-cli`의 `report --release-readiness` option으로 구현한다. 이 slice는 existing ReleaseReadiness artifact를 schema-valid CLI output envelope으로 읽고 missing artifact를 schema-valid error로 반환하되, 새 top-level command, StateStore mutation, signing, publish, deploy, repository settings 변경은 별도 승인 전까지 RESERVED로 둔다.

M9m release review pack foundation은 `packages/star-control-release`의 `ReleaseReviewPackWriter`로 구현한다. 이 slice는 existing ReleaseReadiness value를 검증한 뒤 `.ai-runs/{job_id}/review-packs/release-review-pack.md` Markdown artifact를 한 번만 쓰되, approval record, CLI/API/UI surface, signing, publish, deploy, repository settings 변경은 별도 승인 전까지 RESERVED로 둔다.

Productization release automation dry-run/approval slice는 `packages/star-control-release`의 `ReleaseAutomationPlanner`와 `star-control release --action <name>`으로 구현한다. 이 slice는 signing policy, package publish, deploy, rollback checklist, approval record, release review pack 준비 단계를 dry-run으로 표시하고 dry-run 없는 action은 approval gate token과 `blocked` status로 막는다. 실제 signing/publish/deploy/repository settings mutation executor는 별도 승인된 후속 slice로 남긴다.

M9n recovery command surface는 `packages/star-control-cli`의 `recover --list`로 구현한다. 이 slice는 `StateStore::inspect_recovery` 결과를 schema-valid CLI envelope으로 표시하되, tmp file 삭제, event log trim, recovered copy 생성, artifact 교체, retention cleanup은 별도 승인 전까지 RESERVED로 둔다.

M9o final M9 readiness audit은 `packages/star-control-release`의 `M9ReadinessAuditBuilder`로 구현한다. 이 slice는 M9 필수 hardening/recovery/release-readiness check를 `release-readiness.schema.json` value로 조립하고 missing/duplicate/failed check를 blocker로 표시하되, all-pass 결과도 `ready`가 아니라 final release/deploy/publish reserved blocker가 있는 `reserved` status로 둔다.

M9p final completion audit은 `packages/star-control-release`의 `CompleteImplementationAuditBuilder`로 구현한다. 이 slice는 M0~M9 milestone, full local validation, remote CI evidence, stacked PR clean state, reserved action confirmation을 `release-readiness.schema.json` value로 조립하고 missing/duplicate/failed check를 blocker로 표시하되, all-pass 결과도 `ready`가 아니라 release/deploy/publish와 external repository settings reserved blocker가 있는 `reserved` status로 둔다.

M9q final audit evidence는 `examples/release-contracts/complete-implementation-readiness.example.json`과 `docs/implementation/audit/final-completion-audit.md`로 구현한다. 이 slice는 M0~M9 completion audit evidence를 schema-valid ReleaseReadiness example과 human-readable audit 문서로 고정하되, all-pass evidence도 `ready`가 아니라 release/deploy/publish와 external repository settings reserved blocker가 있는 `reserved` status로 둔다.

M9r stacked PR readiness coordination은 `examples/release-contracts/stacked-pr-readiness.example.json`과 `docs/implementation/audit/stacked-pr-readiness.md`로 구현한다. 이 slice는 stacked PR chain의 contiguous base/head, clean merge state, draft review gate, main merge not performed, final audit evidence link를 schema-valid ReleaseReadiness example과 human-readable audit 문서로 고정하되, main update나 PR merge는 별도 승인 전까지 수행하지 않는다.

M9s CLI providers read-only surface는 `packages/star-control-cli`의 `providers list/show`와 `packages/star-control-provider`의 read-only listing accessor로 구현한다. 이 slice는 public CLI surface에 남아 있던 provider discovery gap을 채우되, provider healthcheck, provider execution, live call, credential raw value 출력, schema field 변경, workflow 변경, release/deploy/publish는 수행하지 않는다.

M9s 후속 productization readiness slice는 `packages/star-control-cli`의 `providers healthcheck`를 live call 없는 offline readiness surface로 구현한다. 이 slice는 manifest/capability presence와 provider kind별 readiness class를 schema-valid CLI envelope으로 반환하고 `live_calls_performed=false`, Local AI live connector disabled, Cloud AI live connector disabled를 명시한다. Provider execution, network/process probe, credential raw value 접근, Local/Cloud AI live connector 구현은 수행하지 않는다.

M9t CLI sentinel command group은 `packages/star-control-cli`의 `sentinel selfcheck/check/gate/review-pack`으로 구현한다. 이 slice는 Star Sentinel rule engine을 CLI에 재구현하지 않고 `packages/star-sentinel` API를 호출해 existing `.ai-runs/{job_id}/tool-output/star-sentinel/{task.json,changed_lines.json}` input을 평가하고 diagnostics, approval, review-pack artifact를 쓴다. Provider execution, provider live call, release/deploy/publish, destructive recovery action, schema field 변경, workflow 변경은 수행하지 않는다.

M9u final evidence refresh는 `docs/implementation/audit/final-completion-audit.md`, `docs/implementation/audit/stacked-pr-readiness.md`, `examples/release-contracts/*readiness.example.json`을 M9t 구현 스택 기준으로 갱신한다. 이 slice는 evidence refresh만 수행하고 PR merge, main update, release/deploy/publish, repository settings 변경, destructive recovery action은 수행하지 않는다.

M9v stacked merge procedure는 `docs/implementation/audit/stacked-pr-merge-procedure.md`로 review order, branch-to-branch merge execution order, pre-merge verification, stop condition, explicit approval phrase를 문서화한다. 이 slice는 절차만 고정하고 PR ready/merge, main update, release/deploy/publish, repository settings 변경, destructive recovery action은 수행하지 않는다.

Validation:

```text
python scripts/ci/run_all.py
cargo fmt --check
cargo check --workspace
cargo test --workspace
provider conformance suite
security guard tests
redaction report tests
audit event writer tests
cost metric budget guard tests
provider conformance hardening tests
state recovery inspection tests
recovery command surface tests
release readiness checks
release readiness writer tests
release review pack writer tests
final M9 readiness audit tests
final completion audit tests
final completion readiness example validation
stacked PR readiness example validation
CLI providers list/show tests
CLI sentinel command group tests
final evidence refresh validation
stacked merge procedure validation
```

## 다음 작업 선택 규칙

- 현재 구현 착수는 E01부터 시작한다.
- E01~E11을 완료하기 전에는 M5 이후 작업을 앞당기지 않는다.
- 단, 문서나 schema가 M5 이후 boundary를 설명하는 것은 허용한다.
- provider 공식 문서가 최신성에 민감하면 adapter 구현 직전에 다시 확인한다.
- 외부 계정, release, deploy, package registry, GitHub settings 변경은 별도 승인 전까지 실행하지 않는다.
