# PLANS.md

## 목적

이 문서는 현재 판단과 다음 실행만 보존하는 bounded snapshot이다. 세부 계약은 [문서 읽는 순서](docs/README.md), 구현 순서는 [최종 구현 로드맵](docs/roadmap/final-implementation.md), 실행 증거는 `docs/testing/`과 생성 validation artifact가 소유한다. historical `DONE`은 해당 revision의 seal이며 현재 source 완료 증거로 자동 승격하지 않는다.

## 확정 불변식

- source·manifest·Catalog가 canonical이고 DB/index/cache는 derived state다. 신규 scanner별 DB나 completion model을 만들지 않는다.
- `partial|stale|unsupported|unverified|not_run|flaky|outcome_unknown`을 pass로 바꾸지 않는다.
- Finding은 source를 직접 바꾸지 않으며 `ChangeRecipeV2 → isolated PatchSetV2 → pre/post Gate → exact approval` 경로만 사용한다.
- external analyzer·LSP·OpenRewrite·mutation engine·Scorecard는 registered adapter다. 설치되지 않은 provider는 설치하지 않고 `unavailable|unverified`로 보존한다.
- `legacy/`, `target/`, Codex runtime DB/cache 및 사용자 management/project data를 직접 수정·정리하지 않는다.
- remote push/PR/publish, dependency·executable 설치, Runtime 교체, signer·timestamp는 별도 승인 없이는 실행하지 않는다.

## 현재 상태

| 범위 | 상태 | 현재 판정 |
|---|---|---|
| P-0039~P-0059 | historical seal | 기존 core·23개 feature·16개 Profile의 현재 source 재감사 근거는 historical과 분리한다. |
| P-0060 Codex routing·delivery | deferred | 설치/package 후속은 본 Code Health source Slice와 분리하며 Runtime을 변경하지 않는다. |
| P-0061 PR0~31 final lock | held | CrossRepo bundle aggregate와 external gate를 보존한다. |
| P-0062 Code Health 정본 설계 | **DONE / local commit** | `5751ab5a269774931ca22afc28cf86b9d98e8039`; FULL 11/11 complete/stable/pass, inventory 23/23·Schema 213·MCP 170/170·Profile 16/16. |
| P-0063 SARIF 2.1.0 normalizer | **DONE / local commit** | `eed833af5387c9d6367199829839419511ba6000`; registered `SarifV210` parser→raw/normalized ArtifactRef→`ValidationRunV2`/`DiagnosticV2`→current Scan generation Finding/Occurrence→immutable import report. FULL 11/11 complete/stable/pass. |
| P-0064A structural clone | **DONE / local commit** | `d04cb2d7b664d52dd6228dddae57fb6a525ce458`; Rust exact token clone, production/test cohort, source-bound Finding/Occurrence, incremental reuse, macro·fixture exclusion과 FULL 11/11 complete/stable/pass. |
| P-0064B complexity regression | **DONE / local commit** | `c59c3a8789c582d8a43d0884855e799a39613588`; Rust AST complexity metric, compatible previous-index baseline의 new/worsened/improved relation, source-bound Finding과 FULL 11/11 complete/stable/pass. |
| P-0064C unused surface | **DONE / local commit** | `17f56418d27ea6fd5738090cefb38fe5fcddc329`; Rust function/type/file/export/dependency candidate, read-only dependency snapshot과 manifest/lockfile disagreement, build-script frontier, FULL 11/11 complete/stable/pass. |
| P-0065 Gate·Radar | **DONE / local commit** | `db3cb92b3ae9c9dbf332ebf8c1dfdafb50dcf75e`; code-health shadow planning과 read-only Radar projection, current schema/evidence, FULL 11/11 complete/stable/pass. |
| P-0066 Git history·ownership·debt | **DONE / local commit** | `a84d4f298a2a9e4a9b252da10abb1ffda8b13b60`; read-only Git/CODEOWNERS/debt snapshot, advisory Impact/Radar, privacy corpus, FULL 11/11 complete/stable/pass. |
| P-0067 semantic refactor provider | **DONE / local commit** | registered provider capability의 isolated preview를 typed PatchSetV2·existing pre/post Gate·exact approval path로만 정규화했고, absent provider fail-closed 및 Git worktree replay fixture를 FULL 11/11 complete/stable/pass로 검증했다. |
| P-0068 mutation·Rule Pack·repository posture | **DONE / local commit** | `937a9ac7898f6d30d1353b11429da52fca9b2199`; changed-code-only mutation budget·strict schema, versioned Rule Pack과 exact-tool SARIF digest binding, read-only posture snapshot/advisory Radar·fail-closed adapter 경계를 FULL 11/11 complete/stable/pass로 고정했다. |
| P-0069 EvaluationRun·Profile 결정 | **DONE / local commit** | `fbbf30ea14df34fc06582c22e30ab22c42ef341a`; Code Health trial/reject evidence를 고정했고, external cost/actual workload cohort 부재로 `trial_candidate`만 기록하며 16개 built-in 유지·17번째 accept hold를 FULL 11/11 complete/stable/pass로 확인했다. |
| P-0070 제품 전수 봉인 | **DONE / local commit** | final audit/ReviewPack, inventory 23/23·Schema 217·MCP 170/170·Profile 16/16, release source closure와 FULL 11/11 complete/stable/pass를 확인했고 signing/publish를 blocked_external로 분리했다. |
| P-0071 전체 STRICT 리뷰·main 전달 | **DONE / PUSHED** | `9ab88e0e069540800a4701d4516ae9692837bc77`; final FULL 11/11과 STRICT review 뒤 `origin/main` readback까지 완료했다. |
| P-0072 Windows x64 Runtime closure | **SOURCE REVIEWED / FULL PASS** | registered validation process를 suspended Job Object에 넣어 descendant escape를 닫았고 실제 `pwsh → pwsh` timeout/early-exit corpus 포함 `star-validation` 49/49와 FULL 11/11이 통과했다. x64 updater-only apply·commit/push는 진행 중이다. |
| public signed Stable | `blocked_external` | certificate/private key/trusted timestamp와 signed lifecycle/publish가 필요하다. |

