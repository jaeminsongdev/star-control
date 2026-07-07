# Current Repository Map

## 목적

이 문서는 현재 Star-Control repository에 존재하는 경로의 의미를 고정한다. `repository-layout.md`가 목표 package 경계를 설명한다면, 이 문서는 구현자가 실제 파일을 볼 때 어떤 경로가 정본이고 어떤 경로가 예약 영역인지 판단하게 해 주는 기준표다.

실제 구현 착수 순서는 `codex-work-queue-current.md`를 우선한다. 이 문서는 repository 경로 상태를 설명하고, 현재 EPIC/TASK의 세부 순서는 `codex-work-queue-current.md`가 결정한다.

## 상태 표기

| 상태 | 의미 |
|---|---|
| `CANONICAL` | 현재 설계와 구현 계약의 정본 경로다. |
| `SCAFFOLD` | 목표 구조를 표시하기 위한 골격이다. |
| `RESERVED` | 장기 목표로 예약했지만 초기 구현 대상은 아니다. |
| `EXAMPLE` | schema, 문서, smoke 검증을 위한 예시다. |
| `BACKLOG` | 장기 구현 후보이며 현재 착수 큐보다 우선하지 않는다. |

## 현재 정본 경로

| 경로 | 상태 | 책임 |
|---|---|---|
| `README.md` | `CANONICAL` | repository 목적, 현재 상태, 첫 읽기 경로를 설명한다. |
| `AGENTS.md` | `CANONICAL` | 이 repository에서 작업하는 AI와 구현자가 지킬 작업 경계와 검증 기준이다. |
| `.github/workflows/` | `CANONICAL` | 현재 repository의 최소 CI 검증선을 둔다. |
| `docs/` | `CANONICAL` | 설계, 구현 계약, 운영 문서, 결정 기록을 둔다. |
| `docs/implementation/` | `CANONICAL` | 구현자가 따라야 하는 책임 경계, 데이터 계약, 실행 흐름, 검증 기준을 둔다. |
| `docs/operations/` | `CANONICAL` | ChatGPT, GitHub, CI, Codex 운영 기준을 둔다. |
| `docs/providers/` | `CANONICAL` | provider 개념, registry, capability 관련 문서를 둔다. |
| `docs/tools/` | `CANONICAL` | builtin tool 개요 문서를 둔다. |
| `docs/decisions/` | `CANONICAL` | 장기 결정 기록을 둔다. |
| `specs/schemas/` | `CANONICAL` | machine-readable JSON schema를 둔다. |
| `configs/` | `CANONICAL` | default config, policy, role, skill, hook, template, registry 후보를 둔다. |
| `builtin-providers/` | `CANONICAL` | builtin provider manifest와 capability profile을 둔다. provider 구현 코드는 두지 않는다. |
| `builtin-tools/star-sentinel/` | `CANONICAL` | Star Sentinel manifest, policy, schema, fixture, example, corpus를 둔다. |
| `examples/` | `EXAMPLE` | provider instance와 sample run artifact를 둔다. 실제 run output 위치가 아니다. |
| `scripts/ci/` | `CANONICAL` | repository policy, data format, manifest, naming, schema example, implementation docs 검증 스크립트와 productization E2E smoke를 둔다. |

## scaffold / reserved 경로

| 경로 | 상태 | 책임 |
|---|---|---|
| `apps/starctl/` | `SCAFFOLD` | 최종 CLI entrypoint 후보. 초기 구현 전에는 문서 골격만 둔다. |
| `apps/star-daemon/` | `CANONICAL` | local daemon app entrypoint. `status`, 테스트 가능한 `serve --max-ticks`, loopback-only `api` HTTP server가 queue/API state를 실제 process surface에서 연다. `serve --max-ticks`는 queued `fake-default` job과 `provider_instance_paths`가 보존된 allowlisted local-process job을 실행하고, 그 외 non-fake/cloud/live provider는 disabled scheduler result로 남긴다. remote exposure/cloud-live scheduler executor/Local·Cloud AI live connector는 아직 disabled다. |
| `apps/star-control-ui/` | `CANONICAL` | 정적 browser UI app. `star-daemon api`를 소비해 daemon state, jobs, job detail, timeline, approval action, release readiness를 표시한다. provider process/Star Sentinel rule/StateStore 직접 mutation/Local·Cloud AI live connector는 구현하지 않는다. |
| `packages/` | `CANONICAL` / `SCAFFOLD` | `star-control-*` Cargo workspace crate와 `star-sentinel` 구현 코드를 둔다. `star-control-api`는 read-only service와 in-process control mutation service까지 구현하며 HTTP server/auth/remote exposure는 아직 reserved다. `star-control-ui`는 read-only view model과 browser-oriented control shell model까지 구현한다. `star-control-security`는 shared redaction utility와 RedactionReport builder를 둔다. 기존 provider/transport/adapter scaffold는 post-core 확장 후보로 남긴다. |
| `integrations/` | `RESERVED` | GitHub ruleset, workflow, 외부 연동 산출물 후보. 실제 연동 작업은 별도 승인 후 처리한다. |

