# PLANS.md

## 목적

현재 작업 상태를 짧게 유지하는 원장이다. 상세 로그, 전체 diff, 반복 검증 출력은 여기에 누적하지 않는다. 장기 보존이 필요한 근거는 `docs/decisions/*`, report, changelog, commit history에 둔다.

## Context Pack

### 현재 목표

- Star-Control repository는 현재 스캐폴드와 정본 설계 문서 상태다.
- v0 runtime 구현 스택은 Rust + Cargo workspace로 결정했다.
- v0 fake provider instance id는 `fake-default`로 결정했다.
- v0 Star Sentinel P0 rule set은 5개 핵심 rule로 결정했다.
- 완전 구현 기본값은 `docs/decisions/0005-full-implementation-defaults.md`를 기준으로 한다.
- 전체 milestone은 `docs/implementation/complete-implementation-roadmap.md`를 기준으로 한다.
- Codex 구현 착수용 EPIC brief는 `docs/implementation/briefs/`를 기준으로 한다.
- `PLANS.md`는 현재 상태만 남기는 bounded snapshot으로 유지한다.

### 반드시 지켜야 할 제약

- `docs/decisions/0005-full-implementation-defaults.md`의 기본 Rust dependency set 밖의 의존성 추가, Cargo 외 package manager 도입, 원격 공개 작업은 명시 요청이 있을 때만 한다.
- 실행 결과는 Star-Control repo가 아니라 대상 프로젝트 `.ai-runs/`에 둔다.
- 외부 보조 자료를 다시 붙이지 않고 이 repository 안의 정본 파일을 기준으로 작업한다.

### 이미 끝난 것

- Star-Control monorepo 스캐폴드, schema, contract, config, registry, provider/tool manifest를 정리했다.
- Star Sentinel 명칭, policy, schema, template, output contract를 정리했다.
- `PLANS.md`와 plan-ledger 운영 기준을 bounded snapshot 방식으로 정리했다.
- v0 runtime stack을 Rust + Cargo workspace로 결정했다.
- v0 fake provider instance id를 `fake-default`로 통일했다.
- Star Sentinel v0 P0 scope와 E09a~E09d 구현 분할을 정리했다.
- 로컬 contract check entrypoint를 `python scripts/ci/run_all.py`로 추가했다.
- E01~E11 Codex 구현 착수용 brief를 추가했다.
- 완전 구현 기본값과 M0~M9 milestone 기준을 문서화했다.
- 검증/AGENTS 효율 병목을 정리하고, `scripts/test.ps1`을 정본 contract runner로 연결했다.
- E01 최소 Cargo workspace와 `packages/star-control-schema` runtime validator를 추가했다.
- E02 `packages/star-control-state` file-based StateStore를 추가했다.
- E03 Artifact Layout Writer helper를 StateStore에 추가했다.
- E04 `packages/star-control-provider` Provider Registry를 추가했다.
- E05 `FakeProviderAdapter`와 provider output writer 연결을 추가했다.
- E06 `packages/star-control-router` RouterEngine을 추가했다.
- E07 `packages/star-control-execution` ExecutionEngine을 추가했다.
- E08 `packages/star-control-cli` CLI fake flow를 추가했다.
- E09a `packages/star-sentinel` P0 evaluator를 추가했다.
- E09b Star Sentinel diagnostics/gate artifact writer를 추가했다.
- E09c Star Sentinel review-pack writer를 추가했다.
- E09d Star Sentinel ledger writer와 selfcheck를 추가했다.
- E10 `packages/star-control-validation` ValidationEngine을 추가했다.
- E11 `packages/star-control-cli/tests/v0_fake_flow.rs` integration smoke를 추가했다.
- M5 local process provider command/sandbox/timeout/cancel 정책 문서를 추가했다.

### 아직 남은 것

