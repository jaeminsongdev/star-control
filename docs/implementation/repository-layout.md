# Repository Layout

## 목적

이 문서는 Star-Control repository의 목표 구조와 package 경계를 정의한다. 현재 repository는 스캐폴드와 설계 문서 단계이므로 모든 package가 즉시 구현되어야 하는 것은 아니다. Codex가 장시간 구현을 진행할 때 책임 경계를 임의로 섞지 않도록 최종 구조를 먼저 고정한다.

현재 실제 경로의 상태는 `current-repository-map.md`를 우선 확인한다. 이 문서는 목표 구조와 package 책임을 설명한다.

리팩토링 중 구조 이동과 하드코딩 판정은 `refactoring-hardcoding-guidelines.md`를 함께 따른다. 특히 실제 Cargo workspace crate와 README-only scaffold를 분리하되, path rename, dependency 추가, workflow 변경은 별도 slice로 다룬다.

실제 구현 착수 순서는 `codex-work-queue-current.md`를 우선한다. 이 문서의 package 순서나 장기 구조 설명이 현재 작업 큐와 다르게 보이면, 현재 착수 큐인 `codex-work-queue-current.md`를 기준으로 한다.

## 현재 정본 경로

```text
README.md
AGENTS.md
.github/workflows/
docs/
specs/
configs/
builtin-providers/
builtin-tools/star-sentinel/
examples/
scripts/ci/
```

현재 scaffold 또는 reserved 경로:

```text
apps/
packages/
integrations/
```

- `apps/`는 CLI, daemon, UI entrypoint 후보를 표시하는 scaffold다.
- `packages/`는 목표 구현 package 경계다.
- `integrations/`는 장기 연동 산출물 후보이며 초기 구현 대상이 아니다.

## 목표 package 경계

```text
packages/
  star-control-core/
  star-control-state/
  star-control-schema/
  star-control-router/
  star-control-execution/
  star-control-provider/
  star-control-validation/
  star-control-report/
  star-control-cli/
  star-control-daemon/
  star-control-api/
  star-control-ui/
  star-control-security/
  star-control-release/
  star-sentinel/
```

위 구조는 목표 core 구조다. package manager 도입 전에는 문서와 스캐폴드만 둘 수 있다. Core runtime crate namespace는 `docs/decisions/0005-full-implementation-defaults.md`에 따라 `star-control-*`로 통일한다.

## provider / transport / adapter extension 경계

기존 scaffold 중 아래 계열은 core namespace가 아니라 core 안정화 이후 확장 package 후보로 분류한다.

```text
packages/star-provider-api
packages/star-provider-host
packages/star-transport-cli
packages/star-transport-http
packages/star-transport-process
packages/star-adapter-code-agent
packages/star-adapter-chat-model
packages/star-adapter-openai-compatible
```

해당 package는 provider manifest, provider instance, capability profile, ProviderAdapter interface가 안정화된 뒤 실제 구현 대상으로 삼는다. E01~E11 core fake flow를 구현할 때 `star-control-provider`를 대체하지 않는다.

## package 책임

### `star-control-core`

- job lifecycle 조정
- module orchestration
- state transition 정책 적용
- provider/tool 직접 구현 금지

### `star-control-state`