## apps와 packages의 관계

`apps/`는 사람이 실행하는 표면을 나타내는 entrypoint scaffold다. `packages/`는 재사용 가능한 구현 module 경계다.

초기 구현 원칙:

1. 구현 코어는 목표상 `packages/` 아래 package 경계로 나눈다.
2. `apps/starctl`은 CLI entrypoint 후보이며 core logic을 직접 소유하지 않는다.
3. `packages/star-control-daemon`은 M7b에서 file-based queue skeleton만 구현한다.
4. `apps/star-daemon`은 process/API surface를 제공한다. `apps/star-control-ui`는 정적 browser app을 제공하고, shared UI read-only/control model은 package layer에 둔다.
5. 새 runtime dependency와 Cargo 외 package manager는 별도 승인 전까지 추가하지 않는다.

## builtin 경계

`builtin-providers/`와 `builtin-tools/`는 구현 코드 위치가 아니다.

```text
builtin-providers/             # provider manifest, capability profile
builtin-tools/star-sentinel/    # tool manifest, policy, schema, fixture, example
packages/star-sentinel/         # Star Sentinel 구현 코드 후보
```

Core package는 provider 제품명을 직접 포함하지 않는다. 새 provider는 manifest, capability profile, adapter 경계로 추가한다.

## 실행 산출물 위치

Star-Control repository 내부에는 실제 실행 산출물을 저장하지 않는다. 실제 run artifact는 대상 프로젝트 아래에 생성한다.

```text
{target-project}/.ai-runs/J-0001/
```

`examples/runs/`는 schema와 문서 검증을 위한 예시일 뿐 실제 실행 산출물이 아니다.

## naming 기준

Star Sentinel 공식 표기는 다음만 사용한다.

```text
Star Sentinel
star-sentinel
star_sentinel
star.sentinel
```

호환 alias는 `builtin-tools/star-sentinel/tool.yaml`의 `legacy_aliases`에만 둔다.

| 목적 | 표기 |
|---|---|
| CLI command | `review-pack` |
| JSON/Markdown artifact | `review_pack.json`, `review_pack.md` |
| package 후보 | `star-sentinel` |
| python entrypoint 후보 | `star_sentinel.main` |
| tool id | `star.sentinel` |

## 현재 계약 상태

현재 repository에는 v0 구현 착수를 위한 주요 계약 문서, schema, canonical example, 최소 CI 검증선이 들어 있다.

| 계약 묶음 | 현재 위치 | 상태 |
|---|---|---|
| core artifact 계약 | `specs/schemas/`, `examples/runs/`, `docs/implementation/data-contracts.md` | `CANONICAL` |
| StateStore / artifact layout | `state-store.md`, `state-store-recovery.md`, `artifact-layout.md`, `artifact-naming.md` | `CANONICAL` |
| provider 계약 | `provider-system.md`, `docs/providers/`, `examples/provider-contracts/` | `CANONICAL` |
| config / policy / role / hook 계약 | `config-system.md`, `examples/config-contracts/` | `CANONICAL` |
| router decision 계약 | `router-decision-matrix.md`, `router-engine.md`, `examples/router-contracts/` | `CANONICAL` |
| execution 계약 | `execution-engine.md`, `examples/execution-contracts/` | `CANONICAL` |
| Star Sentinel P0 계약 | `star-sentinel-p0-contracts.md`, `builtin-tools/star-sentinel/` | `CANONICAL` |
| validation handoff 계약 | `validation-engine.md`, `validation-handoff.md`, `examples/validation-contracts/` | `CANONICAL` |
| CLI / daemon queue / API read-only / UI read-only / reserved surfaces | `cli-command-reference.md`, `daemon-contract.md`, `api-contract.md`, `ui-shell-contract.md` | `CANONICAL` / `RESERVED` |
| security / privacy / observability contracts | `security-privacy-observability-contracts.md`, `security-cost-observability.md`, `packages/star-control-security/`, `packages/star-control-observability/` | `CANONICAL` |
| CI 계약 검증 | `scripts/ci/`, `.github/workflows/ci.yml`, `ci-contract-validation.md` | `CANONICAL` |
| 리팩토링 / 하드코딩 기준 | `refactoring-hardcoding-guidelines.md` | `CANONICAL` |
| 현재 구현 큐 | `codex-work-queue-current.md` | `CANONICAL` |
| 장기 backlog | `codex-work-queue.md` | `BACKLOG` |

