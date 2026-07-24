# P-0056 최신 기능·복구 전수 감사 — 2026-07-24

## 판정 경계

이 문서는 과거 P-0054/P-0055 완료 문구를 현재 사실로 재사용하지 않고, `origin/main` `a93de7e68aff3ac02315d3a324aeaa497e1ede38`를 포함하는 현재 branch에서 1~11단계 Master Checklist와 실사용 전 복구 Slice를 source·Schema·repository·Controller·CLI/MCP·test 증거에 다시 대조한 결과를 소유한다. 2026-07-25 재감사에서는 M7~M11과 final ownership/status audit을 다시 펼쳐 확인했다.

감사 중인 source는 아직 최종 immutable commit이 아니므로 이 문서의 내부 구현 판정과 release/external seal을 분리한다. exact commit, package, lifecycle, SBOM, provenance와 provider readback은 source commit 뒤 새 증거로만 봉인한다. P-0055 `0d0eca9a` artifact와 hash는 historical evidence이며 P-0056 완료 근거가 아니다.

## 다시 발견한 실제 공백과 구현

| 공백 | 현재 구현 | fail-closed 경계 |
|---|---|---|
| Rule·Diagnostic·Baseline·Suppression·Disposition v2가 문서보다 얕았음 | typed v2 contract, deterministic fingerprint, v1 projection/migration, revisioned repository, runner와 query 경로 | provenance·binding이 부족한 history는 `stale|invalid`; current Gate 자동 승격 금지 |
| ChangePlan v2와 v1 migration이 목표 문구에 머묾 | PlanningBundle/ChangePlan v2, Schema·fixture, application/Controller/CLI plan·apply·rollback | legacy lineage가 불완전하면 blocked/replan; v1 in-place 재해석 금지 |
| Validator Guard negative corpus와 error surface가 열려 있었음 | sealed positive/negative/edge/regression corpus, manifest/hash 검증, 528개 정렬 closed `StableErrorCode` catalog와 generated enum Schema | unknown code 광고 금지, skipped/ignored matrix test 금지, validator 약화 시 block |
| 설정 key가 문서에만 있고 실제 실행 입력으로 materialize되지 않음 | `EffectiveConfigV1`, strict user/project TOML, persisted Goal layer, typed CLI/MCP Command override, User→Project→Goal→Command provenance·fingerprint | project/goal/command는 user-only key 금지 및 limit/permission/floor 확대 금지; invalid config에서 mutation 차단 |
| runtime 변경 가능한 policy와 fixed product invariant가 문서에서 섞여 있었음 | scan/index/cache/planning, approval TTL, retention, resource, external freshness, remote/clean-target, release/evaluation/Rust style의 override 허용 key만 실행 경로에서 소비하고 fixed/reserved key는 shipped value만 허용 | accepted-but-unmaterialized override와 unsafe widening은 startup/request 전에 거부 |
| portable recovery가 v1 decision만 보존 | `LocalStateBundle` v2에 v1 decision/active ChangePlan과 `SuppressionV2`·`BaselineV2`·`DispositionV2`를 정렬·봉인, v1 reader 호환, import CAS/current Finding·evidence 검증 | 다른 Project/source/config, duplicate ID, skipped revision, stale target, tampered payload는 전체 transaction 거부 |
| M7 complete reproduction과 `verified_fixed`가 caller payload만으로 승격 가능 | `ReproductionAttemptObservationV1`, `process.failure.reproduce` terminal effect receipt, semantic result fingerprint, current ValidationResult/Run·compatible `not_reproduced` pack·verified recurrence 결속 | missing/reused/mismatched receipt는 pack publish 거부, arbitrary fixed/recurring은 `REGRESSION_EVIDENCE_MISMATCH` |
| M8 일부 phase와 pass/verified/equivalent report가 실제 effect·M3 evidence를 다시 읽지 않음 | dry-run/backup/rehearse/execute/resume/rollback 전체 receipt 의무화, migration validation·restore verification·language equivalence의 current ValidationResult/AUTO_PASS Gate/artifact fingerprint 재검증 | partial/outcome_unknown/stale config·다른 plan/checkout·임의 ref는 success 상태로 승격 금지 |
| M9 handoff·merge/remote lifecycle이 caller의 positive 상태를 받을 수 있었음 | actual goal/participant/plan, local merge result, remote before/effect/after lifecycle와 release handoff를 current source·Gate·receipt에 결속 | direct success/verified/completed 입력, partial/outcome_unknown 누락과 다른 participant evidence 거부 |
| M10 EvaluationRun이 post-result case/sample과 caller count에 의존 | Project Git의 `EvaluationCaseDefinitionV1`·`EvaluationPolicyV1`, exact case set/sample/attempt/ground truth, current ValidationRun→Gate→DiagnosticEvaluation 기반 Finding/Suppression metric | source drift, zero-diagnostic positive miss, new/broadened suppression, new/worsened finding, missing current evidence는 accept 금지 |
| M10 verified cost·budget과 Radar relation이 구현되지 않음 | provider evidence·ValidationRun attribution을 가진 `CostRecordV1`, derived `BudgetSnapshotV1`, unit/currency microunit baseline/candidate metric, Radar의 exact EvaluationRun 역참조 | estimate·unknown=0·cross-cohort attribution·caller Radar success ref 거부 |
| final ownership/status audit이 문서 표뿐이었음 | `FinalProductAuditV1`과 `star release audit`: current direct/management handler 23개, exact 16 Profile resolution, M11 closure, ReleaseManifest/artifact-set/lifecycle evidence 재검증 | 내부 23/16 conformance와 external signing/provider를 분리, ARM64 Preview simulation은 `native_unverified` 유지 |
| M11 style 상태를 fixed toolchain/isolated Patch/Gate에서 우회할 위험 | pinned Rust 1.96, owned isolated preview, rustfmt/allowlisted Clippy, candidate checks, M2 Profile·M4 PatchSet·M3 pre/post Gate·receipt/recovery 재검증 | caller-supplied pass, live mutator, incomplete coverage, tool/config drift와 reused permit 거부 |
| 정본 문서가 과거 “미구현” 상태를 current로 표시 | README·문서 index·개발관리·설정·오류·migration·state·Profile·roadmap을 live code 기준으로 동기화 | historical P-ID 문서는 historical banner를 유지하고 새 source 증거로 승격하지 않음 |

