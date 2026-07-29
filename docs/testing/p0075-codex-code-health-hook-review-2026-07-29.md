# P-0075 Codex Code Health route·Hook UX 최종 감사 — 2026-07-29

## 판정

- Code Health의 source 계약, Schema, handler, CLI/MCP route, Profile, 테스트와 x64 설치본을 전수 대조했다. 제품 inventory는 feature 23/23, generated Schema 217, MCP matrix 170/170, Profile 16/16, Runtime executable 4/4다.
- Codex-facing 필수 경로 `project.register → scan.run → index.status|search → finding.list|diagnostic.list` 6개는 installed Registry revision 13에서 모두 `ready`다. write action 2개는 `write_closed`, read action 4개는 `read_closed`다.
- 현재 Star-Control scan은 operation 성공과 제품 scan 완성을 구분한다. 마지막 bounded scan operation은 성공했지만 `scan_run.status=incomplete`이며 current/LKG index가 없으므로 index read는 `PROJECT_NOT_ATTACHED`, finding·diagnostic read는 빈 목록이다.
- Codex Desktop 설정 화면에 Hook 검토 목록이 없는 것은 discovery 실패가 아니다. 공식 검토 surface는 Codex CLI `/hooks`이고, App Server `hooks/list`는 8개를 발견했으며 `SessionEnd` 하나만 미신뢰로 남았다.
- 실제 cold Controller에서 CLI-only Radar 최초 조회가 10.3초에 실패한 근거로 public CLI/MCP handoff를 3×5초로 보강했다. source FULL·STRICT, x64 stage·installer, 공식 updater 설치·Runtime reconcile, installed TARGET·Doctor·Registry·Hook smoke를 완료했다. 다만 현재 Codex 계정의 다른 live 작업이 Controller lifecycle을 보유해 새 설치본의 격리 cold-live는 만들지 못했으며 PASS로 승격하지 않는다. 서명·ARM64·원격 push와 사람의 최종 Hook trust도 별도 Gate다.

## Codex 설정

| 범위 | 최종 값 | 확인 |
|---|---|---|
| 전역 `C:\Users\thdqu\.codex\config.toml` | `approval_policy="never"`, `sandbox_mode="danger-full-access"`, `network_access=true`, `features.hooks=true` | 재실행 후 현재 작업 permission profile이 unrestricted로 로드됨 |
| repo `.codex/config.toml` | 전역과 동일한 세 권한 값, `features.hooks=true` | tracked config와 live file 대조 |
| repo override 탐색 | `D:\개발` 아래 Star-Control 한 개 | hidden `.codex/config.toml` 전체 탐색 |
| Git 강제 push | 전역 `git-always-force-push=false` | 권한 확대와 원격 강제 push 기본값을 결합하지 않음 |

위 값은 사용자의 명시적 운영 정책이다. Codex 실행 권한과 별개로 push·삭제·외부 계정 등 작업 승인 경계는 `AGENTS.md`를 따른다.

## 전체 기능 경로 감사

| 기능군 | source·계약 경계 | 실행 surface와 현재 판정 |
|---|---|---|
| Project 등록 | exact allowlist root, Git identity, `.star-control/project.toml` 충돌 방지 | `star.core.project.register` ready; 15/15 catalog project available·identity match |
| ScanRun·index 생성 | `ScanRun → CodeIndexSnapshot → Finding/Occurrence`, source fingerprint와 tier limitation 보존 | `star.core.scan.run` ready; 마지막 scan은 bounded output과 `incomplete` 상태를 정직하게 반환 |
| Index read | current/LKG generation만 읽고 incomplete scan을 current로 승격하지 않음 | `index.status`, `index.search` ready; 현재 `PROJECT_NOT_ATTACHED` |
| Finding·DiagnosticV2 | source-bound Finding/Occurrence와 validation DiagnosticV2를 별도 read model로 유지 | `finding.list`, `diagnostic.list` ready; 현재 빈 목록 |
| SARIF 2.1.0 | 외부 raw/normalized ArtifactRef, strict parser와 immutable import report | source/test 완료; 외부 analyzer 결과는 등록된 provider가 있을 때만 인정 |
| structural clone | Rust exact-token clone, production/test cohort, macro·fixture 제외 | source/test 완료; Finding/Occurrence 경로에 통합 |
| complexity regression | Rust AST metric과 compatible previous-index baseline | new/worsened/improved relation을 source-bound Finding으로 보존 |
| unused surface | function/type/file/export/dependency candidate, manifest/lockfile 불일치와 build-script frontier | read-only candidate; 자동 삭제·dependency mutation 없음 |
| Gate·Code Health Radar | current source·validation evidence만 shadow planning/Radar에 사용 | code-health Radar는 current index 부재로 `PROJECT_NOT_ATTACHED` |
| Git history·ownership·debt | Git/CODEOWNERS/debt marker를 raw 개인정보 없이 advisory snapshot으로 정규화 | live history snapshot complete, component 12, debt marker 56; Radar item 68 |
| semantic refactor | registered exact provider의 isolated preview만 PatchSetV2와 pre/post Gate로 연결 | production provider 부재로 `unavailable|unverified`; executable 존재만으로 등록하지 않음 |
| mutation | changed-code-only budget와 strict result Schema | production provider 부재로 실제 mutation result 없음 |
| Rule Pack | versioned manifest와 exact analyzer/SARIF digest binding | stored manifest 0; dummy digest는 `SCAN_INCOMPLETE`/provider unverified로 fail-closed |
| repository posture | read-only posture snapshot과 advisory Radar | production provider 부재; source contract와 fixture만 완료 |
| EvaluationRun·Profile 결정 | trial/reject/accept evidence와 replay; evidence 부족 시 accept 금지 | live `evaluation_run_v2` 0, 17번째 Profile 미생성 |
| built-in Profile | 16개 catalog Profile의 deterministic resolve | 16/16 유지; 관련 6개 조합은 아래와 같음 |

