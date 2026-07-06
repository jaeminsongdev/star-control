# Refactoring and Hardcoding Guidelines

## 목적

이 문서는 Star-Control 전체 리팩토링 전에 적용할 구조 정리 기준과 하드코딩 판정 기준을 고정한다. 목표는 기능 추가가 아니라 책임 경계, 파일 구조, 값의 소유 위치를 명확히 하는 것이다.

이번 기준은 아래 작업을 포함하지 않는다.

- daemon process 구현
- HTTP API server 구현
- browser UI app 구현
- provider live call 실행
- destructive recovery action 실행
- release/deploy/publish automation
- GitHub repository settings 변경
- 새 dependency 또는 Cargo 외 package manager 도입

## 현재 감사 요약

현재 repository는 core runtime 구현과 장기 scaffold가 같은 `packages/` 층에 섞여 있다.

```text
Cargo workspace crate: 14개
README-only scaffold package: 22개
```

가장 큰 Rust 파일은 module split 우선 후보다. 아래 순서는 현재 repository scan 기준이며, 이미 완료된 split은 다시 같은 slice에서 건드리지 않는다.

| 우선순위 | 파일 | 현재 줄 수 | 현재 상태 |
|---|---:|---:|---|
| 1 | `packages/star-sentinel/src/tests/evaluation.rs` | 160 | Star Sentinel evaluator scenario tests가 남아 있음. rule scenario branch가 더 늘 때 split 후보 |
| 2 | `packages/star-control-provider/src/registry_yaml/block.rs` | 155 | registry YAML block parser가 남아 있음. parser branch가 더 늘 때 split 후보 |
| 3 | `packages/star-control-release/src/tests/consistency.rs` | 154 | release consistency scenario tests가 남아 있음. consistency branch가 더 늘 때 split 후보 |
| 4 | `packages/star-control-provider/src/fake/adapter.rs` | 152 | fake provider adapter execution orchestration이 남아 있음. adapter branch가 더 늘 때 split 후보 |
| 5 | `packages/star-control-provider/src/conformance/tests/fixture.rs` | 149 | conformance fixture helper가 남아 있음. fixture branch가 더 늘 때 split 후보 |

2026-07-06 completion audit 기준으로 즉시 분리해야 할 초대형 Rust 파일은 남아 있지 않다. 남은 상위 후보는 149~160줄 범위이므로 새 책임 branch가 추가될 때만 별도 split 후보로 다룬다. 같은 audit에서 `cargo fmt --check`, `cargo check --workspace`, `cargo test --workspace`, `powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1`, `git diff --check`가 통과했고, production Rust/scoped config-app-builtin-spec-example-script/absolute path hardcoding scan도 clean 상태다.