- provider host, transport, adapter, Star Sentinel runtime 구현은 E01~E11 이후 milestone 순서에 맞춰 진행한다.
- v0 fake flow는 E11 integration smoke로 첫 검증 milestone에 도달했지만, 완전 구현의 끝점은 아니다.
- 다음 구현 축은 complete roadmap의 M5 local provider, M6 cloud provider, M7 daemon/API, M8 UI, M9 hardening 순서다.

### 건드리면 안 되는 것

- 사용자 승인 없는 의존성 설치, 파일 삭제, 테스트 약화.
- schema, manifest, registry의 공개 필드명은 변경 전 영향 범위를 확인한다.
- fake flow 안정화 전 local/cloud provider, daemon, API, UI, release automation을 앞당기지 않는다.

### 먼저 확인할 파일

- `README.md`
- `docs/implementation/README.md`
- `docs/decisions/0005-full-implementation-defaults.md`
- `docs/implementation/complete-implementation-roadmap.md`
- `docs/implementation/codex-long-run-workflow.md`
- `docs/implementation/codex-work-queue-current.md`
- `docs/implementation/briefs/README.md`
- 해당 EPIC의 `docs/implementation/briefs/E*.md`

### 먼저 실행할 명령

```text
python scripts/ci/run_all.py
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

### 현재 차단 요소

- 없음.

## 현재 활성 작업

| ID | 상태 | 목표 | 주요 파일 | 다음 조치 |
|---|---|---|---|---|

## 열린 리스크

| ID | 내용 | 영향 | 다음 조치 |
|---|---|---|---|
| R-0001 | v0 fake smoke의 final report/DONE 전이는 integration test harness에서 확인됨 | production CLI의 validate/approve/final report orchestration은 아직 별도 명령으로 노출되지 않음 | M5 이후 provider 확장 전 CLI surface backlog로 정리 |
| R-0002 | runtime validator pattern 지원 범위 제한 | 현재 repository schema pattern만 지원하고 범용 regex는 지원하지 않음 | 새 pattern 추가 시 schema-validator test와 dependency 승인 여부 검토 |
| R-0003 | StateStore 초기 단일 process 기준 | daemon 동시 실행 lock은 아직 없음 | daemon milestone에서 lock policy 추가 |

## Archive References

| 항목 | 위치 |
|---|---|
| 정본 구조 결정 | `docs/decisions/0001-canonical-repository.md` |
| runtime stack 결정 | `docs/decisions/0002-runtime-stack.md` |
| fake provider instance 결정 | `docs/decisions/0003-fake-provider-instance.md` |
| Star Sentinel P0 scope 결정 | `docs/decisions/0004-star-sentinel-p0-scope.md` |
| 완전 구현 기본값 결정 | `docs/decisions/0005-full-implementation-defaults.md` |
| 완전 구현 milestone | `docs/implementation/complete-implementation-roadmap.md` |
| EPIC별 brief | `docs/implementation/briefs/` |
| E01 dependency record | direct dependency `serde_json = "1"`; 목적: JSON schema/document parse; 대안: std-only JSON parser 재구현은 안정성 낮음; 검증: Cargo + contract checks |
| E02 handoff | `StateStore::open`, `create_job`, `save_state`, `append_event`, `resolve_job_path`, provider/tool output dir helper; recovery는 자동 복구 없이 명확한 오류/`list_jobs` corrupt 표시 |
| E03 handoff | provider/tool output writer, approval/review/tmp writer, `ArtifactKind`, `artifact_ref`, `register_artifact_ref`; writer는 existing artifact overwrite를 거부 |
| E04 handoff | `ProviderRegistryLoader::load_registry`, `load_fake_default_registry`, `ProviderRegistry::instance`, `manifest_for_instance`, `capability_for_instance`; fake fixture는 `examples/provider-contracts/provider-instance.fake.example.json`과 `configs/provider-instances/fake-provider.example.yaml`의 `fake-default` |
| E05 handoff | `ProviderAdapter` trait, `ProviderRunContext`, `ExecutionRequest`, `ProviderRunResult`, `FakeProviderAdapter::{success,failed,blocked}`; output shape는 `provider-output/{provider_instance_id}/{request.json,response.json,stdout.txt,stderr.txt}`이며 기존 artifact overwrite는 오류 |
| E06 handoff | `RouterEngine::route`, `JobSpec`, `RouterOutput::{route,decision,workspecs}`; WorkSpec path는 `workspecs/{stage}.json`, 초기 assignment는 enabled `fake-default`와 `policy_profile`을 stage별로 기록 |
| E07 handoff | `ExecutionEngine::execute_stage(job_id, stage)`; precondition은 StateStore에 `job.json`과 `workspecs/{stage}.json`이 있고 registry에 `provider_instance`가 존재하는 것; output은 `provider-output/{provider_instance}/request.json`, `response.json`, `stdout.txt`, optional `stderr.txt`, RunState update, provider start/finish events |
| E08 handoff | `star-control run --project <path> --request <text> --provider fake-default --json`, `status --project <path> --job <job-id> --json`, `report --project <path> --job <job-id> --stage <stage> --json`; command는 schema/config root로 current directory 또는 `STAR_CONTROL_HOME`을 사용하며, fake run은 target project `.ai-runs/`에 job, route, workspec, provider output, report, run-state를 기록 |
| E09a handoff | `read_task`, `read_changed_lines`, `read_p0_rule_registry`, `P0Evaluator::evaluate`; 입력은 `SentinelTask`, `ChangedLines`, `P0RuleRegistry`, 출력은 in-memory `EvaluationResult { decision, diagnostics }`; P0 rule은 scope/test deletion/dependency approval/plaintext secret/validator self-bypass 5개이며 writer, gate file, review pack, ledger/selfcheck는 E09b~E09d 범위 |
| E09b handoff | `build_diagnostics_artifact`, `build_approval_artifact`, `validate_diagnostics_artifact`, `validate_approval_artifact`, `write_gate_artifacts`; output path는 `tool-output/star-sentinel/diagnostics.json`와 `tool-output/star-sentinel/approval.json`; approval request artifact가 아니라 Star Sentinel gate decision만 쓴다 |
| E09c handoff | `build_review_pack_artifact`, `validate_review_pack_artifact`, `write_review_pack_artifacts`; 원본은 `tool-output/star-sentinel/review_pack.json`, `tool-output/star-sentinel/review_pack.md`, 사용자용 copy는 `review-packs/review_pack.json`, `review-packs/review_pack.md`; StateStore에 `write_tool_text`를 추가했다 |
| E09d handoff | `build_gate_ledger_event`, `validate_ledger_events`, `write_ledger_artifact`, `run_selfcheck`; ledger output은 `tool-output/star-sentinel/ledger.jsonl`; selfcheck는 manifest outputs, P0 registry/schema/fixture parse, rule id duplicate, legacy alias 위치를 확인한다 |
| E10 handoff | `ValidationEngine::evaluate_star_sentinel_gate`, `write_outcome`, `ensure_provider_response`, `ensure_approval_response_allows_next_stage`; Star Sentinel `approval.json`을 `validation/validation-decision.json`으로 정규화하고 `tool-output/star-sentinel/validation_runs.json`, `approvals/approval-request.json`, `review-packs/handoff.json`, RunState 전이를 기록한다 |
| E11 handoff | `packages/star-control-cli/tests/v0_fake_flow.rs`; CLI `run`으로 fake provider output을 만든 뒤 Star Sentinel P0 evaluator/gate/review-pack writer와 ValidationEngine을 연결해 AUTO_PASS -> DONE smoke, HUMAN_REVIEW -> WAITING_APPROVAL -> approved -> DONE smoke, BLOCK -> BLOCKED smoke를 검증한다 |
| M5a handoff | `docs/implementation/local-process-provider-policy.md`; local process provider는 shell 없이 executable/args vector만 실행하고, env allowlist, network deny, workspace write deny, timeout/cancel 기록, approval-required/forbidden action guard를 따른다 |
| 이전 완료 이력 | git history |

## 완료 작업

| ID | 완료일 | 한 줄 요약 | 근거 |
|---|---|---|---|
| P-0001 | 2026-06-28 | Star-Control monorepo 스캐폴드와 정본 설계 문서 생성 | `7ccdce5` |
| P-0002 | 2026-06-28 | provider, schema, Star Sentinel 설계 보강 | `c321f11` |
| P-0003 | 2026-06-28 | `PLANS.md`와 plan-ledger 운영을 bounded snapshot 기준으로 압축 | git history |
| P-0004 | 2026-07-01 | v0 runtime stack을 Rust + Cargo workspace로 결정 | `docs/decisions/0002-runtime-stack.md` |
| P-0005 | 2026-07-01 | v0 fake provider instance id를 `fake-default`로 통일 | `docs/decisions/0003-fake-provider-instance.md` |
| P-0006 | 2026-07-01 | Star Sentinel v0 P0 scope와 E09 분할 기준 정리 | `docs/decisions/0004-star-sentinel-p0-scope.md` |
| P-0007 | 2026-07-01 | 로컬 contract check runner 추가 | `scripts/ci/run_all.py` |
| P-0008 | 2026-07-01 | E01~E11 Codex 구현 착수용 brief 추가 | `docs/implementation/briefs/` |
| P-0009 | 2026-07-01 | 완전 구현 기본값과 M0~M9 milestone 문서 정렬 | `docs/decisions/0005-full-implementation-defaults.md`, `docs/implementation/complete-implementation-roadmap.md` |
| P-0010 | 2026-07-01 | 검증/AGENTS 효율 병목 정리 | `AGENTS.md`, `scripts/test.ps1`, `scripts/ci/check_implementation_docs.py` |
| P-0011 | 2026-07-01 | E01 runtime schema validator 추가 | `packages/star-control-schema`, `Cargo.toml` |
| P-0012 | 2026-07-01 | E02 file-based StateStore 추가 | `packages/star-control-state` |
| P-0013 | 2026-07-01 | E03 artifact layout writer helper 추가 | `packages/star-control-state` |
| P-0014 | 2026-07-01 | E04 provider registry와 fake-default 조회 API 추가 | `packages/star-control-provider` |
| P-0015 | 2026-07-01 | E05 deterministic FakeProviderAdapter 추가 | `packages/star-control-provider` |
| P-0016 | 2026-07-01 | E06 deterministic RouterEngine 추가 | `packages/star-control-router` |
| P-0017 | 2026-07-01 | E07 fake provider ExecutionEngine 추가 | `packages/star-control-execution` |
| P-0018 | 2026-07-01 | E08 CLI read-only + fake run 추가 | `packages/star-control-cli` |
| P-0019 | 2026-07-01 | E09a Star Sentinel P0 evaluator 추가 | `packages/star-sentinel` |
| P-0020 | 2026-07-01 | E09b Star Sentinel diagnostics/gate writer 추가 | `packages/star-sentinel` |
| P-0021 | 2026-07-01 | E09c Star Sentinel review-pack writer 추가 | `packages/star-sentinel`, `packages/star-control-state` |
| P-0022 | 2026-07-01 | E09d Star Sentinel ledger/selfcheck 추가 | `packages/star-sentinel`, `builtin-tools/star-sentinel/tool.yaml` |
| P-0023 | 2026-07-01 | E10 ValidationEngine 추가 | `packages/star-control-validation`, `packages/star-control-state` |
| P-0024 | 2026-07-01 | E11 v0 fake integration smoke 추가 | `packages/star-control-cli/tests/v0_fake_flow.rs` |
| P-0025 | 2026-07-01 | M5 local process provider policy 추가 | `docs/implementation/local-process-provider-policy.md`, `configs/policies/provider-policy.yaml` |
