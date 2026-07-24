# P-0056 최신 기능·복구 전수 감사 — 2026-07-24

## 판정 경계

이 문서는 과거 P-0054/P-0055 완료 문구를 현재 사실로 재사용하지 않고, `origin/main` `a93de7e68aff3ac02315d3a324aeaa497e1ede38`를 포함하는 현재 branch에서 1~11단계 Master Checklist와 실사용 전 복구 Slice를 source·Schema·repository·Controller·CLI/MCP·test 증거에 다시 대조한 결과를 소유한다. 2026-07-25 재감사에서는 M7~M11과 final ownership/status audit을 다시 펼쳐 확인했다.

artifact source는 commit `1bce4724c34414cef74862dbe9bf9de1f094ad2f`, tree `4e1c3b1d55bfbe35eb7eaf4455c02bde711bcac4`로 고정했다. 이 source에서 package·lifecycle·SBOM·RustSec·pre-sign provenance와 provider readback을 새로 생성했으며, P-0055 `0d0eca9a` artifact와 hash는 historical evidence일 뿐 P-0056 근거로 재사용하지 않았다. 후속 정본-only seal commit은 이 artifact source revision과 byte set을 바꾸지 않는다.

## 최종 판정

- **기능 전수조사·복구 Slice:** `DONE`. 최신 `origin/main`을 포함한 exact source에서 발견한 공백을 실제 contract·repository·Controller·CLI/MCP·Schema·test 경로로 닫았고, 23개 기능·16 Profile final audit도 current evidence를 재검증한다.
- **서명 제외 외부 봉인:** `DONE / unpublished`. RELEASE의 유일한 미검증 항목은 승인 범위 밖 서명·공개이며, x64 격리 lifecycle, ARM64 교차 simulation, installer model, SBOM·RustSec·provenance, GitHub draft byte 왕복·cleanup과 원격 source readback은 완료했다.
- **현재 설치 상태:** `preserved`. P-0056 candidate는 격리 root에서만 finalize했고 실제 사용자 installation/data와 현재 P-0055 runtime을 건드리지 않았으며 Desktop을 재시작하지 않았다.
- **공개 Stable:** `blocked_external`. Authenticode·trusted timestamp와 signed-byte lifecycle·publication 증거가 없으므로 unsigned Stable 승격은 차단된다.

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

## exact source 로컬 재검증

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
| `pwsh ./scripts/validate.ps1 -Profile release` | 비서명 범위 14/15 PASS, failed 0, signing/publication만 unverified 1; completeness가 미완료라 종료 코드 1을 그대로 보존; `target/validation/20260724T213821223Z-33572/report.json` |

Windows가 `target/debug/incremental` finalize에서 내는 access-denied notice는 cache 재사용만 잃는 nonfatal 경고다. `target/`을 삭제하거나 검증 결과를 숨기지 않았다.

## exact 비서명 외부 봉인 증거

증거 root는 `dist/release-evidence/p0056-1bce4724`다. `dist/`와 `target/`은 생성 증거이므로 source처럼 직접 수정하거나 commit하지 않았고, 아래 hash는 정본 갱신 직전에 실제 파일 11개와 다시 대조했다.