`packages/star-control-provider/src/lib.rs`는 42줄까지 축소되어 public re-export와 module declaration만 남았다. provider registry domain root, manifest/instance/capability/document/registry collection type modules, error, loader, YAML subset parser entrypoint/line scanner/block parser/key-value pair parser/scalar parser, tests는 `registry_domain`, `registry_domain/`, `registry_error`, `registry_error/`, `registry_loader`, `registry_loader/`, `registry_yaml`, `registry_yaml/`, `registry_tests`, `registry_tests/` module로 분리된 상태다. `registry_error.rs`는 75줄까지 축소되어 public `ProviderRegistryError` enum root만 남고 Display message formatting과 `Error::source` mapping은 `registry_error/display.rs`, `registry_error/source.rs`로 분리된 상태다. registry loader root는 public facade와 constructor/getter 중심이고 registry document/manifest/instance/capability loaders, registry assembly, contract IO/schema validation, field extraction, path guard는 `registry_loader/documents.rs`, `registry_loader/assembly.rs`, `registry_loader/contract_io.rs`, `registry_loader/fields.rs`, `registry_loader/paths.rs`로 분리한 상태다. `registry_tests.rs`는 test module root이고 registry load contract, error/path/schema guard, YAML parser scenario tests는 `registry_tests/contracts.rs`, `registry_tests/errors.rs`, `registry_tests/yaml.rs`, registry loader/temp JSON fixture helper는 `registry_tests/helpers.rs`로 분리된 상태다.
`packages/star-sentinel/src/lib.rs`는 39줄까지 축소되어 public re-export와 module declaration만 남았다. `model.rs`는 public model re-export root이고 decision/severity parsing, P0 registry, diagnostic/evaluation DTO, artifact refs, ledger event model은 `model/` 하위 module로 분리된 상태다. Star Sentinel evaluator root는 public `P0Evaluator` orchestration 중심이고 fixture outcome, matcher facade, path matcher, secret matcher, P0 rule evaluation root, rule family evaluators, diagnostic construction helper, changed-line iterator helper는 `evaluator/`와 `evaluator/rules/` 하위 module로 분리된 상태다. Star Sentinel review-pack root는 public builder/validator/writer re-export 중심이고 artifact assembly, signal derivation, markdown rendering, StateStore writer는 `review_pack/` 하위 module로 분리된 상태다. Star Sentinel selfcheck root는 public `run_selfcheck` orchestration 중심이고 legacy alias scan, manifest outputs, P0 registry/schema/fixture contract checks, YAML list helper는 `selfcheck/` 하위 module로 분리된 상태다. Star Sentinel test root와 fixture helper는 `tests.rs`에 남기고 evaluator/gate/review-pack/ledger/selfcheck scenario tests는 `tests/` 하위 module로 분리된 상태다.
`packages/star-control-provider/src/cloud.rs`는 15줄까지 축소되어 public re-export와 module declaration만 남았다. cloud manifest classifier, CLI adapter, API offline adapter, live-approval handoff helper, preflight adapter는 `cloud/` 하위 module로 분리된 상태다. cloud provider policy root는 `cloud_policy.rs`이고 raw credential detection/credential ref allowlist helper, JSON value pointer helper는 `cloud_policy/credentials.rs`, `cloud_policy/value.rs`로 분리된 상태다. cloud API offline adapter root는 59줄까지 축소되어 manifest/policy/preflight/live-approval branch orchestration만 남았고, offline fixture request/response 준비와 provider output write orchestration은 `cloud/api_offline_adapter/fixture.rs`, `output.rs`로 분리된 상태다. cloud API live approval root는 live approval-required execution flow만 남기고 request/transport/approval artifact preparation과 plan artifact writer는 `cloud/api_live/artifacts.rs`, privacy/cost/stdout/stderr sidecar writer는 `cloud/api_live/sidecars.rs`로 분리된 상태다. cloud CLI transport root는 `cloud_cli.rs`이고 command field parser, command policy/timeout/env passthrough guard, process runner/timeout wait, argument renderer는 `cloud_cli/` 하위 module로 분리한 상태다. cloud API artifact root는 `cloud_api_artifacts.rs`이고 response result builder, HTTP transport/approval artifact builder, stdout helper, API/kind naming helper는 `cloud_api_artifacts/` 하위 module로 분리한 상태다. cloud sidecar root는 `cloud_sidecars.rs`이고 response facade, CLI response builder, preflight response builder, privacy handoff, cost metric, stdout/stderr, artifact ref/planned output helper는 `cloud_sidecars/`와 `cloud_sidecars/response/` 하위 module로 분리한 상태다. cloud production path의 `request.json`/`response.json` 파일명은 `cloud_constants.rs` 중앙 상수로 모았다.
`packages/star-control-provider/src/cloud/tests.rs`는 24줄까지 축소되어 test module root와 CLI child-process helper만 남았다. preflight/API/CLI transport scenario tests는 `cloud/tests/preflight.rs`, `cloud/tests/api.rs`, `cloud/tests/cli.rs`로 분리했고, API offline fixture/live approval/unsafe fixture path scenario는 `cloud/tests/api/` 하위 module로 분리했다. cloud test support root는 `cloud/test_support.rs`이고 env guard, execution fixture, JSON reader, registry builder, request fixture, temp/schema path helper는 `cloud/test_support/env.rs`, `execution.rs`, `io.rs`, `registry.rs`, `request.rs`, `temp.rs`로 분리된 상태다.
`packages/star-control-provider/src/openai_compatible.rs`는 14줄까지 축소되어 public re-export와 module declaration만 남았다. OpenAI-compatible request builder root는 `openai_compatible/request.rs`이고 request body builder, request error, endpoint/API field helper는 `openai_compatible/request/body.rs`, `error.rs`, `fields.rs`로 분리된 상태다. response parser root, Chat Completions response parser, Responses API parser, shared field extraction helper, test root는 `openai_compatible/` 하위 module로 분리되어 있고 request builder tests와 response parser tests는 `openai_compatible/tests/` 하위 module로 분리된 상태다. public `OpenAiCompatible*` 타입 이름은 기존 cloud adapter contract 호환을 위해 유지한다.
`packages/star-control-provider/src/conformance.rs`는 24줄까지 축소되어 public re-export와 module declaration만 남았다. ProviderConformanceChecker orchestration은 `conformance/checker.rs`, checked artifact collection root와 optional stderr/declared artifact/cloud-required artifact/file-existence helper는 `conformance/checker/artifacts.rs`, `conformance/checker/artifacts/optional.rs`, `declared.rs`, `cloud.rs`, `verify.rs`로 분리된 상태다. stored response/cloud sidecar artifact validation helper는 `conformance/checker/stored.rs`로 분리된 상태다. public error는 `conformance/error.rs`, profile/report model은 `conformance/types.rs`, helper root는 `conformance/helpers.rs`, artifact list checks, JSON field/ref contract checks, path policy, schema-backed artifact validation은 `conformance/helpers/` 하위 module로 분리된 상태다. provider path policy, response consistency, cloud sidecar schema scenario와 shared fixture는 `conformance/tests/` 하위 module로 분리한 상태다.
`packages/star-control-security/src/lib.rs`는 10줄까지 축소되어 public re-export와 module declaration만 남았다. Redaction contract constants는 `constants.rs`, RedactionFinding/RedactionOutcome model은 `model.rs`, recursive JSON redaction traversal은 `redact.rs`, RedactionReport builder는 `report.rs`, schema-valid redaction scenario tests는 `tests.rs`로 분리된 상태다.
`packages/star-control-provider/src/fake.rs`는 13줄까지 축소되어 public re-export와 module declaration만 남았다. fake adapter execution flow는 `fake/adapter.rs`, fake simulation state/payload helper는 `fake/simulation.rs`, model root는 `fake/model.rs`, provider adapter error root/request/result/execution context/contract validation helper는 `fake/model/` 하위 module로 분리했다. fake provider adapter error Display/source mapping은 `fake/model/error/display.rs`, `fake/model/error/source.rs`로 분리한 상태다. provider output path/overwrite guard helper는 `fake/output.rs`, scenario tests는 `fake/tests.rs`, test fixture/request/temp-store helper는 `fake/tests/helpers.rs`로 분리된 상태다.
`packages/star-control-provider/src/local_process.rs`는 99줄까지 축소되어 adapter orchestration 중심으로 남았다. constants, command policy root, command policy field/timeout parser, executable allow/deny checker, process runner/cancel/timeout wait, forbidden-action evidence, sidecar facade는 `local_process/` 하위 module로 분리했다. local process sidecar root는 6줄 facade이며 response JSON builder, planned output/artifact ref helper, output file creation helper는 `local_process/sidecars/response.rs`, `artifacts.rs`, `files.rs`로 분리한 상태다. local process test root는 36줄까지 축소되어 child-process helper test names를 유지하고, execution/policy/cancellation/forbidden-action scenario tests는 `local_process/tests/` 하위 module로 분리한 상태다. local process test support root는 `local_process/tests/support.rs`이고 env guard, execution fixture, registry builder, request/run-state fixture, temp/schema/store helper는 `local_process/tests/support/env.rs`, `execution.rs`, `registry.rs`, `request.rs`, `temp.rs`로 분리된 상태다.
`packages/star-control-ui/src/lib.rs`는 14줄까지 축소되어 public re-export와 module declaration만 남았다. read-only shell orchestration은 `read_only.rs`, read-only API artifact/report/release helper는 `read_only/api.rs`, browser control shell orchestration은 `browser.rs`, view model builder root는 `view.rs`, view state/artifact/approval helper는 `view/state.rs`, `view/artifacts.rs`, `view/approval.rs`, schema/API helper는 `helpers.rs`, UI contract constants는 `constants.rs`, control action list는 `control_actions.rs`, public error는 `error.rs`, UI test root는 `tests.rs`, read-only/browser scenario tests는 `tests/read_only.rs`, `tests/browser.rs`, shared UI test helper facade는 `tests/helpers.rs`, project/store helper, UI shell helper, job/report/approval/release fixture helper는 `tests/helpers/` 하위 module로 분리된 상태다. UI read-only scenario test root는 test name wrapper이며 list/detail/release/approval/redaction/error scenario bodies는 `tests/read_only/` 하위 module로 분리된 상태다.
`packages/star-control-api/src/read_only.rs`는 92줄까지 축소되어 service registration과 GET routing root만 남았다. daemon state, project list, job detail/events, report, release readiness, API envelope/schema validation helper는 `read_only/daemon.rs`, `read_only/projects.rs`, `read_only/jobs.rs`, `read_only/reports.rs`, `read_only/release.rs`, `read_only/envelope.rs`로 분리된 상태다.
`packages/star-control-api/src/tests.rs`는 135줄까지 축소되어 shared API fixture/helper와 test module root만 남았다. read-only endpoint test root는 `tests/read_only.rs`, projects/daemon/reports/release/errors scenario tests와 assertion/state snapshot helper는 `tests/read_only/` 하위 module, control mutation tests는 `tests/control.rs`로 분리된 상태다.
`packages/star-control-router/src/lib.rs`는 12줄까지 축소되어 public re-export와 module declaration만 남았다. RouterEngine orchestration은 `engine.rs`, router schema/provider contract constants는 `constants.rs`, schema validation과 field extraction helper는 `contract.rs`로 분리된 상태다. request analysis module root는 5줄이며, analysis type root는 `analysis/types.rs`, change type enum, policy/route decision enum, request analysis DTO, size/risk scale enum, stage selection helper는 `analysis/types/change.rs`, `decision.rs`, `request.rs`, `scale.rs`, `stages.rs`로 분리된 상태다. keyword classification orchestration root는 `analysis/classification.rs`, normalized haystack helper와 keyword matching helper는 `analysis/classification/` 하위 module, keyword rule catalog root는 `analysis/classification/rules/catalog.rs`, routine/safety rule family는 `analysis/classification/rules/catalog/routine.rs`, `safety.rs`, size/risk/profile/approval policy orchestration은 `analysis/policy.rs`, change-type별 policy mapping은 `analysis/policy/mapping.rs`로 분리된 상태다. workspec root는 facade이며 stage WorkSpec JSON builder, route assignment/path map builder, stage role helper, workspec artifact path helper는 `workspec/` 하위 module로 분리된 상태다. `tests.rs`는 test module root이고 RouterEngine scenario tests와 shared helper는 `tests/scenarios.rs`, `tests/helpers.rs`로 분리된 상태다.
`packages/star-control-cli/src/lib.rs`는 86줄까지 축소되어 public entrypoint와 shared CLI input helper만 남았다. command modules, output/error/config/args, test support, unit tests는 별도 module로 분리된 상태다. `packages/star-control-cli/src/args.rs`는 5줄 re-export root이며 argument model, option dispatch, parse loop는 `src/args/model.rs`, `options.rs`, `parser.rs`로 분리된 상태다. `packages/star-control-cli/src/run.rs`는 run command orchestration root이며 registry loading, provider override, run artifact list, routed-state/report helper, provider execution/report branch, run-specific constants는 `src/run/` 하위 module로 분리된 상태다. `packages/star-control-cli/src/providers.rs`는 providers list/show command orchestration root이며 reserved option guard, builtin registry loader, provider summary renderer는 `src/providers/options.rs`, `registry.rs`, `summary.rs`로 분리된 상태다. `packages/star-control-cli/src/control.rs`는 68줄 module root이며 approve/cancel/resume command flow와 shared control helper는 `src/control/` 하위 module로 분리된 상태다. `packages/star-control-cli/src/sentinel.rs`는 31줄 dispatch root이며 `src/sentinel/commands.rs`는 8줄 re-export root다. Sentinel check/gate/review-pack/selfcheck command flow는 `src/sentinel/commands/`, evaluation, option validation, path/status helpers는 `src/sentinel/` 하위 module로 분리된 상태다. `packages/star-control-cli/tests/v0_fake_flow/support.rs`는 17줄 root이며 fixture lifecycle, CLI runner, validation flow root, final report writer는 `tests/v0_fake_flow/` 하위 module로 분리된 상태다. v0 validation fixture builders, Sentinel gate/review-pack writer, approval response fixture는 `tests/v0_fake_flow/validation/` 하위 module로 분리된 상태다. `packages/star-control-cli/src/tests.rs`는 6줄 module root이며 run/providers/sentinel/release/recover/control tests와 control command assertion/helper는 `src/tests/` 하위 module로 분리된 상태다. Sentinel command scenario test root는 `src/tests/sentinel.rs`에 두고 command success/error scenario bodies는 `src/tests/sentinel/commands.rs`, `errors.rs`로 분리된 상태다.
`packages/star-control-schema/src/lib.rs`는 12줄까지 축소되어 public re-export와 module declaration만 남았다. schema load/document load/file validation helper는 `loader.rs`, public error는 `error.rs`, Schema/Validation DTO와 JSON type helper는 `types.rs`, subset validator root는 `validator.rs`, scalar/object-array/path/pattern validator helper는 `validator/` 하위 module, validator unit test root는 `tests.rs`, schema/temp file fixture helper는 `tests/helpers.rs`, keyword/structure/loading scenario tests는 `tests/keywords.rs`, `tests/structures.rs`, `tests/loading.rs`로 분리된 상태다.
`packages/star-control-release/src/tests.rs`는 5줄까지 축소되어 test module root만 남았다. readiness writer, review pack, consistency/evidence, profile readiness, M9/complete implementation audit tests는 `tests/` 하위 module로 분리된 상태다.
`packages/star-control-release/src/audits.rs`는 5줄까지 축소되어 audit builder re-export root만 남았다. M9 readiness audit check/builder는 `audits/m9.rs`, complete implementation audit check/builder는 `audits/complete.rs`로 분리된 상태다.
`packages/star-control-release/src/support.rs`는 17줄까지 축소되어 support helper facade와 re-export만 남았다. release check/status helper, evidence path guard, profile/M9/complete implementation name/blocker normalization, release text/version parsing, timestamp helper는 `support/checks.rs`, `support/evidence.rs`, `support/names.rs`, `support/text.rs`, `support/time.rs`로 분리된 상태다.
`packages/star-control-release/src/writer.rs`는 99줄까지 축소되어 public `ReleaseReadinessWriter` facade만 남았다. release readiness JSON file IO, readiness/check JSON builder, schema/status validation은 `writer/io.rs`, `writer/model.rs`, `writer/validation.rs`로 분리된 상태다.
`packages/star-control-observability/src/lib.rs`는 15줄까지 축소되어 public re-export와 module declaration만 남았다. audit writer, cost metric/budget writer, public error, constants, tests는 별도 module로 분리된 상태다. `audit.rs`는 `AuditEventWriter` root와 event builder 중심이고 append/read JSONL IO, schema validation, timestamp helper는 `audit/io.rs`, `audit/validation.rs`, `audit/time.rs`로 분리한 상태다. `cost.rs`는 `CostMetricWriter` root와 metric builder 중심이고, budget threshold/evaluation, provider metric IO, provider path guard, schema/semantic validation helper는 `cost/` 하위 module로 분리한 상태다. `tests.rs`는 test module root이고 audit/cost scenario tests는 `tests/audit.rs`, `tests/cost.rs`, schema/temp project/StateStore fixture helper는 `tests/helpers.rs`로 분리된 상태다.
`packages/star-control-daemon/src/lib.rs`는 12줄까지 축소되어 public re-export와 module declaration만 남았다. daemon config, constants, public error, atomic state IO helper, queue service, tests는 별도 module로 분리된 상태다. `error.rs`는 public `DaemonError` enum root이고 Display message formatting과 `Error::source` mapping은 `error/display.rs`, `error/source.rs`로 분리된 상태다. `queue.rs`는 `DaemonQueue` facade이고 approval response guard, enqueue flow, JSON field helper, schema validation, daemon state IO는 `queue/approval.rs`, `queue/enqueue.rs`, `queue/fields.rs`, `queue/schema.rs`, `queue/state.rs`로 분리된 상태다. `tests.rs`는 28줄까지 축소되어 기존 daemon queue test name wrapper만 남았다. daemon default state, enqueue/terminal/duplicate scenario, approval response scenario, shared fixture/helper는 `tests/state.rs`, `tests/enqueue.rs`, `tests/approval.rs`, `tests/helpers.rs`로 분리된 상태다. daemon process, socket, provider scheduling은 계속 `RESERVED`다.
`packages/star-control-validation/src/lib.rs`는 14줄까지 축소되어 public re-export와 module declaration만 남았다. public DTO는 `types.rs`, ValidationEngine facade와 schema helper는 `engine.rs`, provider response check, Star Sentinel gate evaluation, outcome writer, approval response gate는 `engine/` 하위 module로 분리된 상태다. `engine/gate.rs`는 Star Sentinel approval decision orchestration root이고 approval parsing/inconsistency helper와 normal/failed outcome builder는 `engine/gate/` 하위 module로 분리한 상태다. `engine/writer.rs`는 `write_outcome` orchestration root이고 validation-run artifact write/reference, gate event append, run-state update helper는 `engine/writer/` 하위 module로 분리한 상태다. validation test root는 module declaration과 helper import만 남기고 approval response gate, Star Sentinel gate/outcome writer, provider response scenario tests는 `tests/approval.rs`, `tests/gate.rs`, `tests/provider.rs`로, test fixture facade와 JSON builder/fixture lifecycle helper는 `tests/helpers.rs`, `tests/helpers/builders.rs`, `tests/helpers/fixture.rs`로 분리한 상태다.
`packages/star-control-cli/src/test_support.rs`는 12줄까지 축소되어 test fixture re-export와 module declaration만 남았다. temp project/repo-root helper는 `test_support/project.rs`, local process provider instance fixture는 `test_support/local_process.rs`, Star Sentinel input fixture는 `test_support/sentinel.rs`, approval/release/recovery command fixture는 `test_support/approval.rs`, `release.rs`, `recovery.rs`로 분리된 상태다.
`packages/star-control-cli/src/control/helpers.rs`는 13줄까지 축소되어 control helper re-export와 module declaration만 남았다. approval response validation/state transition helper는 `control/helpers/approval.rs`, job artifact/schema read helper는 `control/helpers/artifacts.rs`, run-state mutation helper는 `control/helpers/state.rs`, CLI event writer는 `control/helpers/events.rs`, timestamp helper는 `control/helpers/time.rs`로 분리된 상태다.
`packages/star-control-cli/src/read_commands.rs`는 read-only command facade로 축소되어 status/report/recover public wrapper만 re-export한다. status command, report command, release-readiness report branch, recover inspect-only branch는 `read_commands/status.rs`, `read_commands/report.rs`, `read_commands/release.rs`, `read_commands/recover.rs`로 분리된 상태다.
`packages/star-control-cli/src/tests/run.rs`는 28줄까지 축소되어 기존 run command test name wrapper만 남았다. fake flow, local-process provider execution, error scenario, shared assertion/config helper는 `tests/run/fake.rs`, `tests/run/local_process.rs`, `tests/run/errors.rs`, `tests/run/helpers.rs`로 분리된 상태다.
`packages/star-control-schema/tests/canonical_examples.rs`는 35줄까지 축소되어 integration test runner만 남았다. canonical schema/example case registry는 `tests/canonical_examples/cases.rs`와 `cases/core.rs`, `provider_execution.rs`, `surface.rs`, `config.rs`, `sentinel.rs`로 domain별 분리된 상태다.
`packages/star-control-execution/src/lib.rs`는 15줄까지 축소되어 public re-export와 module declaration만 남았다. ExecutionEngine root는 `engine.rs`에 남기고 provider dispatch, execution request/stage guard, state/event write helper는 `engine/provider.rs`, `engine/request.rs`, `engine/state.rs`로 분리한 상태다. public error enum root는 `error.rs`, Display/source mapping은 `error/display.rs`, `error/source.rs`, public outcome/assignment model은 `types.rs`, contract/schema helper는 `contract.rs`, run-state/status helper는 `state.rs`로 분리된 상태다. `tests.rs`는 36줄 test module root와 child-process helper만 남고 fake provider, local process, cloud provider execution tests는 `tests/` 하위 module로 분리된 상태다. local-process execution test root는 `tests/local_process.rs`이며 success execution, timeout, cancellation, forbidden-action, conformance matrix scenario는 `tests/local_process/execution.rs`, `timeout.rs`, `cancellation.rs`, `forbidden_action.rs`, `conformance.rs`로 분리된 상태다. cloud provider execution test root는 `tests/cloud.rs`이며 CLI transport, offline API fixture, live approval-required scenario는 `tests/cloud/cli.rs`, `api_offline.rs`, `api_live.rs`로 분리된 상태다. execution test support root는 10줄 facade이며 temp/env/path helper, fixture/registry setup, local-process conformance runner, output assertion helper는 `test_support/`와 `test_support/local_process/` 하위 module로 분리된 상태다. provider live call 확장은 계속 `RESERVED`다.
`packages/star-control-state/src/lib.rs`는 18줄까지 축소되어 public re-export와 module declaration만 남았다. StateStore public type은 `types.rs`, contract constants는 `constants.rs`, public error enum root는 `error.rs`, Display/source mapping은 `error/display.rs`, `error/source.rs`로 분리된 상태다. store module root는 `store.rs`, open/getter lifecycle은 `store/lifecycle.rs`, job allocation/create/list/resume flow는 `store/jobs.rs`, core artifact save/load helper는 `store/core_artifacts.rs`, job-relative path resolution은 `store/paths.rs`, event append/read는 `events.rs`로 분리된 상태다. output helper root는 `outputs.rs`이고 artifact ref/register, provider output, tool output, approval, review-pack, validation, tmp writer는 `outputs/refs.rs`, `outputs/provider.rs`, `outputs/tool.rs`, `outputs/approval.rs`, `outputs/review.rs`, `outputs/validation.rs`, `outputs/tmp.rs`로 분리된 상태다. artifact helper root는 `artifacts.rs`이고 atomic write, JSON/text artifact IO, platform replace, schema validation, timestamp helper는 `artifacts/atomic.rs`, `artifacts/json.rs`, `artifacts/replace.rs`, `artifacts/schema.rs`, `artifacts/time.rs`로 분리된 상태다. `tests.rs`는 test module root와 shared fixture helper만 남고 store test root는 `tests/store.rs`, job/event/path scenario tests는 `tests/store/jobs.rs`, `tests/store/events.rs`, `tests/store/paths.rs`, artifact test root는 `tests/artifacts.rs`, output-dir/ref/safety/writer scenario tests는 `tests/artifacts/dirs.rs`, `refs.rs`, `safety.rs`, `writers.rs`, recovery inspection tests는 `tests/recovery.rs`로 분리된 상태다. destructive recovery action은 계속 `RESERVED`다.
`packages/star-control-api/src/control.rs`는 83줄까지 축소되어 GET/POST routing과 public service facade만 남았다. `control/mutations.rs`는 mutation module root와 공용 invalid-request helper만 남기고 approve/cancel/resume flow는 `control/mutations/approve.rs`, `control/mutations/cancel.rs`, `control/mutations/resume.rs`로 분리했다. approve mutation은 `control/mutations/approve.rs`를 orchestration root로 유지하고 request body parsing, approval request/response build/validation/write helper, approval event/success payload helper는 `control/mutations/approve/request.rs`, `response.rs`, `event.rs`로 분리했다. resume mutation은 `control/mutations/resume.rs`를 orchestration root로 유지하고 approval artifact load/next-action helper와 resume event/success payload helper는 `control/mutations/resume/artifacts.rs`, `event.rs`로 분리했다. `control/helpers.rs`는 helper re-export/module declaration root로 줄이고 request body parsing, API control event writer, run-state transition/approval matching, timestamp helper는 `control/helpers/body.rs`, `events.rs`, `state.rs`, `time.rs`로 분리했다. HTTP server/auth/remote exposure는 계속 `RESERVED`다.
하드코딩 감사에서 확인된 주요 후보는 아래와 같다.