- file-based StateStore
- StateStore public type and contract constants in `src/types.rs` and `src/constants.rs`
- StateStoreError public error boundary
- StateStoreError Display and source mapping in `src/error/`
- StateStore path validation and job directory containment helper
- StateStore module root in `src/store.rs`
- StateStore lifecycle/open/getter helper in `src/store/lifecycle.rs`
- StateStore job allocation/create/list/resume flow in `src/store/jobs.rs`
- StateStore core artifact save/load helper in `src/store/core_artifacts.rs`
- StateStore job-relative path resolution in `src/store/paths.rs`
- event append/read in `src/events.rs`
- provider/tool/approval/review-pack/validation/tmp output helper root in `src/outputs.rs`
- artifact ref/register, provider output, tool output, approval, review-pack, validation, and tmp writers in `src/outputs/`
- artifact helper root in `src/artifacts.rs`
- atomic write, JSON/text artifact IO, platform replace, schema validation, and timestamp helpers in `src/artifacts/`
- inspect-only recovery model root in `src/recovery.rs`
- inspect-only recovery report, issue model/error mapping, job summary, and tmp artifact warning modules in `src/recovery/`
- unit test module root and shared fixture helper in `src/tests.rs`
- store test root in `src/tests/store.rs`
- store job/event/path scenario tests in `src/tests/store/`
- inspect-only recovery tests in `src/tests/recovery.rs`
- artifact/output writer test root in `src/tests/artifacts.rs`
- StateStore artifact output-dir, ref registration, path-safety, and writer scenario tests in `src/tests/artifacts/`
- job.json, run-state.json, events.jsonl 관리
- atomic write와 append 규칙
- Star-Control repository 내부 `.ai-runs` 사용 금지
- inspect-only recovery report for missing/corrupt/tmp artifacts

### `star-control-schema`

- JSON schema loading
- schema subset validation
- schema validation error model
- schema public error boundary in `src/error.rs`
- Schema and Validation DTOs in `src/types.rs`
- schema/document loader and file validation helpers in `src/loader.rs`
- subset validator root plus scalar, object/array, path, and known-pattern helper modules in `src/validator.rs` and `src/validator/`
- validator unit test root in `src/tests.rs`
- schema test fixture/temp file helpers and keyword, structure, loading scenario tests in `src/tests/`
- canonical schema/example integration test runner in `tests/canonical_examples.rs`
- canonical schema/example case registry in `tests/canonical_examples/cases.rs`
- core, provider/execution, surface/security/release, config, and Star Sentinel validation case groups in `tests/canonical_examples/cases/`
- 외부 `jsonschema` dependency 도입은 승인 전 금지

### `star-control-router`

- request 분석
- size/risk/stage 산출
- provider assignment
- approval 필요 여부 판단
- public root in `src/lib.rs`
- RouterEngine orchestration in `src/engine.rs`
- router contract constants in `src/constants.rs`
- schema validation and field extraction helpers in `src/contract.rs`
- public error boundary, public job/route/workspec/output types, unit tests in separate modules
- request analysis module root in `src/analysis.rs`
- analysis type re-export root in `src/analysis/types.rs`
- change type, policy/route decision, request analysis, size/risk scale, and stage selection helpers in `src/analysis/types/`
- request keyword classification orchestration in `src/analysis/classification.rs`
- normalized haystack, keyword matching helper, keyword rule catalog root, and routine/safety rule family modules in `src/analysis/classification/`
- size/risk/profile/approval policy orchestration in `src/analysis/policy.rs`
- change-type policy mapping helpers in `src/analysis/policy/`
- workspec facade in `src/workspec.rs`
- workspec stage JSON builder, route assignment/path map builder, stage role helper, and workspec artifact path helper in `src/workspec/`
- unit test module root in `src/tests.rs`
- RouterEngine scenario tests and shared helpers in `src/tests/scenarios.rs` and `src/tests/helpers.rs`

### `star-control-execution`

- WorkSpec 실행
- ProviderAdapter 호출
- timeout, cancel, retry 처리
- provider output 저장
- public error enum root plus Display/source helpers split across `src/error.rs` and `src/error/`
- public outcome, contract/schema helper, run-state/status helper, orchestration root, provider dispatch, execution request/stage guard, and state/event write helper split across `src/types.rs`, `src/contract.rs`, `src/state.rs`, `src/engine.rs`, and `src/engine/`
- provider integration/unit test module root and child-process helper in `src/tests.rs`
- fake provider execution tests in `src/tests/fake.rs`
- local process provider execution test root in `src/tests/local_process.rs`
- local process execution, timeout, cancellation, forbidden-action, and conformance scenario tests in `src/tests/local_process/`
- cloud provider execution test root in `src/tests/cloud.rs`
- cloud CLI, offline API fixture, and live approval-required scenario tests in `src/tests/cloud/`
- provider integration/unit test support facade in `src/test_support.rs`
- execution temp/env/path helper in `src/test_support/helpers.rs`
- execution fixture lifecycle and route setup in `src/test_support/fixture.rs`
- execution cloud/local-process fixture registry and workspec helpers in `src/test_support/fixture/`
- local-process conformance runner in `src/test_support/local_process.rs`
- local-process output contract assertions in `src/test_support/local_process/`