## EffectiveConfig 실제 경로

`EffectiveConfigV1`은 모든 entry를 typed value와 merge strategy로 정렬하고 각 source kind/id/fingerprint를 provenance로 보존한다. user config는 `%APPDATA%\Star-Control\config.toml`, project config는 `<project>/.star-control/config.toml`에서 1 MiB 이하 UTF-8 fixed-local non-reparse file만 읽는다. Goal config는 Goal store에 durable하게 저장되고, CLI/MCP command override는 128개·canonical 64 KiB 한도 안에서 IPC envelope로 전달된다. backup 없는 source rebuild도 선택한 checkout root에서 User→Project→현재 Goal/Command layer를 다시 resolve하고 plan과 candidate ScanRun에 같은 exact config fingerprint를 요구한다.

lower layer는 다음을 할 수 없다.

- `controller.auto_start`, personal remote auto scope, project discovery root, tool registry trust/location 같은 user-only key 변경
- `minimum_limit`을 늘리거나 `maximum_floor`를 낮추기
- `false_wins`, `true_wins`, `most_restrictive`, `explicit_widening` 의미를 반대로 넓히기
- unrestricted `scan.include_paths`를 빈 intersection으로 우회하거나 `scan.include_untracked`를 다시 켜기
- project config에서 `policy_profile`을 선택하기. project는 더 강한 `required_policy_profile`만 요구 가능

`planning.create`에 명시적 Profile이 없으면 exact 16개 built-in ID 중 검증된 `default_work_profile` 하나를 사용한다. invalid effective config에서는 status·diagnostic·control/cancel 같은 제한된 read/control 명령만 허용하고 project/scan/patch/development effect를 시작하지 않는다.

## 복구 Slice 실제 보존 경계

normal backup/restore는 global store와 관련 project store를 동일 generation set으로 보존하므로 TaskSpec·ScopeRevision·ImpactAnalysis·PlanningBundle v2, v1/v2 local decision, Finding와 검증된 ArtifactRef projection을 함께 복원한다. restore는 immutable side-by-side candidate를 read-only 검증한 뒤 top-level active-set만 atomic replace하며 손상·이전 generation을 덮어쓰거나 삭제하지 않는다.

portable `LocalStateBundle` v2는 project-scoped v1 Suppression·Baseline·Disposition, nonterminal v1 ChangePlan과 v2 Suppression·Baseline·Disposition만 포함한다. global coordinator가 소유하는 PlanningBundle v2는 project bundle import만으로 두 store를 원자 갱신할 수 없어 의도적으로 제외한다. backup 없는 source rebuild는 source-derived projection과 verified ArtifactRef만 재생성하고 PlanningBundle/active ChangePlan, local decision, idempotency·actor history 손실을 typed loss report로 반환한다. 같은 Project에 checkout이 하나면 그것을 사용하고, 여러 checkout이면 실제 Git observation에서 유일한 `main_worktree`만 선택한다. clone만 여러 개이거나 main이 모호하면 임의 root를 고르지 않고 `RevisionConflict`로 중단한다.