- 절대 경로 예시는 이 기준 문서의 금지 예시 외에는 현재 스캔에서 남아 있지 않다. `examples/runs/J-0001/workspec-impl.md`의 sample `project_root`는 `<target-project-root>` placeholder로 정리했다.
- raw credential 형태의 문자열은 `packages/star-control-provider/src/cloud/tests/preflight.rs` redaction test fixture에 있다. secret redaction 검증 목적의 fixture로만 허용되며 artifact, log, report, runtime default로 이동하면 안 된다.
- provider-specific endpoint/model 예시는 `packages/star-control-provider/src/cloud/tests/api/`와 `packages/star-control-execution/src/tests/cloud/` test fixture에 있다. provider manifest/config/fixture에서 온 값으로 다뤄야 하며 core runtime default로 박으면 안 된다.
- PR 번호, CI command, 특정 날짜는 audit evidence 문서와 schema example/test fixture에 있다. 증거 문서와 fixture에는 허용되지만 runtime decision logic에 의존시키면 안 된다.

## 리팩토링 구조 기준

### 실제 구현과 scaffold 분리

`packages/`의 즉시 기준은 다음과 같다.

- Cargo workspace에 등록된 실제 Rust crate만 active implementation으로 본다.
- README-only `star-provider-*`, `star-transport-*`, `star-adapter-*`, `star-tool-*`, `star-*` scaffold는 post-core extension 후보로 본다.
- scaffold를 삭제하지 않는다. 격리가 필요하면 별도 slice에서 `extensions/` 같은 reserved 영역으로 이동하고 문서와 CI 경로를 함께 갱신한다.
- `packages/`를 `crates/`로 rename하는 대형 이동은 전체 경로 churn이 크므로 module split과 같은 slice에 섞지 않는다.