### `star-control-provider`

- provider registry
- provider manifest loading
- provider instance loading
- provider registry domain, public error boundary, loader/path guard/schema validation, YAML subset parser root/line scanner/block parser/scalar parser modules
- ProviderAdapter interface
- fake, human, local, cloud provider adapter 경계
- fake provider adapter execution flow in `src/fake/adapter.rs`
- fake provider simulation state and payload helpers in `src/fake/simulation.rs`
- fake execution request/result/error/context model root in `src/fake/model.rs`
- fake provider model error root, error Display/source helpers, request/result/execution/validation modules in `src/fake/model/`
- fake provider output path and overwrite guard helpers in `src/fake/output.rs`
- fake provider scenario tests in `src/fake/tests.rs` and fixture/request/temp-store helpers in `src/fake/tests/`
- ProviderConformanceChecker result/ref/file/schema consistency hardening
- ProviderConformanceChecker orchestration in `src/conformance/checker.rs`
- ProviderConformanceChecker checked artifact collection root in `src/conformance/checker/artifacts.rs`
- ProviderConformanceChecker cloud-required, declared artifact, optional stderr, and file-existence helpers in `src/conformance/checker/artifacts/`
- ProviderConformanceChecker stored response and cloud sidecar artifact validation helper in `src/conformance/checker/stored.rs`
- ProviderConformanceError public error boundary in `src/conformance/error.rs`
- ProviderConformanceProfile and ProviderConformanceReport model in `src/conformance/types.rs`
- provider conformance helper root in `src/conformance/helpers.rs`
- provider conformance artifact list, JSON field/ref contract, path policy, and schema-backed artifact validation helpers in `src/conformance/helpers/`
- ProviderConformanceChecker unit test root in `src/conformance/tests.rs`
- ProviderConformanceChecker path policy, response consistency, cloud sidecar schema, and shared fixture tests in `src/conformance/tests/`
- local process provider contract constants, command policy root, command policy field/timeout parser, executable allow/deny checker, process runner, forbidden-action evidence, and sidecar facade modules
- local process sidecar response JSON builder, planned output/artifact ref helper, and output file creation helper in `src/local_process/sidecars/`
- local process provider shared test root and child-process helpers in `src/local_process/tests.rs`
- local process provider execution/policy/cancellation/forbidden-action scenario tests in `src/local_process/tests/`
- local process provider test support facade in `src/local_process/tests/support.rs`
- local process provider env guard, execute-with-command fixture, registry builder, request/run-state fixture, and temp project/schema/store helpers in `src/local_process/tests/support/`
- provider registry loader facade in `src/registry_loader.rs`
- provider registry document loaders, registry assembly, contract IO/schema validation, field extraction, and path guard helpers in `src/registry_loader/`
- provider registry domain root plus manifest, instance, capability, registry document, and registry collection modules in `src/registry_domain/`
- provider registry public error enum in `src/registry_error.rs` and Display/source helper impls in `src/registry_error/`
- provider registry YAML subset parser root in `src/registry_yaml.rs` and block, line, pair, parser, scalar helpers in `src/registry_yaml/`
- provider registry test module root in `src/registry_tests.rs`
- provider registry load contract, error/path/schema guard, and YAML parser scenario tests in `src/registry_tests/`
- provider registry loader/temp JSON fixture helpers in `src/registry_tests/`
- cloud provider contract constants and policy decision modules
- cloud provider credential-ref/raw-credential and JSON value policy helpers in `src/cloud_policy/`
- cloud CLI transport root in `src/cloud_cli.rs`
- cloud CLI command field parser, command policy/timeout/env passthrough guard, process runner/timeout wait, and argument renderer modules in `src/cloud_cli/`
- cloud API artifact root and response/transport/stdout/naming helper modules
- cloud provider sidecar root in `src/cloud_sidecars.rs`
- cloud provider response/privacy/cost/stdout/stderr/artifact-ref sidecar helper modules in `src/cloud_sidecars/`
- cloud provider CLI and preflight response builders in `src/cloud_sidecars/response/`
- cloud provider fixture path resolution, project-relative path guard, schema validation IO helper module
- cloud provider manifest classifier, CLI adapter, API offline adapter, live-approval handoff helper, and preflight adapter modules in `src/cloud/`
- cloud API offline adapter facade in `src/cloud/api_offline_adapter.rs`
- cloud API offline fixture request/response preparation and provider output write orchestration helpers in `src/cloud/api_offline_adapter/`
- cloud API live approval-required execution root in `src/cloud/api_live.rs`
- cloud API live request/transport/approval artifact preparation, plan artifact writer, and privacy/cost/stdout/stderr sidecar writer helpers in `src/cloud/api_live/`
- cloud provider request/response artifact file names are owned by `src/cloud_constants.rs`
- cloud provider test root and child-process helper in `src/cloud/tests.rs`
- cloud provider preflight, API, and CLI transport scenario tests in `src/cloud/tests/`
- cloud API offline fixture, live approval, and unsafe fixture path scenario tests in `src/cloud/tests/api/`
- cloud provider test support facade in `src/cloud/test_support.rs`
- cloud provider env guard, executor fixture, JSON reader, registry builder, request fixture, and temp/schema path helpers in `src/cloud/test_support/`
- OpenAI-compatible request builder root, request body builder, request error, endpoint/API field helper, response parser root, Chat Completions parser, Responses API parser, shared response field extraction helper, test root, request tests, and response tests in `src/openai_compatible/`