관련 Profile은 `project_understanding`, `architecture_quality`, `test_correctness`, `ai_development_validation`, `refactor_codemod`, `security_supply_chain`이며, resolve 결과는 20 rule family와 permission floor `exact_durable_approval`을 보존한다. 새 `code_health_maintenance` Profile은 shadow EvaluationRun 근거와 제품 결정 전에는 추가하지 않는다.

## live Code Health 증거

- 등록 Project: `prj_01KYMDB5QKB1ZJYEZ2V7GZXP4S`, checkout `cko_01KYMDB5QKRMBC8NDK51H8804S`.
- bounded scan operation: `opn_01KYMGHCTMQ1T2GSDKFVYCGA3X`; operation은 `succeeded`, 제품 `scan_run.status`는 `incomplete`.
- snapshot: `cix_7kiecdbg2ief7qnxccnzsqfjczrc5gyou5hkrco7575kutvkj3cq`.
- counts: source 1,103, definition/symbol 17,275, reference 87,309, graph edge 519,901, finding projection 2,703, finding 1,118, limitation 1,377.
- limitation은 `INDEX_LANGUAGE_UNSUPPORTED`, `INDEX_RUST_ANALYZER_CROSS_FILE_TARGET_DEFERRED`, `INDEX_RUST_ANALYZER_REFERENCE_BUDGET`, `INDEX_SEMANTIC_UNAVAILABLE`, `INDEX_TIER_EXCLUDED_BY_CLASSIFICATION` 5개 code로 요약된다.
- Git history record는 `p0075-git-history-0e566f6-history`, Radar는 `p0075-git-history-0e566f6`이다. history completeness는 complete이고 Radar state/completeness는 partial이다. Radar fingerprint는 `sha256:a16b0cd4…`다.
- stored record 수는 `quality_rule_pack_manifest=0`, `maintenance_radar_snapshot=1`, `git_history_risk_snapshot=1`, `evaluation_run_v2=0`이다.

## 이번 Slice에서 교정한 결함

| commit | 교정 |
|---|---|
| `99ebcac5fa868001e0ac72e823076f05bd8a1569` | Code Health 6개 action을 required core package와 Codex route에 노출하고 Hook review surface를 CLI `/hooks`로 명시 |
| `5ef68eee6ddd7518910a145945b522f6fee8f324` | validation-configured project의 `.star-control/project.toml` 등록 충돌 교정 |
| `b3cc484ec42cd0b6feb1597f564b585cfcd1964d` | 9.35MB scan result를 bounded view로 바꾸고 malformed IPC를 `IPC_FRAME_INVALID`로 분류, Git history Radar ID 교정 |
| `ca6b452d266d913e731379fc77e85df49ee65bf1` | scan limitation을 count와 최대 5개 unique code로 요약 |
| `0e566f69efd9c0c2280f9b90375995f4b56a1614` | 일반 installed CLI postcheck를 45초 window/15초 attempt와 `kill_on_drop`으로 보강 |
| `1c3e626bada515f2309da09111eb8df30aa361c3` | public CLI/MCP Controller 시작의 5초 재시도와 Rust-style operation root ID 충돌 교정 |
| `cf95c1f7a9b1f4b2654f71098776d927740667ca` | Runtime reconcile의 남은 12초/2초 경계를 기존 45초/15초 cold-start 경계와 통일하고 Codex 권한 정책 반영 |
| `39d3a13f50ae65f6373174486e7b3389ad0c1bd3` | 실제 cold CLI handoff가 2×5초 경계를 넘긴 증거에 따라 개별 5초 timeout은 유지하고 public CLI/MCP 시작 시도를 3회, 총 15초로 보강 |