권장 장기 배치는 아래와 같다.

```text
packages/ 또는 crates/
  star-control-*/        # 실제 Cargo workspace crate
  star-sentinel/         # Star Sentinel 구현 crate

extensions/
  providers/
  transports/
  adapters/
  platform/
```

### crate 내부 module split

`src/lib.rs`는 public API와 re-export 중심으로 유지한다. 구현 세부는 책임별 module로 분리한다.

권장 module 경계:

```text
star-control-cli/src/
  lib.rs
  args.rs
  constants.rs
  config.rs
  error.rs
  output.rs
  run.rs
  run/
    artifacts.rs
    constants.rs
    execution.rs
    registry.rs
    route.rs
    state.rs
  providers.rs
  providers/
    options.rs
    registry.rs
    summary.rs
  read_commands.rs
  sentinel.rs
  sentinel/
    commands.rs
    evaluation.rs
    options.rs
    paths.rs
    status.rs
  control.rs
  control/
    approve.rs
    cancel.rs
    helpers.rs
    resume.rs
  test_support.rs
  tests.rs
  tests/
    run.rs
    providers.rs
    sentinel.rs
    release.rs
    recover.rs
    control.rs

star-control-cli/tests/
  v0_fake_flow.rs
  v0_fake_flow/
    cli.rs
    fixture.rs
    report.rs
    support.rs
    validation.rs

star-control-schema/src/
  lib.rs
  error.rs
  types.rs
  loader.rs
  validator.rs
  validator/
    compound.rs
    path.rs
    pattern.rs
    scalar.rs
  tests.rs
  tests/
    helpers.rs
    keywords.rs
    loading.rs
    structures.rs

star-control-daemon/src/
  lib.rs
  constants.rs
  config.rs
  error.rs
  error/
    display.rs
    source.rs
  io.rs
  queue.rs
  tests.rs

star-control-router/src/
  lib.rs
  error.rs
  types.rs
  analysis.rs
  analysis/
    types.rs
    types/
      change.rs
      decision.rs
      request.rs
      scale.rs
      stages.rs
    classification.rs
    policy.rs
  workspec.rs
  tests.rs

star-control-provider/src/
  lib.rs
  cloud.rs
  cloud_api_artifacts.rs
  cloud_api_artifacts/
    names.rs
    response.rs
    stdout.rs
    transport.rs
  cloud_cli.rs
  cloud_cli/
    fields.rs
    policy.rs
    process.rs
    render.rs
  cloud_constants.rs
  cloud_io.rs
  cloud_policy.rs
  cloud_sidecars.rs
  cloud_sidecars/
    cost.rs
    logs.rs
    privacy.rs
    refs.rs
    response.rs
    response/
      cli.rs
      preflight.rs
  cloud/
    api_live.rs
    api_offline_adapter.rs
    cli_adapter.rs
    manifest.rs
    preflight_adapter.rs
    tests.rs
    tests/
      api.rs
      api/
        live_approval.rs
        offline_fixture.rs
        path_policy.rs
      cli.rs
      preflight.rs
    test_support.rs
  local_process/
    constants.rs
    policy.rs
    policy/
      executable.rs
      fields.rs
    runner.rs
    evidence.rs
    sidecars.rs
    tests.rs
    tests/
      cancellation.rs
      execution.rs
      forbidden_action.rs
      policy.rs
  fake.rs
  fake/
    adapter.rs
    model.rs
    model/
      error.rs
      execution.rs
      request.rs
      result.rs
      validation.rs
    output.rs
    tests.rs
    tests/
      helpers.rs
  registry_domain.rs
  registry_domain/
    capability.rs
    document.rs
    instance.rs
    manifest.rs
    registry.rs
  registry_error.rs
  registry_error/
    display.rs
    source.rs
  registry_loader.rs
  registry_loader/
    assembly.rs
    contract_io.rs
    fields.rs
    paths.rs
  registry_yaml.rs
  registry_yaml/
    block.rs
    line.rs
    pair.rs
    parser.rs
    scalar.rs
  registry_tests.rs
  registry/
  adapters/
  conformance.rs
  conformance/
    checker.rs
    error.rs
    helpers.rs
    helpers/
      artifacts.rs
      fields.rs
      paths.rs
      schema.rs
    tests.rs
    tests/
      cloud_sidecar.rs
      fixture.rs
      path_policy.rs
      response_consistency.rs
    types.rs
  openai_compatible/
    request.rs
    response.rs
    response/
      chat.rs
      fields.rs
      responses.rs
    tests.rs
    tests/
      request.rs
      response.rs

star-sentinel/src/
  lib.rs
  constants.rs
  error.rs
  task.rs
  changed_lines.rs
  json_fields.rs
  model.rs
  evaluator.rs
  evaluator/
    fixture_outcome.rs
    matchers.rs
    rules.rs
  gate.rs
  review_pack.rs
  review_pack/
    artifact.rs
    markdown.rs
    signals.rs
    writer.rs
  ledger.rs
  readers.rs
  schema_io.rs
  selfcheck.rs
  selfcheck/
    aliases.rs
    contracts.rs
    manifest.rs
    yaml.rs
  tests.rs
  tests/
    evaluation.rs
    gate_artifacts.rs
    ledger.rs
    review_pack_artifacts.rs
    selfcheck.rs

star-control-state/src/
  lib.rs
  constants.rs
  error.rs
  error/
    display.rs
    source.rs
  types.rs
  paths.rs
  store.rs
  store/
    core_artifacts.rs
    jobs.rs
    lifecycle.rs
    paths.rs
  artifacts.rs
  events.rs
  outputs.rs
  recovery.rs
  recovery/
    inspection.rs
    issue.rs
    summary.rs
    tmp.rs
  tests.rs
  tests/
    artifacts.rs
    recovery.rs
    store.rs

star-control-api/src/
  lib.rs
  constants.rs
  request.rs
  error.rs
  paths.rs
  read_only.rs
  read_only/
    daemon.rs
    envelope.rs
    jobs.rs
    projects.rs
    release.rs
    reports.rs
  control.rs
  control/
    helpers.rs
    mutations.rs
    mutations/
      approve.rs
      cancel.rs
      resume.rs
  artifacts.rs
  tests.rs
  tests/
    control.rs
    read_only.rs

star-control-ui/src/
  lib.rs
  constants.rs
  error.rs
  helpers.rs
  read_only.rs
  read_only/
    api.rs
  browser.rs
  view.rs
  control_actions.rs
  tests.rs
  tests/
    read_only.rs
    browser.rs

star-control-observability/src/
  lib.rs
  constants.rs
  error.rs
  audit.rs
  cost.rs
  cost/
    budget.rs
    io.rs
    paths.rs
    validation.rs
  tests.rs

star-control-security/src/
  lib.rs
  constants.rs
  model.rs
  redact.rs
  report.rs
  tests.rs

star-control-release/src/
  lib.rs
  constants.rs
  error.rs
  consistency.rs
  profile.rs
  audits.rs
  audits/
    m9.rs
    complete.rs
  writer.rs
  review_pack.rs
  review_pack/
    markdown.rs
    storage.rs
  support.rs
  test_support.rs
  tests.rs
  tests/
    readiness.rs
    review_pack.rs
    consistency.rs
    profile.rs
    audits.rs

star-control-validation/src/
  lib.rs
  constants.rs
  error.rs
  error/
    display.rs
    source.rs
  types.rs
  engine.rs
  engine/
    approval.rs
    gate.rs
    provider.rs
    writer.rs
    writer/
      artifacts.rs
      events.rs
      run_state.rs
  builders.rs
  artifacts.rs
  state.rs
  tests.rs
  tests/
    approval.rs
    gate.rs
    provider.rs
    helpers.rs
    helpers/
      builders.rs
      fixture.rs

star-control-execution/src/
  lib.rs
  constants.rs
  error.rs
  types.rs
  contract.rs
  state.rs
  engine.rs
  engine/
    provider.rs
    request.rs
    state.rs
  tests.rs
  tests/
    cloud.rs
    fake.rs
    local_process.rs
  test_support.rs
  test_support/
    fixture.rs
    fixture/
      cloud.rs
      local_process.rs
      workspec.rs
    helpers.rs
    local_process.rs

star-control-release/src/
  lib.rs
  constants.rs
  error.rs
  consistency.rs
  profile.rs
  audits.rs
  audits/
    m9.rs
    complete.rs
  writer.rs
  review_pack.rs
  support.rs
  tests.rs
  test_support.rs
```