### `star-control-validation`

- validation requirement 실행
- Star Sentinel tool invocation
- approval gate 반영
- validation contract constants and public error boundary modules
- validation public DTO module
- validation engine facade and schema-root helper in `src/engine.rs`
- provider response check, Star Sentinel gate evaluation, outcome writer, and approval response gate modules in `src/engine/`
- Star Sentinel gate approval parsing/inconsistency helper and outcome builder modules in `src/engine/gate/`
- validation outcome writer artifact/event/run-state helper modules in `src/engine/writer/`
- validation decision, approval request, review-pack handoff, validation-run builder module
- schema/file helper and run-state helper modules
- validation/validation-decision.json 생성
- validation_runs.json 관리
- validation test module root in `src/tests.rs`
- approval response, Star Sentinel gate/outcome writer, and provider response scenario tests in `src/tests/`
- validation test helper facade, JSON builders, and fixture lifecycle helpers in `src/tests/helpers.rs` and `src/tests/helpers/`

### `star-control-report`

- ReportSpec 생성
- user-facing report 생성
- changed_files, risks, validation, artifacts 정리

### `star-control-cli`

- `run`, `status`, `report`, `approve`, `cancel`, `resume` 명령
- CLI argument root는 `src/args.rs`에 두고 `ParsedArgs` model, option dispatch, parse loop는 `src/args/` module에 둔다
- `run`은 route/workspec/provider execution wrapper root인 `src/run.rs`와 registry/artifact/route/state/execution helper를 담은 `src/run/` module로 유지
- `status`, `report`, `recover --list`는 file-based StateStore read-only command facade인 `src/read_commands.rs`로 유지
- CLI read-only status/report/release-readiness/recover helper는 `src/read_commands/` module에 둔다
- `approve`, `cancel`, `resume`은 file-based StateStore control command facade와 `src/control/approve.rs`, `cancel.rs`, `resume.rs` module로 유지
- control helper facade는 `src/control/helpers.rs`에 둔다
- approval response validation/state transition helper는 `src/control/helpers/approval.rs`에 둔다
- job artifact/schema read helper는 `src/control/helpers/artifacts.rs`에 둔다
- run-state mutation helper는 `src/control/helpers/state.rs`에 둔다
- CLI event writer와 timestamp helper는 `src/control/helpers/events.rs`, `time.rs`에 둔다
- `report --release-readiness` read-only release readiness surface
- `recover --list` inspect-only recovery surface
- `providers` command group은 builtin provider manifest/capability read-only discovery wrapper인 `src/providers.rs`로 유지
- providers command reserved option guard, builtin registry loader, provider summary helper는 `src/providers/` module에 둔다
- `sentinel` command group은 dispatch root인 `src/sentinel.rs`, subcommand re-export root인 `src/sentinel/commands.rs`, check/gate/review-pack/selfcheck command modules, evaluation, option validation, path/status helper를 담은 `src/sentinel/` module로 유지
- unit test fixture/helper facade는 `src/test_support.rs`에 둔다
- control command scenario tests and command assertion helpers are split under `src/tests/control.rs` and `src/tests/control/`
- CLI temp project/repo-root helper는 `src/test_support/project.rs`에 둔다
- local-process provider instance fixture는 `src/test_support/local_process.rs`에 둔다
- Star Sentinel input fixture는 `src/test_support/sentinel.rs`에 둔다
- approval/release/recovery command fixtures는 `src/test_support/approval.rs`, `release.rs`, `recovery.rs`에 둔다
- unit test module root is kept in `src/tests.rs`
- command-focused unit tests are kept in `src/tests/run.rs`, `providers.rs`, `sentinel.rs`, `release.rs`, `recover.rs`, and `control.rs`
- Sentinel command scenario success/error bodies are split under `src/tests/sentinel/commands.rs` and `src/tests/sentinel/errors.rs`
- CLI run command test wrappers are kept in `src/tests/run.rs`; fake flow, local-process execution, error scenarios, and shared assertions are split across `src/tests/run/`
- v0 fake integration smoke scenarios are kept in `tests/v0_fake_flow.rs`
- v0 fake integration smoke fixture lifecycle, CLI runner, validation flow root, Sentinel gate/review-pack writer, fixture builders, approval response fixture, and final report helpers are split across `tests/v0_fake_flow/`
- stdout/stderr/exit code 계약
- daemon 없이도 file-based flow 실행 가능해야 함

