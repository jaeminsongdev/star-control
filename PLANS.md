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
| P-0072 Windows x64 Runtime closure | **DONE / PUSHED / INSTALLED** | `f13533922fa68a906494e97cea4087578697d49c`; Runtime generation `rt_9582adc516129569`, source FULL 11/11·installed TARGET 8/8·Doctor 4/4, `origin/main` readback까지 완료했다. |
| P-0073 Codex Plugin·Skill·Hook 재설계 | **DONE / INSTALLED / local commits** | operations Skill/metadata의 Code Health·updater route와 `SessionEnd` lifecycle을 x64 installed Updater로 적용했다. source FULL 11/11, installed TARGET 8/8, Doctor 4/4, rendered/cache identity와 terminal integration receipt를 확인했다. |
| P-0074 SessionEnd 3초 계약 교정 | **DONE / INSTALLED / local commits** | `d825516b86823793d53b607d6b2a0b6852d459fb`; `SessionEnd.timeout=3`, 2초 내부 deadline, terminal Updater receipt, rendered/cache identity, Hook smoke·Doctor·installed TARGET을 확인했다. |
| P-0075 Codex Code Health route·Hook UX closure | **DONE / INSTALLED / local commits** | `39d3a13f50ae65f6373174486e7b3389ad0c1bd3`; Code Health 6개 action·16 Profile·Hook 경계를 전수 감사하고 public CLI/MCP handoff를 3×5초로 보강했다. source FULL·STRICT, x64 설치, Runtime reconcile, final installed TARGET 8/8·Doctor 4/4를 완료했다. 격리 cold-live와 사람 `/hooks` trust는 PASS로 승격하지 않는다. |
| P-0076 Star-Control 자체 Code Health 적용 | **DONE / INSTALLED / local commits** | `ccb71661b86895783c440760fcd0bd76dcff9e3f`; 자체 scan이 드러낸 GoalRecord·redaction·SQLite quota·cursor clone과 대형 `index.status` IPC 결함을 교정했다. source FULL 11/11, Runtime `rt_8921c607a0af357f`, current self-scan, installed TARGET 8/8·Doctor 4/4를 확인했다. |
| P-0077 목표추진 병렬 구현 Skill·Controller cold-start | **ACTIVE** | Sol Max가 설계·DAG·재계획을 소유하고 Terra High 목표추진 작업자가 실제 구현·교정을 수행하는 기본 Skill을 제품화한다. 7.1GB management DB startup 중 pipe가 열리지 않는 `IPC controller unavailable`도 typed busy/terminal retention 상태를 보존하는 non-blocking startup으로 교정하고, 12개 forward scenario·FULL·STRICT·x64 설치·`origin/main` readback까지 닫는다. |
| public signed Stable | `blocked_external` | certificate/private key/trusted timestamp와 signed lifecycle/publish가 필요하다. |

## P-0074 closure

- 구현·설치 감사는 [P-0074 SessionEnd Hook 3초 계약 교정·설치 감사](docs/testing/p0074-session-end-hook-timeout-2026-07-28.md)가 소유한다.
- source FULL은 11/11 complete·stable·pass다. closure MCP operations의 Controller idle shutdown은 pass로 합성하지 않고 동일 canonical native entrypoint로 fallback했다. 설치 후 TARGET은 8/8 complete·stable·pass, Doctor는 4/4 pass다.
- x64 outer source는 `d825516b…`, active Runtime은 `rt_4f5e2b2ea6dbe52d`로 분리되며 candidate 재검사는 `no_change`다.
- Codex cache/runtime DB 직접 수정, signing/publication, non-x64 output, 원격 push는 수행하지 않았다.

## P-0075 closure

- 구현·설치·Hook 감사는 [P-0075 Codex Code Health route·Hook UX 최종 감사](docs/testing/p0075-codex-code-health-hook-review-2026-07-29.md)가 소유한다.
- exact installed source는 `39d3a13f50ae65f6373174486e7b3389ad0c1bd3`, active Runtime은 `rt_98f48bd0bef83be9`, activation revision은 25다. restart receipt의 `partially_applied`는 숨기지 않고 공식 reconcile `upd_c_eG_MOEgqw9a6wSgDi733MUtAWYv5v-DDoyf3NYfo8`로 selector만 승격했다.
- 첫 installed TARGET은 Windows sharing violation 한 건으로 7/8 fail이었다. exact 테스트 5/5 pass 뒤 새 TARGET operation `opn_01KYP0ADDAKVR7Z6J9M71NTY8F`이 8/8 complete·stable·pass였고 Doctor 4/4, Code Health 6/6 action ready/trusted다.
- Hook source/root, rendered/cache identity와 paired smoke를 확인했다. Desktop 설정은 Hook browser가 아니며 `SessionEnd` trust는 Codex CLI `/hooks` 사람 Gate로 남긴다.

## P-0076 closure

- 상세 재현·수정·self-scan 전후 증거는 [P-0076 자체 Code Health dogfood](docs/testing/p0076-self-code-health-dogfood-2026-07-29.md)가 소유한다.
- source full scan `scn_01KYP58EJXA5A5QX1H6XQHNZ9J`은 1,103 source, Finding 3,151, 21,263,878-byte snapshot으로 성공했다. post-refactor scan `scn_01KYP66XD8XVPG9S4S0MEN733J`에서 cursor codec clone occurrence는 0이다.
- bounded `index.status` 회귀, additive Schema, inventory 23/23·Schema 217·Profile 16/16·error 533·MCP 170/170과 clean commit FULL `target/validation/20260729T071000617Z-14616/report.json` 11/11을 통과했다. 중간 FULL 10/11과 격리 재실행 증거는 상세 문서가 보존한다.
- 공식 Updater로 x64 Runtime `rt_8921c607a0af357f`를 activation revision 27에 적용했다. candidate와 실제 process identity 모두 Codex restart 불필요를 확인했다. final incremental scan `scn_01KYPBXCHWQNH5HHHNZHRQWBKR`, current bounded `index.status`, installed TARGET `target/validation/20260729T072410843Z-15216/report.json` 8/8, Doctor 4/4가 pass다. 검증 Goal은 revision 2 `cancelled`다.
- active Runtime은 product-code commit `ccb71661…`에 정확히 묶이고, 뒤따르는 DONE 문서·source-evidence commit은 별도 current-byte 봉인이다. Codex cache/DB 직접 수정, dependency 설치, remote push는 수행하지 않았다.