module split은 아래 조건을 지켜야 한다.

- public API 이름과 동작을 유지한다.
- 기존 schema/example/test contract를 약화하지 않는다.
- module split 중 새 기능을 추가하지 않는다.
- tests 하단 helper가 커지면 `test_support` 또는 integration test helper로 이동한다.
- 새 dependency 없이 표준 Rust module 이동으로 처리한다.

### core와 builtin 경계

- core runtime crate는 `star-control-*` namespace를 유지한다.
- Star Sentinel 구현은 `packages/star-sentinel`에 둔다.
- `builtin-tools/star-sentinel/`에는 manifest, policy, schema, fixture, template, corpus만 둔다.
- `builtin-providers/`에는 provider manifest와 capability profile만 둔다.
- provider 제품명은 core crate/module 이름에 넣지 않는다. 제품별 정보는 provider manifest, capability profile, config, fixture에 둔다.

### 진행 중인 module split 상태

- `packages/star-sentinel/src/lib.rs`에서 `constants`, `error`, `task`, `changed_lines`, `json_fields`, `model`, `evaluator`, `gate`, `review_pack`, `review_pack/artifact`, `review_pack/markdown`, `review_pack/signals`, `review_pack/writer`, `ledger`, `readers`, `schema_io`, `selfcheck`, `selfcheck/aliases`, `selfcheck/contracts`, `selfcheck/manifest`, `selfcheck/yaml`, `tests`를 분리했고, `model.rs`에서 `model/artifacts`, `model/decision`, `model/diagnostic`, `model/ledger_event`, `model/registry`를 분리했으며, `evaluator.rs`에서 `evaluator/fixture_outcome`, `evaluator/matchers`, `evaluator/matchers/path`, `evaluator/matchers/secret`, `evaluator/rules`, `evaluator/rules/allowed_paths`, `evaluator/rules/dependency`, `evaluator/rules/secrets`, `evaluator/rules/test_deletion`, `evaluator/rules/validator`, `evaluator/rules/diagnostics`, `evaluator/rules/lines`를 분리했고, `tests.rs`에서 `tests/evaluation`, `tests/gate_artifacts`, `tests/review_pack_artifacts`, `tests/ledger`, `tests/selfcheck`를 분리했다.
- `packages/star-control-provider/src/cloud.rs`에서 `cloud_constants`, `cloud_policy`, `cloud_policy/credentials`, `cloud_policy/value`, `cloud_cli`, `cloud_api_artifacts`, `cloud_api_artifacts/names`, `cloud_api_artifacts/response`, `cloud_api_artifacts/stdout`, `cloud_api_artifacts/transport`, `cloud_sidecars`, `cloud_sidecars/cost`, `cloud_sidecars/logs`, `cloud_sidecars/privacy`, `cloud_sidecars/refs`, `cloud_sidecars/response`, `cloud_sidecars/response/cli`, `cloud_sidecars/response/preflight`, `cloud_io`, `cloud/manifest`, `cloud/cli_adapter`, `cloud/api_offline_adapter`, `cloud/api_live`, `cloud/api_live/artifacts`, `cloud/api_live/sidecars`, `cloud/preflight_adapter`, `cloud/tests`, `cloud/tests/preflight`, `cloud/tests/api`, `cloud/tests/api/offline_fixture`, `cloud/tests/api/live_approval`, `cloud/tests/api/path_policy`, `cloud/tests/cli`, `cloud/test_support`, `cloud/test_support/env`, `cloud/test_support/execution`, `cloud/test_support/io`, `cloud/test_support/registry`, `cloud/test_support/request`, `cloud/test_support/temp`를 분리했다.
- `packages/star-control-provider/src/lib.rs`에서 `registry_domain`, `registry_domain/capability`, `registry_domain/document`, `registry_domain/instance`, `registry_domain/manifest`, `registry_domain/registry`, `registry_error`, `registry_loader`, `registry_loader/documents`, `registry_loader/assembly`, `registry_loader/contract_io`, `registry_loader/fields`, `registry_loader/paths`, `registry_yaml`, `registry_yaml/block`, `registry_yaml/line`, `registry_yaml/pair`, `registry_yaml/parser`, `registry_yaml/scalar`, `registry_tests`, `registry_tests/contracts`, `registry_tests/errors`, `registry_tests/helpers`, `registry_tests/yaml`을 분리했다.
- `packages/star-control-provider/src/local_process.rs`에서 `local_process/constants`, `local_process/policy`, `local_process/policy/executable`, `local_process/policy/fields`, `local_process/runner`, `local_process/evidence`, `local_process/sidecars`, `local_process/sidecars/response`, `local_process/sidecars/artifacts`, `local_process/sidecars/files`, `local_process/tests`를 분리했고, `local_process/tests.rs`에서 `tests/execution`, `tests/policy`, `tests/cancellation`, `tests/forbidden_action`, `tests/support`를 분리했다.
- `packages/star-control-provider/src/conformance.rs`에서 `conformance/checker`, `conformance/checker/artifacts`, `conformance/checker/artifacts/cloud`, `conformance/checker/artifacts/declared`, `conformance/checker/artifacts/optional`, `conformance/checker/artifacts/verify`, `conformance/checker/stored`, `conformance/error`, `conformance/helpers`, `conformance/helpers/artifacts`, `conformance/helpers/fields`, `conformance/helpers/paths`, `conformance/helpers/schema`, `conformance/types`, `conformance/tests`, `conformance/tests/cloud_sidecar`, `conformance/tests/fixture`, `conformance/tests/path_policy`, `conformance/tests/response_consistency`를 분리했다.
- `packages/star-control-provider/src/cloud_cli.rs`에서 `cloud_cli/fields`, `cloud_cli/policy`, `cloud_cli/process`, `cloud_cli/render`를 분리했다.
- `packages/star-control-provider/src/fake.rs`에서 `fake/adapter`, `fake/simulation`, `fake/model`, `fake/output`, `fake/tests`, `fake/tests/helpers`를 분리했고, `fake/model.rs`에서 `model/error`, `model/error/display`, `model/error/source`, `model/request`, `model/result`, `model/execution`, `model/validation`을 분리했다.
- `packages/star-control-provider/src/openai_compatible.rs`에서 `openai_compatible/request`, `openai_compatible/request/body`, `openai_compatible/request/error`, `openai_compatible/request/fields`, `openai_compatible/response`, `openai_compatible/response/chat`, `openai_compatible/response/fields`, `openai_compatible/response/responses`, `openai_compatible/tests`를 분리했고, `tests.rs`에서 `tests/request`, `tests/response`를 분리했다.
- `packages/star-control-ui/src/read_only.rs`에서 `read_only/api`를 분리했다.
- `packages/star-control-daemon/src/lib.rs`에서 `constants`, `config`, `error`, `error/display`, `error/source`, `io`, `queue`, `tests`를 분리했다.
- `packages/star-control-router/src/lib.rs`에서 `constants`, `contract`, `engine`, `error`, `types`, `tests`, `tests/helpers`, `tests/scenarios`, `analysis/types`, `analysis/types/change`, `analysis/types/decision`, `analysis/types/request`, `analysis/types/scale`, `analysis/types/stages`, `analysis/classification`, `analysis/classification/haystack`, `analysis/classification/rules`, `analysis/classification/rules/catalog`, `analysis/classification/rules/catalog/routine`, `analysis/classification/rules/catalog/safety`, `analysis/policy`, `analysis/policy/mapping`, `workspec`를 분리했다.
- `packages/star-control-ui/src/lib.rs`에서 `constants`, `error`, `helpers`, `read_only`, `browser`, `view`, `control_actions`, `tests`를 분리했고, `tests.rs`에서 `tests/read_only`, `tests/browser`를 분리했다.
- `packages/star-control-cli/src/lib.rs`에서 `constants`, `config`, `error`, `args`, `args/model`, `args/options`, `args/parser`, `output`, `run`, `providers`, `read_commands`, `sentinel`, `sentinel/commands`, `sentinel/commands/check`, `sentinel/commands/gate`, `sentinel/commands/review_pack`, `sentinel/commands/selfcheck`, `sentinel/evaluation`, `sentinel/options`, `sentinel/paths`, `sentinel/status`, `control`, `test_support`, `tests`를 분리했고, `run.rs`에서 `run/artifacts`, `run/constants`, `run/execution`, `run/registry`, `run/route`, `run/state`를 분리했으며, `control.rs`에서 `control/approve`, `control/cancel`, `control/helpers`, `control/helpers/approval`, `control/helpers/artifacts`, `control/helpers/state`, `control/helpers/events`, `control/helpers/time`, `control/resume`을 분리했고, `tests.rs`에서 `tests/run`, `tests/providers`, `tests/sentinel`, `tests/sentinel/commands`, `tests/sentinel/errors`, `tests/release`, `tests/recover`, `tests/control`을 분리했다. `packages/star-control-cli/tests/v0_fake_flow.rs`는 smoke scenario root만 남기고 fixture lifecycle/CLI runner/validation flow/report writer helper는 `tests/v0_fake_flow/support`, `tests/v0_fake_flow/fixture`, `tests/v0_fake_flow/cli`, `tests/v0_fake_flow/validation`, `tests/v0_fake_flow/validation/builders`, `tests/v0_fake_flow/validation/gate`, `tests/v0_fake_flow/validation/approval`, `tests/v0_fake_flow/report`로 분리했다.
- `packages/star-control-state/src/lib.rs`에서 `constants`, `error`, `error/display`, `error/source`, `types`, `paths`, `store`, `artifacts`, `events`, `outputs`, `recovery`, `recovery/inspection`, `recovery/issue`, `recovery/summary`, `recovery/tmp`, `tests`를 분리했고, `store.rs`에서 `store/lifecycle`, `store/jobs`, `store/core_artifacts`, `store/paths`를 분리했으며, `tests.rs`에서 `tests/artifacts`, `tests/artifacts/dirs`, `tests/artifacts/refs`, `tests/artifacts/safety`, `tests/artifacts/writers`, `tests/recovery`, `tests/store`, `tests/store/events`, `tests/store/jobs`, `tests/store/paths`를 분리했다.
- `packages/star-control-release/src/lib.rs`에서 `constants`, `error`, `consistency`, `profile`, `audits`, `audits/m9`, `audits/complete`, `writer`, `review_pack`, `review_pack/markdown`, `review_pack/storage`, `support`, `tests`, `test_support`를 분리했고, `tests.rs`에서 `tests/readiness`, `tests/review_pack`, `tests/consistency`, `tests/profile`, `tests/audits`, `tests/audits/m9`, `tests/audits/complete`, `tests/audits/helpers`를 분리했다.
- `packages/star-control-api/src/lib.rs`에서 `constants`, `request`, `error`, `paths`, `read_only`, `read_only/daemon`, `read_only/envelope`, `read_only/jobs`, `read_only/projects`, `read_only/release`, `read_only/reports`, `control`, `control/helpers`, `control/helpers/body`, `control/helpers/events`, `control/helpers/state`, `control/helpers/time`, `control/mutations`, `artifacts`, `tests`를 분리했고, `control/mutations.rs`에서 `mutations/approve`, `mutations/approve/request`, `mutations/approve/response`, `mutations/approve/event`, `mutations/cancel`, `mutations/resume`, `mutations/resume/artifacts`, `mutations/resume/event`을 분리했으며, `tests.rs`에서 `tests/read_only`, `tests/read_only/daemon`, `tests/read_only/errors`, `tests/read_only/helpers`, `tests/read_only/projects`, `tests/read_only/release`, `tests/read_only/reports`, `tests/control`을 분리했다.
- `packages/star-control-execution/src/lib.rs`에서 `constants`, `error`, `error/display`, `error/source`, `types`, `contract`, `state`, `engine`, `engine/provider`, `engine/request`, `engine/state`, `tests`, `test_support`를 분리했고, `tests.rs`에서 `tests/cloud`, `tests/cloud/cli`, `tests/cloud/api_offline`, `tests/cloud/api_live`, `tests/fake`, `tests/local_process`를 분리했으며, `test_support.rs`에서 `test_support/fixture`, `test_support/fixture/cloud`, `test_support/fixture/local_process`, `test_support/fixture/workspec`, `test_support/helpers`, `test_support/local_process`, `test_support/local_process/assertions`를 분리했다.
- `packages/star-control-validation/src/lib.rs`에서 `constants`, `error`, `types`, `engine`, `builders`, `artifacts`, `state`, `tests`를 분리했고, `engine.rs`에서 `engine/approval`, `engine/gate`, `engine/gate/approval`, `engine/gate/outcome`, `engine/provider`, `engine/writer`, `engine/writer/artifacts`, `engine/writer/events`, `engine/writer/run_state`를 분리했으며, `tests.rs`에서 `tests/approval`, `tests/gate`, `tests/provider`, `tests/helpers`, `tests/helpers/builders`, `tests/helpers/fixture`를 분리했다.
- `packages/star-control-schema/src/lib.rs`에서 `error`, `types`, `loader`, `validator`, `validator/compound`, `validator/path`, `validator/pattern`, `validator/scalar`, `tests`, `tests/helpers`, `tests/keywords`, `tests/loading`, `tests/structures`를 분리했다.
- `packages/star-control-observability/src/lib.rs`에서 `constants`, `error`, `audit`, `audit/io`, `audit/time`, `audit/validation`, `cost`, `cost/budget`, `cost/io`, `cost/paths`, `cost/validation`, `tests`, `tests/helpers`를 분리했다.
- `packages/star-control-security/src/lib.rs`에서 `constants`, `model`, `redact`, `report`, `tests`를 분리했다.
- `packages/star-control-cli/src/test_support.rs`에서 `test_support/project`, `test_support/local_process`, `test_support/sentinel`, `test_support/approval`, `test_support/release`, `test_support/recovery`를 분리했다.
- `packages/star-control-cli/src/tests/control.rs`에서 `tests/control/helpers`를 분리했다.
- `packages/star-control-schema/tests/canonical_examples.rs`에서 `canonical_examples/cases`, `cases/core`, `cases/provider_execution`, `cases/surface`, `cases/config`, `cases/sentinel`을 분리했다.
- `packages/star-control-cli/src/control/helpers.rs`에서 `helpers/approval`, `helpers/artifacts`, `helpers/state`, `helpers/events`, `helpers/time`을 분리했다.
- `packages/star-control-execution/src/tests/cloud.rs`에서 `tests/cloud/cli`, `tests/cloud/api_offline`, `tests/cloud/api_live`를 분리했다.
- 현재 즉시 진행할 refactoring/hardcoding slice는 닫힌 상태다. 다음 slice는 새 기능 branch가 추가되어 위 top 후보가 커질 때 책임 경계를 다시 산정한다.