### `star-control-daemon`

- file-based daemon queue state
- StateStore job queue reference 등록
- terminal/approval/duplicate queue guard
- daemon contract constants, public config/error boundary, atomic state IO helper, queue facade, and unit test root are split into `src/constants.rs`, `src/config.rs`, `src/error.rs`, `src/io.rs`, `src/queue.rs`, `src/tests.rs`
- DaemonError Display and source mapping are split into `src/error/`
- daemon approval guard, enqueue flow, field helper, schema validation, and state IO helpers in `src/queue/`
- daemon default-state, enqueue, approval, and shared test helpers in `src/tests/`
- RESERVED: background runner, socket, API server, provider session scheduling

### `star-control-api`

- UI와 외부 도구가 사용하는 API
- API contract constants, request types, public error boundary, path parser/project id validator modules
- read-only request/router facade in `src/read_only.rs`
- read-only daemon/project/job/event/report/release-readiness/envelope helpers in `src/read_only/`
- API control service facade and GET/POST routing in `src/control.rs`
- control mutation module root and shared invalid-request helper in `src/control/mutations.rs`
- approve/cancel/resume in-process control mutation flows in `src/control/mutations/`
- approve request body, approval response build/validation/write, and approval event/success payload helpers in `src/control/mutations/approve/`
- resume approval artifact load, next-action, event, and success payload helpers in `src/control/mutations/resume/`
- control helper re-export root in `src/control/helpers.rs`
- control request body, API event, run-state/approval matching, and timestamp helpers in `src/control/helpers/`
- schema/control artifact helper in `src/artifacts.rs`
- api-response envelope validation
- unit tests in `src/tests.rs`
- read-only endpoint test root and control mutation scenario tests in `src/tests/`
- read-only daemon/errors/projects/release/report scenario tests plus assertion and state snapshot helpers in `src/tests/read_only/`
- RESERVED: HTTP server, remote exposure, auth/session, provider scheduling