설치된 `cf95c1f`에서 Controller가 cold인 상태로 `development record show maintenance_radar_snapshot p0075-git-history-0e566f6 --json`을 실행했을 때 10.3초 뒤 `IPC controller unavailable`로 실패했고, 이미 기동된 Controller에 대한 즉시 재시도는 51ms에 성공했다. 이는 각 연결 시도의 5초 p95 계약이 아니라 총 2회 예산이 이 호스트의 약 11초 관리 DB cold start보다 짧다는 실측이다. `39d3a13`은 각 시도를 5초로 유지하면서 3회/15초만 허용한다.

새 설치 작업을 연 뒤 첫 사용자 CLI Radar read는 58ms에 성공했지만 Codex MCP가 Controller를 먼저 시작했으므로 cold 증거로 쓰지 않는다. 실제 `SessionEnd` lifecycle과 50.8초 idle 관찰로 격리를 시도했으나 다른 live Codex 작업 때문에 Controller가 유지됐다. 다른 작업이나 프로세스를 강제 종료하지 않았으며 새 설치본의 3번째 시도 live 사용 여부는 `unverified`다.

과거 초대형 operation `opn_01KYMDC1RV9J4A3P095VXYF57R` 재조회는 설치된 `cf95c1f`에서도 `IPC_AUTH_FAILED`가 아니라 `IPC_FRAME_INVALID`를 반환한다.

## 검증·설치 증거

| 항목 | 결과 |
|---|---|
| focused updater test | `star-updater-core` 19/19 pass |
| focused Clippy | `star-updater-core --all-targets -- -D warnings` pass |
| focused IPC test | `star-ipc` unit 26/26 + property 1/1 pass |
| focused IPC Clippy | `star-ipc --all-targets -- -D warnings` pass |
| source inventory | feature 23/23, Schema 217, MCP 170/170, Profile 16/16, Runtime EXE 4/4 |
| source FULL closure | `target/validation/20260729T040530019Z-20384/report.json`, 11/11 complete·stable·pass, 151,082ms, input `c0126714fa1f56eac16e5a8a0afa147ae141d4285d152157426932783f17c9e7`, report `sha256:ce2acd189acfd29db5def32745265b2bc3612a1cf8a8f9ed46875101d7ac65e1` |
| STRICT | Blocker 0, Major 0 |
| x64 stage | `dist/stage/p0075-final-39d3a13/x64`, 561 files, set `sha256:9ada5d7c6e467c3a957bad39de9ba668382621f30306c8b2020baff2186289b1`, manifest `sha256:c8dace083dbcfe50f827794171d77b48bb31bd87ed3f8b6c5311ad18dfa23f6d` |
| installer | `dist/installers/p0075-final-39d3a13/star-control-windows-x64-0.1.0-setup.exe`, `sha256:c14104266b326ac261db2562de63346eb799392bf83cffb921476bc40c6664a0`, `NotSigned` |
| installed source | `39d3a13f50ae65f6373174486e7b3389ad0c1bd3`, x64, `unsigned_local`, root/stage manifest byte-identical |
| Runtime activation | `rt_98f48bd0bef83be9`, revision 25, prior `rt_7335071b0d31a7b0` |
| reconcile | `upd_c_eG_MOEgqw9a6wSgDi733MUtAWYv5v-DDoyf3NYfo8`, 23 expected ToolId, integration verified, fallback PID 0 |
| first installed TARGET | operation `opn_01KYP03XHG8VBTHDX2SPMZ8B7B`, 7/8 fail, `ERROR_SHARING_VIOLATION(32)`, evidence `sha256:0c6fed43df45ef6368fbde9507973140a1fd36c59411915a8fa6cdbfabef7fbc` |
| targeted retry | `running_image_keeps_its_lease_and_the_next_call_observes_new_bytes` 5/5 pass |
| final installed TARGET | operation `opn_01KYP0ADDAKVR7Z6J9M71NTY8F`, 8/8 complete·stable·pass, 123,672ms, evidence `sha256:ddc7c4b2bfe4dc6e6049f27e72a709aa00152b89d5902d620287d7b4f7ce8958` |
| installed Doctor | 4/4 pass, implemented Controller command 23 |
| installed Registry | required package ready/trusted, Code Health 6/6 action ready/trusted, revision 13 |