## 하드코딩 기준

### 허용

아래 값은 계약값이므로 하드코딩할 수 있다. 단, inline string을 반복하지 말고 상수, enum, fixture builder 중 한 곳에 모은다.

- schema 파일명
- canonical artifact 상대 경로
- 공식 명칭과 ID: `Star Sentinel`, `star-sentinel`, `star_sentinel`, `star.sentinel`
- fake/test 계약값: `fake-default`, `provider.fake`
- schema enum 값: `DONE`, `FAILED`, `BLOCKED`, `WAITING_APPROVAL`
- CLI command 이름
- UI schema/view/action/transport 계약값
- test fixture의 의도적 sample 값
- 계약으로 고정된 `schema_version`

### 조건부 허용

아래 값은 중앙 상수, config default, policy file, enum 중 맞는 위치에 둔다. runtime 흐름 안에 임의 inline string으로 흩뿌리지 않는다.

- timeout, retry count, budget threshold
- default provider
- default config path
- validation command 목록
- CLI exit code
- allowed/forbidden action 이름
- reserved status/blocker 문구

### 금지

아래 값은 production code나 runtime default에 하드코딩하지 않는다.

- `D:\...`, `C:\Users\...`, `/Users/...`, `/home/...` 같은 절대 경로
- 사용자명, home directory, workspace 경로
- API key, token, password, credential raw value
- provider-specific endpoint, model, API version을 core runtime default로 박는 것
- 외부 repo, branch, PR 번호, CI run id
- 현재 날짜/시간을 고정값으로 사용하는 runtime logic
- local executable 절대 경로
- OS-specific shell command
- release version, changelog 내용
- 실제 target project 경로
- approval required action을 자동 허용하는 값