disposable E2E는 project 등록→scan/Finding→v1/v2 local decision→patch/Validation/Gate→backup→corrupt/recovery-only→verified restore→source rebuild, future version, missing/hash-mismatched set, activation crash points, second writer와 redaction을 임시 root에서 검증한다. 실제 Git main+linked worktree를 한 Project로 등록한 뒤 unique main 선택부터 rebuild apply까지도 검증한다. 실제 사용자 management root와 실제 project에는 손상·restore를 수행하지 않는다.

## 현재 로컬 재검증

| 명령 | 결과 |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo run --locked -p star-schema-gen -- --check` | PASS, generated manifest 202 files |
| `cargo test -p star-contracts --locked -- --nocapture` | PASS, unit 26/26, contracts 30/30, evidence 7/7, property 2/2 |
| `cargo test -p star-release --locked -- --nocapture` | PASS, 25/25; case/policy/cost/budget/finding/suppression/audit tamper 포함 |
| `cargo check -p star-release -p star-controller -p star-cli --locked` | PASS |
| `cargo test -p star-cli m10_release_and_evaluation_commands_emit_typed_controller_requests --locked` | PASS |
| `cargo test -p star-controller m10_release_and_evaluation_commands_are_controller_owned --locked` | PASS; 23 feature handler registry check 포함 |
| `cargo clippy -p star-release -p star-controller -p star-cli -p star-state -p star-application --all-targets --locked -- -D warnings` | PASS |
| `cargo test -p star-development complete_reproduction_attempt_requires_a_receipt_bound_semantic_observation --locked` | PASS |
| `cargo test -p star-controller development_effect_receipt_binds_terminal_operation_subject_and_executable --locked -- --nocapture` | PASS, reproduction semantic receipt E2E 포함 |
| `cargo test -p star-controller every_effectful_migration_phase_requires_a_terminal_effect_receipt --locked -- --nocapture` | PASS |
| `cargo test --workspace --locked` | PASS, 종료 코드 0, 158.4초 |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | PASS, 종료 코드 0; `-D warnings` 위반 0 |
| `cargo run --locked -p star-matrix-check -- --details` | PASS, `expected=170`, `mapped=170`, `missing=[]` |
| `pwsh ./scripts/validate.ps1 -Profile target` | PASS, `requested=target`, `required=full`, `effective=full`, 10/10, 141.2초; `target/validation/20260724T204834885Z-22604/report.json` |

Windows가 `target/debug/incremental` finalize에서 내는 access-denied notice는 cache 재사용만 잃는 nonfatal 경고다. `target/`을 삭제하거나 검증 결과를 숨기지 않았다.

## source seal 뒤 닫을 Gate

다음 항목은 exact source commit을 만든 뒤에만 실행·기록한다.

1. exact commit의 RELEASE 결과. 전체 workspace test/clippy/matrix와 pre-seal TARGET effective FULL은 PASS
2. x64 release package와 disposable install/finalize/Bridge/status/rollback lifecycle
3. Rust 1.96 `aarch64-pc-windows-msvc` cross-build, PE `0xaa64`, cfg/target simulation과 Rust style corpus; 결과는 `native_unverified`
4. unsigned installer model, SPDX SBOM, RustSec audit와 pre-sign provenance
5. authenticated GitHub disposable draft upload/download digest readback과 cleanup
6. branch push와 remote exact commit/tree readback

현재 설치된 P-0055 runtime은 registry revision 7에서 release 17/17 action이 `ready`임을 다시 읽었다. 새 P-0056 byte를 current host에 설치하는 것은 disposable exact lifecycle과 별개다. fixed executable 교체 때문에 실제 restart가 필요하지 않은 한 Desktop을 재시작하지 않는다. Project discovery의 correctness는 watcher에 의존하지 않고 manual/full 또는 외부에서 호출한 incremental CLI가 소유하며 자체 scheduler를 추가하지 않는다.

## 남는 외부 경계

사용자 결정에 따라 ARM64 native 장비 실행은 cross-build·PE·simulation으로 대체하고 `native_unverified`를 유지한다. Authenticode certificate/private key/trusted timestamp가 없으므로 signed Runtime/installer, signed clean lifecycle, final signed provenance와 public Stable publish/readback은 `blocked_external`이다. unsigned Stable로 낮추거나 과거 서명 전 artifact를 공개하지 않는다.