## 남은 정리 대상

아래 항목은 현재 구현 착수 전후로 보강할 수 있지만, `codex-work-queue-current.md`의 순서를 앞지르지 않는다.

| 대상 | 처리 기준 |
|---|---|
| handoff schema required field 강화 | 별도 schema/example PR에서 처리한다. |
| forbidden action vocabulary 고정 | schema/example/docs를 함께 수정한다. |
| work queue consistency CI | 별도 CI PR에서 추가한다. |
| Schema validator 세부 분할 | `packages/star-control-schema/src/error.rs`, `types.rs`, `loader.rs`, `validator.rs`, `validator/compound.rs`, `validator/path.rs`, `validator/pattern.rs`, `validator/scalar.rs`, `tests.rs`, `tests/helpers.rs`, `tests/keywords.rs`, `tests/structures.rs`, `tests/loading.rs`로 public error boundary, Schema/Validation DTO, schema/document loader와 file validation helper, subset validator root, object/array traversal, path/schema-path helper, known-pattern matcher, scalar type/string validation, validator unit test root, schema/temp file fixture helper, keyword/structure/loading scenario tests는 분리됨. `src/lib.rs`는 public re-export 중심이다. |
| Schema canonical example test 세부 분할 | `packages/star-control-schema/tests/canonical_examples.rs`는 integration test runner root이며 `tests/canonical_examples/cases.rs`, `cases/core.rs`, `cases/provider_execution.rs`, `cases/surface.rs`, `cases/config.rs`, `cases/sentinel.rs`로 canonical schema/example validation cases는 domain별 분리됨. |
| E08 CLI 세부 분할 | constants/config/error/args/output/run/providers/read-only/sentinel/control/test_support/tests module은 분리됨. `src/lib.rs`는 public entrypoint와 shared CLI input helper 중심이고, `src/args.rs`는 CLI argument root이며 `src/args/model.rs`, `options.rs`, `parser.rs`로 ParsedArgs model, option dispatch, parse loop가 분리됨. `src/run.rs`는 run command orchestration root이며 registry loading, artifact list, provider override route/workspec rewrite, routed-state/report helper, provider execution/report branch, run-specific constants는 `src/run/` 하위 module로 분리됨. `src/providers.rs`는 providers list/show command orchestration root이며 reserved option guard, builtin registry loader, provider summary helper는 `src/providers/options.rs`, `registry.rs`, `summary.rs`로 분리됨. `src/read_commands.rs`는 status/report/recover command facade이며 status command, report command, release-readiness report branch, recover inspect-only branch는 `src/read_commands/status.rs`, `report.rs`, `release.rs`, `recover.rs`로 분리됨. `src/sentinel.rs`는 dispatch root이며 `src/sentinel/commands.rs`, `src/sentinel/commands/check.rs`, `gate.rs`, `review_pack.rs`, `selfcheck.rs`, `evaluation.rs`, `options.rs`, `paths.rs`, `status.rs`로 Star Sentinel command flow와 helper가 분리됨. `src/control.rs`는 approve/cancel/resume command facade이며 `src/control/approve.rs`, `cancel.rs`, `resume.rs`, `helpers.rs`, `helpers/approval.rs`, `helpers/artifacts.rs`, `helpers/state.rs`, `helpers/events.rs`, `helpers/time.rs`로 command flow와 shared approval/artifact/state/event/time helper가 분리됨. `src/tests.rs`는 test module root이며 providers/sentinel/release/recover/control command tests와 control command assertion/helper는 `src/tests/` 하위 module로 분리됨. `src/tests/sentinel.rs`는 Sentinel command scenario test wrapper이며 success/error scenario bodies는 `src/tests/sentinel/commands.rs`, `errors.rs`로 분리됨. `src/tests/run.rs`는 run command test wrapper이며 fake flow/local-process/error scenario와 shared helper는 `src/tests/run/` 하위 module로 분리됨. `tests/v0_fake_flow.rs`는 v0 smoke scenario root이고 fixture lifecycle/CLI runner/validation flow root/Sentinel gate-review-pack writer/fixture builders/approval response fixture/report writer helper는 `tests/v0_fake_flow/` 하위 module로 분리됨. 남은 CLI 정리는 new command branch가 추가될 때 별도 submodule slice로 다룬다. |
| CLI test fixture 세부 분할 | `packages/star-control-cli/src/test_support.rs`는 fixture re-export root이며 `src/test_support/project.rs`, `local_process.rs`, `sentinel.rs`, `approval.rs`, `release.rs`, `recovery.rs`로 temp project/repo-root helper, local-process provider instance fixture, Star Sentinel input fixture, approval/release/recovery command fixture는 분리됨. |
| E09 Star Sentinel P0 세부 분할 | `packages/star-sentinel/src/`의 constants/error/task/changed-lines/json-field/model/evaluator/gate/review-pack/ledger/readers/schema IO/selfcheck/tests 단위는 분리됨. `src/lib.rs`는 public re-export와 module declaration 중심이고 `src/model.rs`는 public model re-export root이며 decision/severity, P0 registry, diagnostic/evaluation DTO, artifact refs, ledger event model은 `src/model/` 하위 module로 분리됨. `src/evaluator.rs`는 public evaluator orchestration facade이며 fixture outcome, matcher facade, path matcher, secret matcher, rule evaluation root, allowed-paths/dependency/secret/test-deletion/validator rule family evaluators, diagnostic construction helper, changed-line iterator helper는 `src/evaluator/`, `src/evaluator/matchers/`, `src/evaluator/rules/` 하위 module로 분리됨. `src/selfcheck.rs`는 public selfcheck orchestration root이며 legacy alias scan, manifest output check, P0 registry/schema/fixture contract checks, YAML-list helper는 `src/selfcheck/` 하위 module로 분리됨. `src/tests.rs`는 shared fixture helper와 test module root이며 evaluator/gate/review-pack/ledger/selfcheck scenario tests는 `src/tests/` 하위 module로 분리됨. |
| StateStore 세부 분할 | `packages/star-control-state/src/constants.rs`, `error.rs`, `error/display.rs`, `error/source.rs`, `types.rs`, `paths.rs`, `store.rs`, `store/lifecycle.rs`, `store/jobs.rs`, `store/core_artifacts.rs`, `store/paths.rs`, `events.rs`, `outputs.rs`, `outputs/refs.rs`, `outputs/provider.rs`, `outputs/tool.rs`, `outputs/approval.rs`, `outputs/review.rs`, `outputs/validation.rs`, `outputs/tmp.rs`, `artifacts.rs`, `artifacts/atomic.rs`, `artifacts/json.rs`, `artifacts/replace.rs`, `artifacts/schema.rs`, `artifacts/time.rs`, `recovery.rs`, `recovery/action.rs`, `recovery/inspection.rs`, `recovery/issue.rs`, `recovery/summary.rs`, `recovery/tmp.rs`, `tests.rs`, `tests/store.rs`, `tests/store/jobs.rs`, `tests/store/events.rs`, `tests/store/paths.rs`, `tests/recovery.rs`, `tests/artifacts.rs`, `tests/artifacts/dirs.rs`, `tests/artifacts/refs.rs`, `tests/artifacts/safety.rs`, `tests/artifacts/writers.rs`로 contract constants, public error enum/display/source boundary, public type boundary, path validation, store module root, lifecycle/open/getter helper, job allocation/create/list/resume flow, core artifact save/load helper, job-relative path resolution, event append/read, output helper root, artifact ref/register helper, provider/tool/approval/review-pack/validation/tmp writer, artifact helper root, atomic write, JSON/text artifact IO, platform replace, schema validation, timestamp helper, inspect-only recovery report, recovery action dry-run plan, approval-gated recovery executor, artifact replacement source selection executor, recovery issue model/error mapping, job summary, tmp artifact warning collection, test module root/shared fixture, store job/event/path scenario tests, recovery test, artifact output-dir/ref/safety/writer scenario tests는 분리됨. |
| release readiness 세부 분할 | `packages/star-control-release/src/constants.rs`, `error.rs`, `consistency.rs`, `profile.rs`, `automation.rs`, `audits.rs`, `audits/m9.rs`, `audits/complete.rs`, `writer.rs`, `writer/io.rs`, `writer/model.rs`, `writer/validation.rs`, `review_pack.rs`, `review_pack/markdown.rs`, `review_pack/storage.rs`, `support.rs`, `support/checks.rs`, `support/evidence.rs`, `support/names.rs`, `support/text.rs`, `support/time.rs`, `tests.rs`, `test_support.rs`, `tests/readiness.rs`, `tests/review_pack.rs`, `tests/consistency.rs`, `tests/profile.rs`, `tests/audits.rs`, `tests/audits/m9.rs`, `tests/audits/complete.rs`, `tests/audits/helpers.rs`로 release contract constants, public error boundary, version/changelog checker, evidence file checker, profile readiness builder, release automation dry-run planner/local result executor, audit builder re-export root, final M9 readiness audit builder, complete implementation audit builder, readiness writer facade, readiness file IO, readiness/check JSON builder, schema/status validation, review-pack writer facade, release review-pack Markdown rendering/no-overwrite text writer helper, support facade, release check/status helper, evidence path guard, name/blocker normalization, text/version parsing, timestamp helper, unit test root, fixture helper, responsibility-focused test modules, audit test root, M9/complete audit scenario tests, audit readiness assertion helper는 분리됨. `src/lib.rs`는 public re-export 중심이다. |
| API control-plane 세부 분할 | `packages/star-control-api/src/constants.rs`, `request.rs`, `error.rs`, `paths.rs`, `read_only.rs`, `read_only/daemon.rs`, `read_only/envelope.rs`, `read_only/jobs.rs`, `read_only/projects.rs`, `read_only/release.rs`, `read_only/reports.rs`, `control.rs`, `control/helpers.rs`, `control/helpers/body.rs`, `control/helpers/events.rs`, `control/helpers/state.rs`, `control/helpers/time.rs`, `control/mutations.rs`, `control/mutations/approve.rs`, `control/mutations/approve/request.rs`, `control/mutations/approve/response.rs`, `control/mutations/approve/event.rs`, `control/mutations/cancel.rs`, `control/mutations/resume.rs`, `control/mutations/resume/artifacts.rs`, `control/mutations/resume/event.rs`, `artifacts.rs`, `tests.rs`, `tests/read_only.rs`, `tests/read_only/daemon.rs`, `tests/read_only/errors.rs`, `tests/read_only/helpers.rs`, `tests/read_only/projects.rs`, `tests/read_only/release.rs`, `tests/read_only/reports.rs`, `tests/control.rs`로 contract constants, request types, public error boundary, path parsing/project id validation, read-only service routing root, daemon/projects/jobs/events/report/release-readiness/envelope helper, control service facade/routing, control helper re-export root, request body helper, API event writer, run-state transition/approval matching helper, timestamp helper, approve/cancel/resume mutation flow, approve request body/approval response/event payload helper, resume approval artifact load/next-action/event payload helper, schema/control artifact helper, shared API test fixture/helper, read-only endpoint test root, read-only daemon/errors/projects/release/report scenario tests, read-only assertion/state snapshot helper, control mutation tests는 분리됨. HTTP server/auth/remote exposure는 계속 `RESERVED`다. |
| RouterEngine 세부 분할 | `packages/star-control-router/src/lib.rs`, `constants.rs`, `contract.rs`, `engine.rs`, `error.rs`, `types.rs`, `tests.rs`, `tests/helpers.rs`, `tests/scenarios.rs`, `analysis.rs`, `analysis/types.rs`, `analysis/types/change.rs`, `analysis/types/decision.rs`, `analysis/types/request.rs`, `analysis/types/scale.rs`, `analysis/types/stages.rs`, `analysis/classification.rs`, `analysis/classification/haystack.rs`, `analysis/classification/rules.rs`, `analysis/classification/rules/catalog.rs`, `analysis/classification/rules/catalog/routine.rs`, `analysis/classification/rules/catalog/safety.rs`, `analysis/policy.rs`, `analysis/policy/mapping.rs`, `workspec.rs`, `workspec/stage.rs`, `workspec/route.rs`, `workspec/role.rs`, `workspec/path.rs`로 public re-export root, router contract constants, schema validation/field extraction helper, RouterEngine orchestration, public error boundary, public job/route/workspec/output types, unit test module root, RouterEngine scenario tests, router shared test helper, request analysis module root, analysis type re-export root, change type enum, policy/route decision enum, request analysis DTO, size/risk scale enum, stage selection helper, keyword classification orchestration, normalized haystack helper, keyword matching helper, keyword rule catalog root, routine/safety rule family, size/risk/profile/approval policy orchestration, change-type별 policy mapping, workspec facade, stage WorkSpec JSON builder, route assignment/path map builder, stage role helper, workspec artifact path helper는 분리됨. 남은 정리는 provider assignment 지능화가 필요할 때 별도 slice로 다룬다. live provider 확장은 계속 `RESERVED`다. |
| ExecutionEngine 세부 분할 | `packages/star-control-execution/src/constants.rs`, `error.rs`, `error/display.rs`, `error/source.rs`, `types.rs`, `contract.rs`, `state.rs`, `engine.rs`, `engine/provider.rs`, `engine/request.rs`, `engine/state.rs`, `tests.rs`, `tests/fake.rs`, `tests/local_process.rs`, `tests/local_process/execution.rs`, `tests/local_process/timeout.rs`, `tests/local_process/cancellation.rs`, `tests/local_process/forbidden_action.rs`, `tests/local_process/conformance.rs`, `tests/cloud.rs`, `tests/cloud/cli.rs`, `tests/cloud/api_offline.rs`, `tests/cloud/api_live.rs`, `test_support.rs`, `test_support/helpers.rs`, `test_support/fixture.rs`, `test_support/fixture/cloud.rs`, `test_support/fixture/local_process.rs`, `test_support/fixture/workspec.rs`, `test_support/local_process.rs`, `test_support/local_process/assertions.rs`로 contract constants, public error enum root, public error Display/source helpers, public outcome/assignment model, contract/schema helper, run-state/status helper, ExecutionEngine orchestration root, provider dispatch, execution request/stage guard, state/event write helper, test module root/child-process helper, fake provider tests, local-process execution/timeout/cancellation/forbidden-action/conformance scenario tests, cloud CLI/offline API/live approval-required scenario tests, temp/env/path helper, fixture lifecycle/route setup, cloud fixture registry setup, local-process fixture registry setup, fixture workspec assignment, local-process conformance runner, output contract assertion helper는 분리됨. provider live call 확장은 계속 `RESERVED`다. |
| UI shell 세부 분할 | `packages/star-control-ui/src/lib.rs`는 public re-export/module declaration 중심이며 `constants.rs`, `error.rs`, `helpers.rs`, `read_only.rs`, `read_only/api.rs`, `browser.rs`, `view.rs`, `view/state.rs`, `view/artifacts.rs`, `view/approval.rs`, `control_actions.rs`, `tests.rs`, `tests/read_only.rs`, `tests/read_only/list.rs`, `tests/read_only/detail.rs`, `tests/read_only/release.rs`, `tests/read_only/approval.rs`, `tests/read_only/redaction.rs`, `tests/read_only/errors.rs`, `tests/browser.rs`, `tests/helpers.rs`, `tests/helpers/project.rs`, `tests/helpers/shell.rs`, `tests/helpers/job.rs`로 UI contract constants, public error boundary, schema/API helper, read-only shell orchestration, read-only API artifact/report/release helper, browser control shell orchestration, view model builder root, view state/artifact/approval projection helper, browser control action list helper, test module root, read-only/browser scenario test roots, read-only list/detail/release/approval/redaction/error scenario bodies, project/store helper, shell fixture helper, job/report/approval/release fixture helper는 분리됨. browser UI app/TypeScript package manager/HTTP server/remote UI runtime은 계속 `RESERVED`다. |
| ValidationEngine 세부 분할 | `packages/star-control-validation/src/constants.rs`, `error.rs`, `types.rs`, `engine.rs`, `engine/approval.rs`, `engine/gate.rs`, `engine/gate/approval.rs`, `engine/gate/outcome.rs`, `engine/provider.rs`, `engine/writer.rs`, `engine/writer/artifacts.rs`, `engine/writer/events.rs`, `engine/writer/run_state.rs`, `builders.rs`, `artifacts.rs`, `state.rs`, `tests.rs`, `tests/approval.rs`, `tests/gate.rs`, `tests/provider.rs`, `tests/helpers.rs`, `tests/helpers/builders.rs`, `tests/helpers/fixture.rs`로 contract constants, public error/DTO boundary, ValidationEngine facade/schema helper, approval response gate, Star Sentinel gate evaluation orchestration, gate approval parsing/inconsistency helper, validation outcome builder, provider response check, outcome writer orchestration, validation-run artifact write/reference helper, gate event append helper, run-state update helper, validation decision/approval/review-pack builders, schema/file helper, run-state helper, test module root, approval/gate/provider scenario tests, shared test helper facade, JSON builders, fixture lifecycle는 분리됨. |
| Observability 세부 분할 | `packages/star-control-observability/src/constants.rs`, `error.rs`, `audit.rs`, `audit/io.rs`, `audit/time.rs`, `audit/validation.rs`, `cost.rs`, `cost/budget.rs`, `cost/io.rs`, `cost/paths.rs`, `cost/validation.rs`, `tests.rs`, `tests/audit.rs`, `tests/cost.rs`, `tests/helpers.rs`로 audit event writer root, audit JSONL IO, audit schema validation, audit timestamp helper, cost metric writer root, budget threshold/evaluation, provider metric IO, provider path guard, cost metric schema/semantic validation, public error, contract constants, test module root, audit/cost scenario tests, schema/temp project/StateStore test helper는 분리됨. `apps/star-daemon`은 HTTP approve/cancel/resume control action을 AuditEventWriter에 자동 연결하고, fake/local/cloud provider execution path는 schema-valid `cost-metric.json` sidecar를 남기며, cloud provider hard budget enforcement는 transport 전 block으로 연결된다. remaining provider audit/redaction wiring, external billing/quota 조회, release readiness finalization은 계속 `RESERVED`다. |
| Security redaction 세부 분할 | `packages/star-control-security/src/constants.rs`, `model.rs`, `redact.rs`, `report.rs`, `tests.rs`로 redaction contract constants, RedactionFinding/RedactionOutcome model, recursive JSON redaction traversal, RedactionReport builder, schema-valid redaction tests는 분리됨. `packages/star-control-state`는 `StateStore::write_redaction_report_json`으로 schema-valid RedactionReport를 job 내부 `audit/<file>` artifact로 저장하고 ArtifactRef를 반환한다. `star-control report --json`은 report output을 shared redaction utility로 처리하고 finding이 있으면 `audit/redaction-report-<stage>.json`을 저장한다. `packages/star-control-provider/src/provider_redaction.rs`는 fake/local/cloud provider output artifact를 저장 전 redaction하고 finding이 있으면 `audit/provider-redaction-<provider>-<artifact>.json`을 저장한다. release/deploy/publish external executor, Local/Cloud AI live connector execution은 계속 `RESERVED`다. |
| daemon queue/app 세부 분할 | `packages/star-control-daemon/src/constants.rs`, `config.rs`, `error.rs`, `error/display.rs`, `error/source.rs`, `io.rs`, `queue.rs`, `queue/approval.rs`, `queue/enqueue.rs`, `queue/fields.rs`, `queue/schema.rs`, `queue/state.rs`, `tests.rs`, `tests/state.rs`, `tests/enqueue.rs`, `tests/approval.rs`, `tests/helpers.rs`로 daemon queue skeleton의 contract constants, public config/error boundary, public error display/source mapping, atomic state IO helper, queue facade, approval response guard, enqueue flow, provider instance path preservation, field helper, schema validation, daemon state IO, test root, default-state/enqueue/approval scenario tests, shared fixture helper는 분리됨. `apps/star-daemon`은 process/API surface, `fake-default` scheduler tick, local-process scheduler executor를 제공한다. socket/remote exposure/cloud-live scheduler executor는 계속 `RESERVED`다. |
| Local process provider test support 세부 분할 | `packages/star-control-provider/src/local_process/tests.rs`는 child-process helper test names를 보존하는 test root이며 `src/local_process/tests/support.rs`, `support/env.rs`, `support/execution.rs`, `support/registry.rs`, `support/request.rs`, `support/temp.rs`로 support facade, env guard, execute-with-command fixture, registry builder, request/run-state fixture, temp project/schema/store helper는 분리됨. |
| local/cloud provider | adapter 구현은 `packages/star-control-provider/`에 둔다. provider crate root는 public re-export 중심이며 registry domain root/manifest/instance/capability/document/registry collection/error root/error Display-source helper/loader/loader document loaders/loader registry assembly/loader contract IO/field extraction/path guard/YAML parser entrypoint/line scanner/block parser/key-value pair parser/scalar parser/registry test root/load contract-error-YAML scenario tests/registry test fixture helper, fake adapter/simulation/model root/error root/error Display-source helper/request/result/execution/validation/output helper/scenario tests/test fixture helper, local process constants/policy root/policy executable checker/policy field parser/runner/evidence/sidecars root/sidecars response-artifacts-files/test root/execution-policy-cancellation-forbidden-action scenario tests/local process test_support facade-env-execution-registry-request-temp helpers, conformance checker root/checker artifact collection root/checker cloud-required-declared-optional-stderr-file-existence helpers/checker stored artifact validation/error/helper root/helper artifact list/helper JSON field-ref contract/helper path policy/helper schema-backed artifact validation/types/test root/path-policy-response-consistency-cloud-sidecar-fixture tests, cloud constants/policy root/policy credential-ref and raw-credential helper/policy JSON value helper/CLI transport root/CLI command field parser/CLI command policy/CLI process runner/CLI argument renderer/API artifact root/API artifact response/transport/stdout/naming helper/sidecar root/sidecar cost-log-privacy-ref-response facade/sidecar response CLI-preflight helpers/fixture-path-schema IO helper/manifest classifier/CLI adapter/API offline adapter facade/API offline fixture request-response helper/API offline provider output write helper/live-approval root/live-approval artifact prep and sidecar writer/preflight adapter/cloud test root/preflight/API root/API offline fixture/API live approval/API unsafe fixture path/CLI scenario tests/cloud test_support facade/env/execution/io/registry/request/temp helper, OpenAI-compatible request root/request body-error-field helpers/response root/Chat Completions parser/Responses API parser/field extraction helper/test root/request tests/response tests는 module 분리됨. live provider call과 외부 실행 확장은 별도 승인 전까지 `RESERVED`다. |
| provider cost/budget integration | `packages/star-control-provider/src/provider_cost.rs`는 fake/local-process provider용 schema-valid zero-cost metric sidecar helper를 제공한다. fake/local/cloud provider output은 `provider-output/{provider_instance_id}/cost-metric.json`을 남기며, productization E2E smoke는 CLI fake run의 cost metric sidecar를 확인한다. `CloudProviderPolicyDecision`은 `budget.max_estimated_cost` hard limit 초과를 `cloud_budget_estimated_cost_exceeded` blocked result로 정규화한다. 외부 billing/quota 조회는 계속 `RESERVED`다. |
| daemon process/HTTP API server/browser UI app | daemon queue skeleton과 `apps/star-daemon` process/API surface, loopback-only HTTP API server, HTTP control action audit integration, `fake-default` queue scheduler tick, local-process scheduler executor, API read-only/control mutation service, UI read-only/control shell model, 정적 `apps/star-control-ui` browser app까지 구현됨. remote exposure와 cloud-live scheduler executor/Local·Cloud AI live connector execution은 별도 slice까지 `RESERVED`다. |
| release automation | dry-run/approval plan surface와 approval-gated local result artifact executor는 `ReleaseAutomationPlanner`와 `star-control release --action`으로 구현됨. 실제 external signing/publish/deploy/repository settings mutation executor는 별도 승인 전까지 `RESERVED`다. |