## 값의 위치 결정 규칙

| 값의 성격 | 위치 |
|---|---|
| schema/contract 고정값 | crate 상수 또는 schema/example |
| runtime 기본값 | `configs/defaults/` 또는 package-level default builder |
| policy 값 | `configs/policies/` |
| provider-specific 값 | `builtin-providers/**/provider.yaml` 또는 capability profile |
| test-only sample | test fixture 또는 `test_support` |
| user/environment 값 | CLI/API input, config, env reference |
| credential | `credential_ref`만 저장, raw value 저장 금지 |

## 리팩토링 slice 순서

1. 기준 문서 고정: 이 문서와 README/implementation README/PLANS 연결.
2. 구조 감사 문서화: workspace crate와 README-only scaffold inventory 갱신.
3. scaffold 격리: 승인된 경우 README-only scaffold를 `extensions/`로 이동.
4. module split 1차: `star-control-cli`, `star-sentinel`, `star-control-provider`부터 시작.
5. module split 2차: `star-control-state`, `star-control-release`, `star-control-api`, `star-control-execution`, `star-control-validation`.
6. 하드코딩 정리: 금지 후보를 production path에서 제거하고 test fixture는 `test_support`로 격리.
7. 공통 타입 추출: 반복되는 domain type, path guard, error mapping은 별도 `star-control-core` 후보로 정리하되 dependency 방향을 먼저 검토한다.