## P-0072 활성 Slice

- 범위: Windows x64 registered process containment, current source seal, Runtime-only updater apply, Codex/fixed integration 무재시작 증거, local/remote `main` 일치다.
- 제외: Authenticode/timestamp/public publish와 non-x64 build·package·native evidence. Scorecard/OpenRewrite 등 absent executable은 설치하지 않는다.
- preflight: `main`/clean, `HEAD == origin/main == 9ab88e0`; installed `D:\도구\Star-Control` x64 verified, integration registered, management normal/read_write다.
- Profile closure: `project_understanding`, `ai_development_validation`, `architecture_quality`, `test_correctness`, `security_supply_chain`, `ci_release_deploy`; fingerprint `sha256:a9cf179b2d5b26fdfd22d1b010e7271277fba9865eb85a7229e02d3366e708d8`.

| 파일 | 상태 | 변경 요약 | 검증 상태 |
|---|---|---|---|
| `star-validation/process_executor` | 검토 완료 | suspended start → Job assign → primary-thread resume; bounded tree termination | package 49/49 pass |
| Windows dependency/lock | 구현됨 | existing workspace `windows 0.62.2` target-only 연결 | offline check pass, version/download 변화 없음 |
| validation evidence | 봉인됨 | inventory 23/23·Schema 217·MCP 170/170·Profile 16/16·Runtime executable 4/4 | `target/validation/20260727T172403972Z-36576/report.json`, FULL 11/11 complete/stable/pass |
| runtime evidence | 진행 중 | x64 generation·updater apply·PID/hash readback | Runtime-only Gate pending |

- 완료 조건: current inventory, FULL complete/stable/pass, STRICT review, updater-only apply committed, Codex PID/creation time과 fixed integration 불변, installed generation current, commit/push/readback, clean worktree다.

## 열린 위험과 보류

- R-0062: executable 존재만으로 registered provider가 되지 않는다. `cargo-mutants`와 pinned `rust-analyzer`는 관찰됐지만 mutation/semantic-refactor port의 exact descriptor·protocol·artifact binding이 없으므로 real result는 `unavailable|unverified`다. Scorecard/OpenRewrite는 설치하지 않는다.
- R-0063: `code_health_maintenance` 17번째 Profile은 EvaluationRun evidence와 제품 결정 전까지 추가하지 않는다. 기본은 기존 16 Profile 조합이다.
- R-0064: raw source, author name/email, secret 및 개인 absolute path는 ArtifactRef·fingerprint·Radar에 저장하지 않는다.

## Context Pack

- 현재 목표: P-0072 source seal과 x64 Runtime-only updater apply를 Codex 재시작 없이 완료하고 `main`을 원격 readback까지 일치시킨다.
- 다음 명령: source evidence 재봉인 → final FULL → source commit → x64 stage/inspect/updater apply → no-restart readback → final seal/commit/push.
- 건드리면 안 되는 것: existing user worktree, linked `target/`, `legacy/`, Codex runtime DB/cache, fixed Plugin/Hook, signing/publication, non-x64 output.
- 다음 완료 기준: installed Runtime generation과 final source closure가 일치하고 Codex/fixed integration identity가 유지된 채 `HEAD == origin/main == remote/main`, clean이다.
