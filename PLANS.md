# PLANS.md

## 목적

이 문서는 Star-Control의 **현재 설계·구현·외부 Gate 상태만** 남기는 bounded 원장이다. 단계별 상세 설계는 [문서 읽는 순서](docs/README.md), 구현 순서는 [최종 구현 로드맵](docs/roadmap/final-implementation.md), MCP 상세 결함과 증거는 [MCP 구현 완료 감사](docs/testing/mcp-completion-audit.md)가 소유한다.

## 현재 최종 설계 상태

- 0~10단계 전용 정본, 기능 요약, 계약·아키텍처·설치 문서, roadmap와 ADR이 서로 연결됐다. 선행 단계 누락은 없으며 상세 판정은 [10단계 gap matrix](docs/contracts/ci-release-evaluation-and-product-completion.md#09단계-선행-정본-gap-matrix)에 있다.
- 구현된 제품 범위는 MCP 기반 수직 Slice, P0 공통 개발 관리 첫 수직 Slice, Windows 설치 transport, 13개 추적 allowlist, tracked-path ValidationPlan, native validator 실행·evidence 조회와 success-only derived cache precursor다. M1 stable Checkout·persisted Catalog/Index와 full M2~M10 본체의 type·Schema·migration·engine·CLI·Corpus·release/provider adapter는 구현 전이다.
- 10단계는 local_quick·target·full·release, build-once artifact 승격, ready/approved/published 분리, Windows x64·ARM64 설치 수명주기와 EvaluationRun v2·Catalog lifecycle을 설계했다.

## 0~10단계 연결과 구현 상태

| 단계 | 정본 | 현재 상태 | 다음 제품 Gate |
|---:|---|---|---|
| 0 | [공통 개발 관리·로컬 관리 DB](docs/contracts/development-management.md) | P0 첫 수직 Slice 구현, 전체 lifecycle 미완 | Project v1→v2 migration과 repository compatibility |
| 1 | [Project Catalog·Code Index](docs/contracts/project-catalog-and-code-index.md) | 추적 allowlist·exact-root status precursor 구현, M1 본체 구현 전 | current Project/Checkout graph·freshness·query |
| 2 | [변경 계획·영향 분석](docs/contracts/change-planning-and-impact.md) | tracked-path ValidationPlan precursor 구현, full M2 구현 전 | TaskSpec→ready ValidationPlan full engine |
| 3 | [공통 검증·품질 Gate](docs/features/common-validation-gate.md) | native validator 실행·evidence 조회 precursor 구현, M3 본체 구현 전 | exact subject/evidence binding과 validator guard |
| 4 | [안전한 Patch·Refactor·codemod](docs/contracts/safe-patch-and-codemod.md) | 설계 확정·구현 전 | immutable preview·single-Project apply·recovery |
| 5 | [Managed Registry](docs/contracts/managed-symbol-registry.md) | 설계 확정·구현 전 | Git source Registry·consumer compatibility |
| 6 | [계약·문서·설정·환경](docs/contracts/contract-compatibility-and-environment.md) | 설계 확정·구현 전 | comparator·trace·doctor·clean-room readiness |
| 7 | [실패·보안·의존성 유지보수](docs/contracts/failure-security-and-dependency-maintenance.md) | 설계 확정·구현 전 | reproducibility·fresh external data·Radar |
| 8 | [Migration·성능·언어·플랫폼](docs/contracts/migration-performance-and-platform.md) | 설계 확정·구현 전 | recovery state·comparable measurement·equivalence |
| 9 | [CrossRepo ChangeBundle](docs/contracts/cross-repo-change-bundle.md) | 설계 확정·구현 전 | project-local effect·merge·remote adapter conformance |
| 10 | [CI·Release·평가·최종 제품 완성](docs/contracts/ci-release-evaluation-and-product-completion.md) | 설계 확정·구현 전 | M1~M9 current Gate 뒤 release/evaluation 구현 |

## 고정 제약

- CLI-only core가 기본이며 Codex 연동은 같은 application command의 선택 소비자다. AI 연동은 Codex만 지원한다.
- local AI, 다른 AI provider, OpenAI API 직접 호출, browser UI, HTTP control UI와 자체 예약 실행을 추가하지 않는다.
- compiler, scanner, debugger, profiler, package manager, CI, installer, signer, registry와 deploy service를 다시 구현하지 않고 typed adapter·evidence·Gate만 소유한다.
- canonical source·manifest·Catalog·policy가 정본이고 management DB·Project Catalog·Code Index·Finding projection은 derived state다.
- Controller application만 persisted current projection의 writer다. CLI·MCP·Codex·CI·installer·provider adapter는 직접 DB·Gate·release status를 쓰지 않는다.
- `legacy/`와 사용자 변경을 보존한다. 과거 Schema 생성 명령이 만든 로컬 `--check/`와 루트 `manifest.json`은 삭제하지 않되 Git 정본·commit 대상에서 제외하며, Schema 정본은 `specs/schemas/`만 사용한다.
- package/dependency 설치, system setting·PATH 변경, 파일 삭제·대량 이동은 action별 명시적 승인 없이는 실행하지 않는다. 검증된 명시 범위의 local commit은 별도 승인 없이 가능하되 사용자 변경을 숨기거나 섞지 않는다. push/PR, publish/deploy/withdrawal/rollback과 외부 account effect는 별도 승인을 받는다.
- release 계층은 같은 Task ID, source revision, Tool version, config·Catalog·Profile fingerprint를 사용한다. final artifact는 한 번 build·package하고 같은 digest를 승격한다.
- `ready`, `approved`, `published`, `publish_outcome_unknown`, `rollback_required`를 합치지 않는다. verified remote after-state 없이 published로 표시하지 않는다.
- EvaluationRun은 새/worsened 결함 방지를 우선하며 required validator·Corpus·ratchet을 약화해 통과율을 올리지 않는다. CLI-only와 Codex-integrated cohort를 합산하지 않는다.

## 남은 구현과 외부 Gate

| 구분 | 남은 항목 | 해제 조건 |
|---|---|---|
| 제품 기반 | M1~M3 contract migration, current graph, planning, common Gate | 각 단계 Schema·fixture·pure engine·CLI-only E2E와 제품 Gate |
| 안전한 변경 | M4~M6 Patch/Recipe, Registry, compatibility·doctor | single-Project recovery, consumer migration, clean-room evidence |
| 유지보수·migration | M7~M9 failure/supply-chain/Radar, migration/performance/language, ChangeBundle | fake adapter conformance 뒤 실제 target·Git·remote corpus |
| 최종 제품 | M10 ReleaseManifest/EvaluationRun/Catalog lifecycle, release/evaluation engine, package/install/provider adapter | M1~M9 current evidence와 required core owner 완료 뒤 구현 |
| MCP release blocker | required core 17개 중 6개 action ready, 11개 owning handler·Schema와 current evidence 미완 | [독립 감사](docs/testing/mcp-independent-audit-2026-07-12.md)의 BLOCK 해소·재검증 |
| Windows 환경 | Windows 11 24H2 build 26100+ x64·native ARM64 clean environment | build/runtime/IPC/install/update/rollback/uninstall 실기 evidence |
| 배포 기술 | Inno Setup local transport 이후 CI/artifact registry/release/deploy provider | channel 정책 확정, adapter conformance와 비용 승인 |
| 공급망 | channel별 SBOM·provenance·signing 필요성 | 법적·조직 policy 판정, credential/PKI·비용 승인과 evidence |
| 원격 공개 | publish·deploy·withdrawal·rollback·account change | exact manifest/digest/destination/before snapshot별 사용자 승인 |

## 현재 작업

| ID | 상태 | 목표 | 근거·다음 조치 |
|---|---|---|---|
| P-0037 | DONE — rollout | P-0005~P-0010 최종 전환 | 13개 native validator·manifest·CI/Hook을 단일 gate로 승격하고 nested Cargo graph·success-only derived cache·artifact 무결성·창 없는 child lifecycle을 구현; source FULL 10/10, contract 13/13, 최신 repair 설치와 installed MCP·cache-hit E2E를 확인 |
| P-0036 | DONE — cleanup | P-0011 구 체계 정리 | 사용자 결정에 따라 `.스타`, Star-Workflow·로컬_AI checkout, Star-Control ignored `legacy/`, 구 PATH를 제거하고 하나_프로젝트는 source를 보존한 채 nested Git·false-green workflow만 제거; 13개 allowlist·생태계 정본·언어 validator·Star-Control FULL과 잔여 process/terminal 0건 확인 |
| P-0035 | DONE — install | P-0010 repair 설치와 PATH 전환 | Attempt11 offline repair 완료; x64 program·manifest hash, PATH 우선순위, integration verified/registered와 installed MCP doctor·catalog·validation·evidence E2E를 확인했고 FULL 10/10·표시 터미널 0건을 기록함; Hook 신뢰는 사용자 검토 경계로 보존 |
| P-0034 | DONE — implementation | P-0005 Pilot explicit-unit resolver 복구 | Star-Control·개발도구 resolver가 closure 밖 사용자 함수를 찾던 결함을 self-contained function capture로 교체; 두 contract test 통과와 실제 `-Unit docs` 호출의 policy `partial` 도달 확인, Star-Control FULL 10/10 PASS; 개발도구 FULL은 resolver가 아니라 dirty Core dependency Gate에서 FAIL |
| P-0033 | DONE — governance | P-0009 AGENTS 프로젝트별 확산 | active canonical 13개를 30~80줄 소유권·역할·ready-only/native fallback·TARGET/FULL·승인 경계로 정렬하고 동결 2개를 읽기 전용화; 언어는 향후 code unit 활성화 조건을 포함하고 `래거시/하나_프로젝트`·LAWOS·참고본은 제외; CI gate·등록·설치·push 없음 |
| P-0032 | DONE — CI promotion | P-0008 CI·Hook 전환 | 13개 active repo의 PR `TARGET`, main `FULL`, 수동 `RELEASE`를 native entrypoint 1회 호출로 고정하고 Star-Control·개발도구 shadow/중복 command를 제거; 개발도구 pre-push는 짧은 TARGET, legacy false-green은 catalog 밖 source로 동결 |
| P-0031 | DONE — implementation | P-0007 영향도·cache·evidence precursor | sealed `ValidationPlan` v1, staged/unstaged/untracked·toolchain·lock/config/validator/command key, nested Cargo package/reverse consumer와 complete stable pass 전용 derived cache 구현; authoritative Gate/EvidenceBundle writer는 없음 |
| P-0030 | DONE — implementation | P-0006 최소 read-only action과 프로젝트 catalog | Git 추적 allowlist에 검증된 13개 active canonical만 고정하고 `doctor`, project `list/status` handler·Schema·action별 readiness·CLI/MCP 계약 구현; register·DB write 없음 |
| P-0029 | DONE — implementation | P-0005 프로젝트 공통 검증 pilot | `scripts/validate.ps1`에 adaptive `quick\|target\|full\|release`, unit·BaseRef·JSON evidence 계약을 구현하고 Rust 1.96.0 pin·CI를 동일 진입점으로 정렬; Star-Control FULL 10 checks PASS |
| P-0028 | DONE — implementation | P-0004 Plugin·Hook·MCP 지침 실제 표면 정렬 | fixed MCP 12개 유지; `star-control-operations`로 source·renderer 계약 교체; ready-only 호출·native fallback·Hook snapshot 검증; 설치본 repair는 후속 Gate |
| P-0027 | DONE — governance | P-0003 작업트리·구현 경계 정리 | `main@19379fe` clean 기준선에서 AGENTS 실행 경계 갱신; ready action 부재 시 native fallback, ignored 생성물·linked worktree·`legacy/` 보존; commit·push 없음 |
| P-0026 | DONE — implementation | 사용자 선택형 Windows 설치와 Codex Plugin/MCP/Hook 연동 수직 Slice | [로컬 실증](docs/testing/windows-installation-evidence-2026-07-14.md): x64/ARM64 패키지·`D:\도구\Star-Control` repair·Codex 앱 Plugin 설치·최종 검증 PASS; 외부 Gate는 아래 위험으로 분리 |
| P-0025 | IN PROGRESS — docs only | M11 `rust_style_auto_fix` Profile 설계와 정본 연결 | 공식 Rust 근거 재확인 → 신규 기능 정본·ADR·기존 계약/아키텍처/roadmap 반영 → Markdown 검증 |
| P-0024 | DONE — docs only, M10 제품 구현 전 | 10단계 상세 설계와 0~10단계 최종 감사 | [10단계 정본](docs/contracts/ci-release-evaluation-and-product-completion.md), [ADR-0010](docs/decisions/ADR-0010-Build-Once-승격과-Release-평가-Gate-경계.md), 아래 최종 문서 검증 |
| P-0012 | IN PROGRESS — verdict BLOCK | MCP 정본·제품·테스트·생성물·실행 증거 독립 감사와 결함 해소 | required core owner 구현 뒤 current evidence 재생성 |

## 열린 위험

| ID | 위험 | 현재 통제 | 해소 증거 |
|---|---|---|---|
| R-0042 | M1~M9 미구현 상태에서 fixture나 historical evidence로 release-ready를 가장할 수 있음 | 단계별 current Gate와 required core owner를 release preflight에서 fail-closed | 실제 M1~M9 제품 Gate·subject binding E2E |
| R-0043 | x64 성공 또는 ARM64 cross-build만으로 전체 Windows 지원을 주장할 수 있음 | native ARM64 runtime/install lifecycle required, 미확인은 `blocked_external` | clean x64·native ARM64 실기 matrix |
| R-0044 | 승격·서명·publish 중 rebuild나 byte 변경으로 source와 공개 artifact가 달라질 수 있음 | build once, artifact set digest, signing 뒤 새 candidate, after-state 재검증 | package/sign/publish byte digest chain |
| R-0045 | installer/update/rollback 실패가 user config·management state·project evidence를 훼손할 수 있음 | program payload와 user data 분리, backup·restore·uninstall preserve policy | crash 지점별 install lifecycle corpus |
| R-0046 | provider timeout·receipt·UI 표시를 published로 오판할 수 있음 | `publish_outcome_unknown`, read-only reconcile, exact after snapshot | provider별 pagination/rate/timeout/digest conformance |
| R-0047 | ground truth·표본 부족 또는 오탐 때문에 유용하지 않은 자동화가 승격될 수 있음 | comparable protocol, adjudication, `trial\|reject\|needs_review`, protected metrics | representative corpus와 독립 adjudication |
| R-0048 | 평가 결과를 좋게 만들기 위해 Rule·Check·Profile·Corpus를 약화할 수 있음 | validator guard, new/worsened ratchet, policy fingerprint, lifecycle migration Gate | adversarial validator/evaluation fixtures |
| R-0049 | SBOM·provenance·signing 요구를 항상 필수 또는 항상 불필요로 오판할 수 있음 | channel별 applicability decision과 근거를 ReleaseManifest에 고정 | 실제 공개 정책·법적 검토·provider evidence |

## 최신 검증 상태

- UNIT RESOLVER PASS — Star-Control·개발도구 validation contract test가 각각 exit 0이고 실제 docs unit 호출은 runner error 없이 validator 변경의 FULL 요구를 `partial`로 보존했다. 무제한 FULL에서 Star-Control 10/10은 PASS, 개발도구는 기존 dirty `D:/개발/코어`를 dependency-lock이 거부해 FAIL했으며 Core를 수정·정리하지 않았다.
- AGENTS ROLLOUT PASS — active canonical 13개 root AGENTS가 30~80줄, 사용자/AI 역할, ready-only action과 native fallback, TARGET/FULL, 승인 경계를 충족하고 구 도구명·구 운영 경로 0건이다. 구 관제·로컬 AI checkout은 제거했고 하나_프로젝트는 nested Git 없는 읽기 전용 source로만 남겼으며 LAWOS는 제외 상태다.
- CI PROMOTION PASS — active 13개에 같은 native contract를 두고 workflow 15개 YAML parse, contract 13/13, duplicate Cargo/schema/matrix command와 `continue-on-error`·echo-only false-green 0건을 확인했다. GitHub App은 13개 repository·main branch와 admin 접근을 확인했지만 branch protection/ruleset 상세 API는 현재 connector surface에 없어 required-check 서버 판정은 별도 외부 Gate로 남긴다.
- VALIDATION ENTRYPOINT PASS — adaptive profile·unit partial·`pass/fail/not_run/partial/unverified/flaky` 결과 계약, hash-pinned PyYAML과 stateless evidence를 계약 테스트로 확인했고 Rust 1.96.0에서 Star-Control FULL 10 checks를 통과했다.
- VALIDATION PLAN PASS — changed file/source/class, explicit root·nested Cargo workspace, direct unit·reverse consumer, adaptive profile·fallback과 validator-inclusive cache key를 확인했다. cache reader/writer는 immutable run/ref, complete stable pass, suppression 없음과 모든 artifact hash를 재검증하며 dirty·toolchain·lockfile·manifest·script/config/command/schema drift 또는 artifact 소실 시 execute로 복귀한다.
- ACTION SOURCE PASS — tracked allowlist parser·exact-root probe, handler/Schema/readiness mismatch guard와 fixed MCP wire `doctor`·`project list/status`·`validation plan/run`·`evidence get` dispatch를 확인했다. `validation.run`은 tracked native report와 cache disposition을 반환하고 non-pass를 성공으로 승격하지 않는다. authoritative management DB·GateDecision·EvidenceBundle writer는 추가하지 않았다.
- PLUGIN PASS — 새 Skill·Plugin validator, Hook exact snapshot, 고정 MCP 도구명 positive/negative 계약, 소유 package TARGET과 workspace FULL 검증을 통과했다. 설치본·Plugin cache·runtime state는 migration하지 않았다.
- POLICY PASS — P-0003 AGENTS QUICK Gate에서 변경 파일·필수 정책·heading·56줄 제한과 `git diff --check`를 확인했고, staged·untracked 0건 및 `manifest.json` hash·`--check/` 13개·등록 worktree 3개 불변을 확인했다.
- LEGACY CLEANUP PASS — 삭제 전 절대경로·reparse·HEAD·dirty fingerprint를 기록한 뒤 `.스타`, 구 checkout 2개, 하나_프로젝트 nested Git, ignored `legacy/`, 빈 `.codex`와 구 PATH를 제거했다. 하나_프로젝트 source 2,048개는 보존됐고 언어 FINDINGS=0, 생태계 FULL PASS, legacy 이름 directory·old process·validation process·표시 terminal은 모두 0건이며 `config.toml` SHA-256은 불변이다.
- FULL/PROCESS PASS — P-0036 정본·allowlist 변경 뒤 Star-Control native FULL이 38.6초에 10/10 PASS했고 evidence `target/validation/20260716T170713374Z-14220/report.json`을 남겼다. `partial`·`unverified`·`flaky`·`not_run` 성공 승격은 없고 검증 종료 뒤 cargo/rustc process와 표시 terminal은 0건이다.
- PACKAGE/INSTALL PASS — 최신 x64 package·offline repair 뒤 program·manifest hash, Star-Control PATH 우선순위, integration `verified/registered`, Plugin/Hook source 일치와 `config.toml` 보호 키 불변을 확인했다. native ARM64 실행·설치 증거는 기존 경계로 남는다.
- INSTALLED E2E PASS — 재시작 뒤 installed MCP에서 doctor·project list/status·validation plan/run/evidence가 ready이고 직접 실행과 같은 FULL 10/10 evidence를 반환하며 동일 입력 재실행은 검증된 cache hit였다. 표시 터미널과 잔여 validation child process는 0건이다.
- DOC PASS — 현재 Markdown 72개, Markdown link 929개에서 local file target 오류 0건, `docs/README.md` 읽는 순서 1~55 연속, `git diff --check` PASS다.
- 미검증 경계 — native ARM64 실행·설치, `/PURGEDATA`, code signing·공개 배포는 각각 환경·삭제·비용/외부 영향 승인이 필요한 별도 Gate다.

## 다음 수직 Slice

1. 별도 P-ID에서 allowlist membership을 강제하는 idempotent `project register` handler·Schema·CLI/MCP E2E를 구현하고, 승인된 전환에서만 `registration_enabled`를 연다.
2. 등록 결과는 `catalog/projects.toml`을 정본으로 가져온 derived management projection으로 만들며 임의 재귀 탐색과 인접 checkout 자동 등록을 금지한다.
3. 이어서 P0 `Project` v1→v2 checkout migration의 old/future/invalid fixture와 repository compatibility를 구현한다.
4. stable `ProjectCheckout`과 current `ProjectCatalogSnapshot`, 같은 `WorkspaceSnapshot`의 minimal `CodeIndexSnapshot`을 구현한다.
5. source/manifest canonical, DB/index derived, Controller single writer와 stale/freshness fail-closed를 unit·repository·CLI-only E2E로 검증한다.
6. P-0031 tracked-path precursor를 full M2 완료로 승격하지 않는다. 이 M1 Gate가 통과하기 전 full M2 graph planning, source rewrite, cross-project effect와 M10 release 구현을 시작하지 않는다.

## Archive References

- MCP 상세 감사·독립 판정·검증 행렬: [완료 감사](docs/testing/mcp-completion-audit.md), [독립 감사](docs/testing/mcp-independent-audit-2026-07-12.md), [검증 행렬](docs/testing/mcp-verification-matrix.md)
- P0 구현·통합 기준 commit: `0e94b23`