## 열린 위험과 보류

- R-0062: executable 존재만으로 registered provider가 되지 않는다. `cargo-mutants`와 pinned `rust-analyzer`는 관찰됐지만 mutation/semantic-refactor port의 exact descriptor·protocol·artifact binding이 없으므로 real result는 `unavailable|unverified`다. Scorecard/OpenRewrite는 설치하지 않는다.
- R-0063: `code_health_maintenance` 17번째 Profile은 EvaluationRun evidence와 제품 결정 전까지 추가하지 않는다. 기본은 기존 16 Profile 조합이다.
- R-0064: raw source, author name/email, secret 및 개인 absolute path는 ArtifactRef·fingerprint·Radar에 저장하지 않는다.
- R-0065: Codex Hook은 보조 guardrail이다. `PermissionRequest`를 실제 Star-Control PermissionPlan/Approval 판정 없이 추가해 강제 통제로 표현하지 않는다.
- R-0066: Codex Hook trust와 새 작업은 사용자 보안 경계다. `integration status`가 `hook_trust_required=true`, `requires_new_task=true`를 보존하며 제품은 Codex trust DB/cache를 직접 수정하지 않는다.
- R-0067: `SessionEnd`는 advisory라 timeout이 Codex 작업을 막지는 않지만 `root_stop` 관찰을 잃을 수 있다. 내부 deadline과 updater의 보수적 process census를 함께 유지한다.
- R-0068: 첫 installed TARGET의 `ERROR_SHARING_VIOLATION(32)` report는 삭제하지 않는다. targeted 5회와 final TARGET은 통과했지만 일회성 Windows 실행 이미지 unlock 지연 가능성을 별도 이력으로 유지한다.
- R-0069: 새 설치본의 3×5초 격리 cold-live는 다른 live Codex 작업이 Controller lifecycle을 보유해 실행하지 못했다. 기존 10.3초 실패, 단위 계약, source FULL과 installed TARGET은 cold-live PASS를 대신하지 않는다.
- R-0070: 현재 active Runtime Controller는 7.1GB project management DB startup 중 pipe 생성 전에 장시간 CPU를 사용해 CLI/MCP가 `IPC controller unavailable`을 반환한다. DB/cache 직접 수정이나 autostart 활성화로 우회하지 않고 source startup 경계와 공식 Updater로 교정한다.

## P-0075 봉인 범위

- Hook trust 저장소·Codex cache/runtime DB는 직접 수정하지 않는다. 공식 `hooks/list` discovery와 Codex CLI `/hooks` 사람 검토 경계를 보존한다.
- required core package에는 `project.register`, `scan.run`, `index.status`, `index.search`, `finding.list`, `diagnostic.list`의 owning command·strict input/output Schema·lane만 additive로 연결한다.
- 외부 analyzer executable을 자동 등록·설치하지 않으며 semantic/mutation/posture provider 부재는 `unavailable|unverified`로 유지한다.
- 검증은 source FULL, STRICT review, official Updater candidate inspect와 installed Registry search/describe를 사용한다. Runtime/Bridge apply는 candidate inspect가 허용하고 exact scope가 현재 요청과 일치할 때만 수행한다.
- `cf95c1f` 설치본의 실제 cold Controller는 2×5초 예산을 약 0.3초 초과했다. 각 연결 시도는 5초로 유지하고 총 시도만 3회로 늘렸다. 새 설치본의 isolated cold handoff는 후속 수동 Gate로 분리한다.

## Context Pack

- 현재 상태: P-0077은 active다. source checkout `23a1438adb54d289bbcf19b1a26c06d34982f46a` 위 working tree에 새 Skill 9개와 closed inventory·Controller startup 교정이 함께 있으며, source FULL `target/validation/20260729T120132516Z-12016/report.json`은 11/11 PASS다. installed x64 Runtime은 아직 `rt_8921c607a0af357f`라 MCP Registry와 Profile resolution의 cold-start `unavailable`은 설치 전 상태로 남아 있다.
- 완료 조건: 새 Skill의 9개 정본 파일과 packaging/render/install closed inventory, Sol Max 설계·Terra High 목표추진 worker 계약, 12개 forward scenario, non-blocking Controller startup 회귀, source FULL·STRICT, official x64 install, installed TARGET·Doctor/live action, commit, `origin/main` exact readback이다.
- 건드리면 안 되는 것: existing linked worktree, `target/` 정리, `legacy/`, management DB·Codex runtime/cache 직접 수정, dependency·lockfile, signing/publication, non-x64 output.
- 다음 실행: Sol Max STRICT 교정을 재검토하고 source evidence·FULL을 다시 봉인한 뒤 clean pinned packaging worktree에서 x64 installer를 만든다. 설치는 verified candidate와 공식 Updater 경로만 사용하며 installed TARGET·Doctor/live action을 확인하고 push는 모든 Gate 뒤 마지막에 수행한다.