| 묶음 | 판정 | exact 증거 |
|---|---|---|
| source delivery | PASS | origin branch가 commit `1bce4724c34414cef74862dbe9bf9de1f094ad2f`, tree `4e1c3b1d55bfbe35eb7eaf4455c02bde711bcac4`를 readback하고 `origin/main` `a93de7e68aff3ac02315d3a324aeaa497e1ede38`를 포함 |
| TARGET / RELEASE | PASS / signing completeness `unverified` | TARGET 10/10 `sha256:d342b08ef5d7babdd8c0572efb58087e2d35c7af9ec4430e36dae94db79c37cc`; RELEASE 14/15, failed 0, signing/publication만 unverified 1, `sha256:4ea2c35daf5f5383596cc38c436deb0ea02bc707f51ebb6fd5d20612db0b3608` |
| x64 package | PASS | manifest entry 503개, set `sha256:97f9a066adec36ecc321ddb5f3060c3b7e34aeaa088228554be1e68f40566e6f`, manifest `sha256:c5b9ddc763aa88a50d8ab2e745cff630f7b7751a67ccb8a4f3a319613d216935`, Runtime generation `rt_4b2ac59f03322d5d`, nested manifest `sha256:346fbc179922f1ca1f49ecb03cac3fd3156b553917c7efbd66cc8f6b2924120c`, PE `0x8664` |
| ARM64 package·simulation | PASS / `native_unverified` | Rust 1.96 `aarch64-pc-windows-msvc` cross-build와 corpus check·Clippy, manifest entry 503개, set `sha256:5e4a9ffa647939a7c967842d04896bd32894da3e8c597e51db4a3a8ce2c86fdc`, manifest `sha256:d245ee43275e3df6fa09e276a6018568ee39ef228ee7a24c0a22a8eef8e6c611`, generation `rt_683a544874d87204`, PE `0xaa64` |
| installer model | PASS / public 불가 | Inno Setup 6.7.3 x64 26,085,112 bytes `sha256:4e196ccae2225c2d5ed479b6dc76654dcd1db8510500568552f2d73049b200d4`; ARM64 21,723,245 bytes `sha256:ac6f9444edb7a5799c7e024a3cce038c3454cc9f0d61928a193ae85e447a99f1`; 둘 다 `NotSigned` |
| SBOM·RustSec | PASS | SPDX SBOM 각 7 packages: x64 `sha256:b78afc44e3645e0d2a2c18bc423adea24369c667300afe17e9fc11db3da00cab`, ARM64 `sha256:14d18be74399db49f1048879b43e48f612c5aad87ad2691024c25b320dfbedf0`; RustSec DB 1,169 advisories·223 dependencies·vulnerability 0·warning 0, `sha256:9724fb84dd7e94b570b19784b3b5a9b90f948f04d88d119ee61bcae038725390` |
| x64 격리 lifecycle·rollback | PASS | isolated finalize→Bridge v2 initialize→installation/update status, installation `ins_01KYB1TEVGZ1H9MSH4FBWF9660`, activation revision 1, generation `rt_4b2ac59f03322d5d`; lifecycle evidence `sha256:58b3954d03c4dfa32f5f76d4282223aa5a2cad1e07efa01af61311aa9448c473`, failed-update rollback tests PASS, user installation untouched, restart 없음 |
| GitHub draft byte 왕복·cleanup | PASS / unpublished | disposable draft release `359566726`, asset `488863823`; local/provider/download가 모두 `sha256:d3f6aed24673f5dba55043d79196987eb51b98c1366563e43e3b06d9123fcf77`; cleanup 뒤 release ID/tag와 Git tag ref 모두 absent, evidence `sha256:53ca2369967078bdc0034766bfc6c9a05fd3eab29f461ac19355b87583340484` |
| pre-sign provenance | PASS / public 불가 | `provenance.pre-sign.json`, `sha256:3bde861329cff0cb8f6a8bbae12a1e40391275e7614f3eedbbd67442ff97d226`; source·toolchain·validation·artifact·security·lifecycle·remote readback과 서명 후 재생성 의무를 결속 |

현재 설치된 P-0055 runtime은 registry revision 7에서 release 17/17 action이 `ready`인 상태를 보존했다. 새 P-0056 byte를 current host에 maintenance install하는 것은 격리 lifecycle과 별개이며 비서명 봉인 완료 조건이 아니다. Project discovery의 correctness는 watcher에 의존하지 않고 manual/full 또는 외부에서 호출한 incremental CLI가 소유하며 자체 scheduler를 추가하지 않는다.

`dist/release-evidence/p0056-caf02807-tree`는 commit 전 source revision이 final index/commit과 일치하지 않은 비채택 증거다. 그 directory의 artifact와 digest는 P-0056 봉인에 사용하지 않았다.

## 완료 조건 대조

- 최신 main 기준 기능·복구 전수조사와 발견 공백의 제품 구현·Schema·정본 반영: **PASS**.
- exact source workspace test, all-feature Clippy, 170/170 matrix와 TARGET effective FULL: **PASS**.
- RELEASE failed 0, 서명·공개 외 비서명 항목: **PASS**. 유일한 unverified를 pass로 승격하지 않았다.
- x64 package·격리 lifecycle·failed-update rollback과 실제 사용자 data 보존: **PASS**.
- ARM64 cross-build·PE·cfg/target simulation과 `native_unverified` 경계: **PASS**.
- installer model, SBOM, RustSec, exact pre-sign provenance: **PASS**.
- authenticated GitHub draft upload/download digest readback·cleanup과 remote source commit/tree readback: **PASS**. draft는 publish하지 않았다.
- 불필요한 Desktop restart 없음, 현재 P-0055 installation 유지, unsigned Stable fail-closed: **PASS**.

## 남는 외부 경계

서명 제외 제품·복구 Slice와 외부 실행 경로에는 열린 blocker가 없다. 사용자 결정에 따라 ARM64 native 장비 실행은 cross-build·PE·simulation으로 대체했고 `native_unverified`를 유지한다.

남은 blocker는 서명·공개 한 층이다. Authenticode certificate/private key/trusted timestamp가 없으므로 signed Runtime/installer, signed clean lifecycle, final signed provenance와 public Stable publish/readback은 `blocked_external`이다. signing으로 byte가 바뀌면 새 candidate로 보고 package identity·SBOM·provenance·lifecycle·publish/readback을 다시 실행하며, unsigned Stable이나 과거 pre-sign artifact를 공개하지 않는다.