### `star-control-ui`

- API read-only service를 소비하는 UI read-only view model
- API control service를 소비하는 browser-oriented control shell model
- job list, job detail, timeline, provider output, validation, approval, review pack viewer data
- release readiness viewer data
- approve/cancel/resume action panel과 mutation result view
- public re-export and module declarations in `src/lib.rs`
- UI contract constants in `src/constants.rs`
- public error boundary in `src/error.rs`
- schema/API envelope helper in `src/helpers.rs`
- read-only shell orchestration in `src/read_only.rs`
- read-only API artifact, report, and release-readiness helpers in `src/read_only/`
- browser control shell orchestration in `src/browser.rs`
- view model root in `src/view.rs`; state, artifact, and approval projection helpers in `src/view/`
- browser control action list helper in `src/control_actions.rs`
- shared UI test root in `src/tests.rs`
- UI test helper facade in `src/tests/helpers.rs`
- UI project/store, shell, and job/report/approval/release fixture helpers in `src/tests/helpers/`
- read-only/browser scenario test roots in `src/tests/read_only.rs` and `src/tests/browser.rs`
- UI read-only list/detail/release/approval/redaction/error scenario bodies in `src/tests/read_only/`
- RESERVED: browser UI app, TypeScript/Node package manager, HTTP server, remote UI runtime

### `star-control-security`

- shared redaction utility
- RedactionReport builder
- public re-export/module declaration root in `src/lib.rs`
- redaction contract constants in `src/constants.rs`
- RedactionFinding and RedactionOutcome model in `src/model.rs`
- recursive JSON redaction traversal and key/string detection in `src/redact.rs`
- RedactionReport builder in `src/report.rs`
- schema-valid redaction scenario tests in `src/tests.rs`
- secret-like key/string detection without storing raw values
- RESERVED: RedactionReport artifact storage, retention/recovery command, release readiness automation

### `star-control-observability`

- public re-export and module declarations in `src/lib.rs`
- contract constants and public error boundary modules
- AuditEventWriter
- audit event builder/root in `src/audit.rs`
- audit JSONL append/readback helper in `src/audit/io.rs`
- audit schema validation helper in `src/audit/validation.rs`
- audit timestamp helper in `src/audit/time.rs`
- schema-valid `audit/audit-events.jsonl` append/readback helper
- StateStore job directory containment for audit log paths
- shared redaction utility application before audit persistence
- CostMetricWriter
- cost metric writer root and metric builder in `src/cost.rs`
- cost budget thresholds and warning-only budget evaluation in `src/cost/budget.rs`
- cost metric write/readback helper in `src/cost/io.rs`
- cost provider path guard in `src/cost/paths.rs`
- cost metric schema and semantic validation in `src/cost/validation.rs`
- schema-valid provider output `cost-metric.json` write/readback helper
- warning-only CostBudgetThresholds evaluation
- test module root in `src/tests.rs`
- audit and cost scenario tests in `src/tests/audit.rs` and `src/tests/cost.rs`
- observability test schema/temp project/StateStore helpers in `src/tests/`
- RESERVED: API/CLI/daemon/provider automatic audit/cost integration, hard budget enforcement, retention/recovery command, release readiness automation

### `star-control-release`