각 slice는 하나의 구조 목적만 가져야 한다. module split과 path rename, dependency 추가, schema field 변경, workflow 변경을 한 slice에 섞지 않는다.

## 승인 필요 항목

아래 작업은 별도 승인 전까지 실행하지 않는다.

- README-only scaffold 삭제
- `packages/` 전체를 `crates/`로 rename
- 새 dependency 또는 dependency version 변경
- Cargo 외 package manager 도입
- CI workflow permission 또는 job 삭제/약화
- provider live call, release/deploy/publish, destructive recovery
- GitHub repository settings 변경

## 검증 기준

문서-only slice:

```text
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
git diff --check
```

Rust module split slice:

```text
cargo fmt --check
cargo check --workspace
cargo test --workspace
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
git diff --check
```

Hardcoding scan:

```text
rg -n --glob 'packages/**/*.rs' --glob '!**/tests.rs' --glob '!**/tests/**' --glob '!**/test_support.rs' '(D:\\|C:\\Users|/Users/|/home/|\bsk-[A-Za-z0-9_-]{12,}|\bghp_[A-Za-z0-9_]{20,}|Authorization:\s*Bearer\s+[A-Za-z0-9._-]{8,}|(?i:\b(api[_-]?key|password|token)\b\s*[:=]\s*["''][A-Za-z0-9][^"'']{3,}["'']))'
rg -n --glob 'apps/**' --glob 'configs/**' --glob 'builtin-providers/**' --glob 'builtin-tools/**' --glob 'specs/**' --glob 'scripts/**' '(D:\\|C:\\Users|/Users/|/home/|\bsk-[A-Za-z0-9_-]{12,}|\bghp_[A-Za-z0-9_]{20,}|Authorization:\s*Bearer\s+[A-Za-z0-9._-]{8,}|(?i:\b(api[_-]?key|password|token)\b\s*[:=]\s*["''][A-Za-z0-9][^"'']{3,}["'']))'
rg -n '(D:\\|C:\\Users|/Users/|/home/)' --glob '!docs/implementation/refactoring-hardcoding-guidelines.md'
rg -n 'PR #[0-9]+|pull/[0-9]+|actions/runs/[0-9]+|2026-[0-9]{2}-[0-9]{2}' docs examples --glob '!docs/implementation/refactoring-hardcoding-guidelines.md'
```

`token`, `password`, `api_key` 같은 redaction/policy key 자체는 위반이 아니다. 위반 여부는 raw value, user/local path, provider-specific runtime default인지로 판정한다.

검증은 line-by-line로 반복하지 않는다. logical slice 완료 시 실행하고, `scripts/test.ps1`와 `python scripts/ci/run_all.py`는 같은 단계에서 중복 실행하지 않는다.
