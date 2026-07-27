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
| P-0071 전체 STRICT 리뷰·main 전달 | **REVIEWED / FULL PASS / DELIVERY PENDING** | `origin/main..HEAD` 11커밋의 코드·계약·실기능·보안·유지보수성 결함을 수정했고 preliminary FULL 11/11 complete/stable/pass를 확인했다. 문서 seal 뒤 final FULL, intended-only commit, `origin/main` push/readback만 남았다. |
| public signed Stable | `blocked_external` | certificate/private key/trusted timestamp와 signed lifecycle/publish가 필요하다. |

## P-0071 활성 Slice

- 범위: `00bd842..e5efc11`의 65개 파일, 8,663 additions를 canonical owner·contract/Schema·handler·CLI/MCP route·positive/negative/failure/replay corpus·current evidence의 여섯 층으로 검토한다.
- 현재 preflight: checkout identity match, installed x64 verified, integration registered, management normal/read_write, doctor 4/4 PASS, `origin/main...HEAD=0/11`이다.
- Profile closure: `project_understanding`, `ai_development_validation`, `api_contract_change`, `architecture_quality`, `test_correctness`, `security_supply_chain`, `ci_release_deploy`; resolution fingerprint `sha256:45f8bcb9191198a92cb42155704ad62bb2a1af5f94e5093f4a41f43a470044d3`.

| 파일 | 상태 | 변경 요약 | 검증 상태 |
|---|---|---|---|
| `origin/main..HEAD` 변경 파일 | 검토 완료 | STRICT 코드·기능·계약·보안 리뷰와 Blocker/Major 수정 | regression corpus와 전체 workspace test 통과 |
| P-0071 review patch | 수정됨 | process/SARIF, complexity, Git history, mutation/Rule Pack/posture, registered effect 보강 | preliminary FULL 11/11 complete/stable/pass |
| `PLANS.md`와 P-0071 audit | 수정됨 | bounded review/delivery snapshot과 residual boundary | 문서 seal 포함 final FULL 예정 |

- preliminary FULL: `target/validation/20260727T163051012Z-25012/report.json`, 174,588ms, 11/11 pass, partial/unverified/flaky 0.
- current inventory: feature 23/23, Schema 217, MCP 170/170, Profile 16/16, Runtime executable 4/4.

- 완료 조건: Blocker/Major 수정과 회귀 corpus, current inventory evidence, FULL complete/stable/pass, strict self-review, local commit, push, `HEAD == origin/main == remote/main`, clean worktree다.

## 열린 위험과 보류

- R-0062: real external analyzer/provider executable과 network snapshot은 설치·승인 없이 실행할 수 없다. adapter fixture만 구현하고 실제 provider 결과는 `unavailable|unverified`로 남긴다.
- R-0063: `code_health_maintenance` 17번째 Profile은 EvaluationRun evidence와 제품 결정 전까지 추가하지 않는다. 기본은 기존 16 Profile 조합이다.
- R-0064: raw source, author name/email, secret 및 개인 absolute path는 ArtifactRef·fingerprint·Radar에 저장하지 않는다.

## Context Pack

- 현재 목표: P-0062~P-0070의 전체 변경을 STRICT 재검토하고 실제 기능 증거를 재생성한 뒤 `origin/main`에 전달한다.
- 먼저 확인할 파일: contracts/ports/application/validation/project/release, CLI/controller route, schemas/fixtures, product inventory와 P-0070 audit.
- 먼저 실행할 명령: 변경 public surface/위험 패턴 scan; commit별 diff review; focused corpus와 CLI/controller smoke; inventory; final FULL.
- 건드리면 안 되는 것: existing dirty/user changes, `target/`, `legacy/`, installed Runtime/cache, public signing/publish/install state.
- 다음 완료 기준: review findings가 닫히고 FULL 통과 뒤 P-0071 commit을 push하여 local/remote SHA가 일치한다.