- ReleaseReadinessWriter
- schema-valid `release/release-readiness.json` write/readback helper
- ReleaseConsistencyChecker for version/changelog checks
- ReleaseEvidenceFileChecker for read-only version/changelog evidence files
- ReleaseProfileReadinessBuilder for profile/version/changelog readiness assembly
- ReleaseReviewPackWriter for `review-packs/release-review-pack.md`
- release review-pack Markdown rendering and no-overwrite text writer helpers in `src/review_pack/`
- M9ReadinessAuditBuilder for final M9 hardening/recovery/release readiness audit assembly in `src/audits/m9.rs`
- CompleteImplementationAuditBuilder for final M0~M9 completion audit assembly in `src/audits/complete.rs`
- release readiness unit test fixture helpers in `src/test_support.rs`
- release readiness unit test module root in `src/tests.rs`
- release readiness tests in `src/tests/readiness.rs`
- release review-pack tests in `src/tests/review_pack.rs`
- version/changelog consistency and evidence file tests in `src/tests/consistency.rs`
- profile readiness tests in `src/tests/profile.rs`
- release audit test root in `src/tests/audits.rs`
- M9/complete implementation audit scenario tests and readiness assertion helpers in `src/tests/audits/`
- release contract constants and public error boundary modules
- release consistency/evidence checker module
- release profile readiness builder module
- final readiness audit builder root and M9/complete audit builder modules
- release readiness writer facade in `src/writer.rs`
- release readiness file IO, JSON builder, and schema/status validation helpers in `src/writer/`
- release review-pack writer module
- shared release normalization/support facade in `src/support.rs`
- release check/status, evidence path guard, name/blocker normalization, release text/version parsing, and timestamp helpers in `src/support/`
- unit tests in `src/tests.rs`
- reserved/not_ready readiness artifact generation
- RESERVED: signing, publish, deploy automation, repository/package registry settings changes

### `star-sentinel`

- Star Sentinel builtin tool 구현
- policy, diagnostics, approval gate, review pack, ledger, schema IO, readers, selfcheck
- crate root는 public API re-export와 thin orchestration만 소유한다
- public re-export and module declarations in `src/lib.rs`
- contract constants in `src/constants.rs`
- public error boundary in `src/error.rs`
- task/changed-lines parsing in `src/task.rs` and `src/changed_lines.rs`
- public model re-export in `src/model.rs`, with decision/severity, P0 registry, diagnostic/evaluation DTO, artifact refs, and ledger event model split under `src/model/`
- P0 evaluator orchestration in `src/evaluator.rs`
- fixture outcome, matcher facade, path matcher, secret matcher, and rule evaluation root modules in `src/evaluator/`
- P0 allowed-paths, dependency, secret, test-deletion, validator, diagnostic construction, and changed-line iterator helpers in `src/evaluator/rules/`
- gate and ledger artifact builders in `src/gate.rs` and `src/ledger.rs`
- review-pack builder root in `src/review_pack.rs`, with artifact assembly, signal derivation, Markdown rendering, and StateStore writer helpers in `src/review_pack/`
- schema IO/readers/selfcheck root in `src/schema_io.rs`, `src/readers.rs`, `src/selfcheck.rs`
- selfcheck legacy alias, manifest output, registry/schema/fixture contract, and YAML-list helper modules in `src/selfcheck/`
- unit tests in `src/tests.rs`
- core와 직접 결합 금지

## apps 경계

```text
apps/
  starctl/
  star-daemon/
  star-control-ui/
```

- `apps/starctl/`은 CLI entrypoint 후보다.
- `apps/star-daemon/`은 daemon entrypoint 후보이며 초기 구현 대상이 아니다.
- `apps/star-control-ui/`는 browser UI app 후보이며 초기 구현 대상이 아니다. M8a/M8b의 library-level view/control shell model은 `packages/star-control-ui/`에 둔다.
- app layer는 core logic을 직접 소유하지 않고, 안정화된 package API를 호출하는 얇은 표면이어야 한다.

## builtin 경계

```text
builtin-providers/
  test/
  local-process/
  local-server/
  cloud-cli/
  cloud-api/

builtin-tools/
  star-sentinel/
```