`39d3a13` offline installer의 restart receipt `upd_CUbpbI_tf8b6grqITRvQS8L4kxkbHINafTpYfGGVefA`는 `partially_applied`를 남겼고 selector는 prior Runtime을 유지했다. root payload·installation record·stage manifest는 exact match였으며 candidate 재검사는 `no_change`였다. 새로 설치된 updater의 공식 reconcile이 위 operation으로 `rt_98f48bd0bef83be9`를 정상 승격했다. 과거 `cf95c1f` 설치의 `rollback_failed` receipt와 이번 `partially_applied` receipt는 역사 증거로 보존하고 성공으로 재작성하지 않았다.

첫 installed TARGET의 operation 상태 `succeeded`는 validation subprocess가 정상 반환됐다는 뜻일 뿐 report `fail`을 덮지 않는다. 동일 exact source에서 실패 테스트를 5회 통과한 뒤 전체 TARGET을 새로 실행해 8/8 PASS를 얻었으며, 첫 report도 삭제하지 않는다.

## Hook 진단과 남은 사람 Gate

- installed integration은 `verified`/`registered`, `requires_new_task=true`, `hook_trust_required=true`, `hook_review_surface=codex_cli`, `hook_review_command=/hooks`다.
- source/root Hook file은 서로 byte-identical(`sha256:fec608c0e55a5345a97d3dd1232159408cbb63e0a6eb69e383fffcdf526c8133`), rendered/cache Hook file도 서로 byte-identical(`sha256:f20b80cee24c8ae9aab8ad5cb7f56ac3bf50cb8c39c28453d4272a78b26013b7`)이다. renderer가 절대 Windows command를 넣으므로 source와 rendered hash가 다른 것은 정상이다.
- 8개 event가 모두 렌더됐다. `SessionEnd.timeout=3`, 나머지는 10초다.
- installed Hook smoke는 `SessionStart` JSON output·exit 0, `SessionEnd` 무출력·exit 0이다.
- 전역 trust state는 기존 7개 event를 보유하고 `SessionEnd` key가 없다. App Server가 계산한 current hash는 `sha256:968d14df66efccebded73826f7356a3634873b2c90e872f8dcb388e0fe1682cb`다.
- Desktop 설정 화면은 Hook browser가 아니다. Codex CLI에서 이 repo를 열고 `/hooks`를 실행해 `"D:\도구\Star-Control\star.exe" hook session-end`를 직접 검토·신뢰해야 한다.
- WindowsApps의 bundled `codex.exe`는 shell 실행 ACL이 거부한다. Codex UI 자체 자동화는 Computer Use 경계상 사용자 허락이 있어도 수행하지 않으며, 제품과 AI는 Hook trust DB/cache를 직접 수정하지 않는다.

공식 동작 근거는 [OpenAI Hooks documentation](https://learn.chatgpt.com/docs/hooks.md)이다.

## 남은 경계

- 사람 Gate: Codex CLI `/hooks`에서 `SessionEnd` trust 승인 후 새 작업으로 확인.
- 외부 Gate: Authenticode certificate/private key/trusted timestamp, signed installer lifecycle, SBOM/provenance, 원격 publish·digest readback.
- 플랫폼 Gate: 이번 설치 증거는 Windows x64만 소유한다. ARM64 native install을 주장하지 않는다.
- provider Gate: semantic refactor, mutation, Rule Pack analyzer, repository posture의 exact production descriptor·protocol·artifact binding은 아직 없다.
- 상태 Gate: current Code Health scan은 incomplete이고 live EvaluationRun은 0이다. source/test 존재를 현재 제품 결과로 승격하지 않는다.
- cold-live Gate: 새 설치본의 3번째 5초 attempt 사용은 다른 live Codex 작업을 종료하지 않고 격리할 수 없어 `unverified`다. 기존 `cf95c1f` 10.3초 실패와 `39d3a13` source/unit/FULL·installed TARGET을 서로 다른 증거로 유지한다.
- Git: `39d3a13f50ae65f6373174486e7b3389ad0c1bd3`까지 local commit이며 원격 push는 수행하지 않았다.