- `builtin-providers/`는 provider manifest와 capability profile을 둔다.
- `builtin-tools/star-sentinel/`은 tool manifest, policy, schema, fixture, example을 둔다.
- 구현 코드는 `packages/` 아래에 둔다.

## docs 경계

```text
docs/
  implementation/
  operations/
  providers/
  tools/
  decisions/
```

- `docs/implementation/`: Codex와 구현자가 따를 전체 구현 계약.
- `docs/operations/`: ChatGPT, GitHub, CI, Codex 운영 기준.
- `docs/providers/`: provider 개념, registry, capability 문서.
- `docs/tools/`: builtin tool 개요.
- `docs/decisions/`: 장기 결정 기록.

## specs 경계

```text
specs/
  schemas/
```

`specs/schemas/`는 Star-Control core-level schema를 둔다.

예시:

- `job.schema.json`
- `run-state.schema.json`
- `route.schema.json`
- `workspec.schema.json`
- `report.schema.json`

Star Sentinel 전용 schema는 `builtin-tools/star-sentinel/schemas/`에 둔다.

## configs 경계

`configs/`는 runtime default, template, role, policy, registry 후보를 둔다. implementation package가 생기기 전까지는 정본 설정과 template 중심으로 유지한다.

## examples 경계

`examples/`는 Star-Control core-level sample artifact를 둔다. Star Sentinel 전용 example은 `builtin-tools/star-sentinel/examples/`에 둔다.

`examples/runs/`는 실제 실행 결과 저장 위치가 아니라 schema와 문서 검증을 위한 sample이다.

## scripts 경계

`scripts/ci/`는 현재 repository의 계약 검증 스크립트를 둔다.

현재 검사 후보:

- repository policy
- data format
- manifest contract
- Star Sentinel naming policy
- schema example
- implementation documentation

후속 PR에서 provider contract, config contract, policy fixture, work queue consistency 검사를 추가할 수 있다.

## 금지되는 구조

```text
packages/codex-provider-core/       # 특정 제품명이 core package에 들어감
packages/star-control-star-sentinel # core와 tool 경계 혼동
.ai-runs/                           # Star-Control repo 내부 실행 산출물
```

## 현재 구현 순서 기준

실제 착수 순서는 `docs/implementation/codex-work-queue-current.md`를 우선한다. 현재 v0 구현 순서는 다음과 같다.

```text
E01 Schema / Runtime Validator
E02 File-based StateStore
E03 Artifact Layout Writer
E04 Provider Registry
E05 FakeProviderAdapter
E06 RouterEngine
E07 ExecutionEngine
E08 CLI read-only + fake run
E09 Star Sentinel P0
E10 ValidationEngine
E11 Integration Smoke
```

장기 구현 흐름은 아래 원칙을 따른다. 전체 milestone은 `complete-implementation-roadmap.md`를 기준으로 한다.

1. schema/runtime validator를 먼저 안정화한다.
2. file-based StateStore와 artifact layout을 안정화한다.
3. provider registry와 fake provider를 붙인다.
4. router와 execution engine을 fake flow 기준으로 연결한다.
5. CLI read-only와 fake run을 안정화한다.
6. Star Sentinel P0와 ValidationEngine을 연결한다.
7. fake provider 기반 integration smoke를 만든다.
8. local process provider를 먼저 붙인다.
9. local model/server provider와 cloud CLI/API provider를 순차 확장한다.
10. daemon, API, UI는 CLI file-based flow와 approval flow가 안정화된 뒤 확장한다.
11. release automation은 release readiness와 approval flow가 안정화된 뒤 별도 승인으로만 구현한다.

## PR 경계 원칙

- 한 PR은 하나의 계약 또는 하나의 package 목적만 다룬다.
- 문서, schema, example, CI 검증이 함께 움직여야 하는 계약은 같은 PR에 둔다.
- StateStore, Router, ProviderAdapter, ExecutionEngine, ValidationEngine, CLI 구현을 한 PR에 섞지 않는다.
- schema 변경 PR은 example과 schema-example-check 영향을 함께 검토한다.
