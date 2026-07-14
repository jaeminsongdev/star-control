# PLANS.md

## 목적

이 문서는 Star-Control의 **현재 설계·구현·외부 Gate 상태만** 남기는 bounded 원장이다. 단계별 상세 설계는 [문서 읽는 순서](docs/README.md), 구현 순서는 [최종 구현 로드맵](docs/roadmap/final-implementation.md), MCP 상세 결함과 증거는 [MCP 구현 완료 감사](docs/testing/mcp-completion-audit.md)가 소유한다.

## 현재 최종 설계 상태

- 0~10단계 전용 정본, 기능 요약, 계약·아키텍처·설치 문서, roadmap와 ADR이 서로 연결됐다. 선행 단계 누락은 없으며 상세 판정은 [10단계 gap matrix](docs/contracts/ci-release-evaluation-and-product-completion.md#09단계-선행-정본-gap-matrix)에 있다.
- 구현된 제품 범위는 MCP 기반 수직 Slice, P0 공통 개발 관리 첫 수직 Slice와 P-0026 Windows 설치 transport다. M1~M10 본체의 type·Schema·migration·engine·CLI·Corpus·release/provider adapter는 구현 전이다.
- 10단계는 local_quick·target·full·release, build-once artifact 승격, ready/approved/published 분리, Windows x64·ARM64 설치 수명주기와 EvaluationRun v2·Catalog lifecycle을 설계했다.
- 이번 P-0025는 M11 Rust 코드 스타일 자동 교정 Profile의 문서 설계 전용이다. build, test, package, installer, signing, publish, deploy, release 생성, 원격 계정 변경과 제품/generated 파일 변경을 수행하지 않는다.
- 이번 P-0026는 사용자가 별도로 승인한 Windows 설치·Codex Plugin 연동 수직 Slice다. 선택형 current-user 설치 경로, 설치 기록, Inno Setup 패키지, 로컬 Marketplace와 MCP/Hook 렌더링, 설치·복구·제거 검증까지만 구현하며 원격 공개·유료 서명은 수행하지 않는다.

## 0~10단계 연결과 구현 상태

| 단계 | 정본 | 현재 상태 | 다음 제품 Gate |
|---:|---|---|---|
| 0 | [공통 개발 관리·로컬 관리 DB](docs/contracts/development-management.md) | P0 첫 수직 Slice 구현, 전체 lifecycle 미완 | Project v1→v2 migration과 repository compatibility |
| 1 | [Project Catalog·Code Index](docs/contracts/project-catalog-and-code-index.md) | 설계 확정·구현 전 | current Project/Checkout graph·freshness·query |
| 2 | [변경 계획·영향 분석](docs/contracts/change-planning-and-impact.md) | 설계 확정·구현 전 | TaskSpec→ready ValidationPlan pure engine |
| 3 | [공통 검증·품질 Gate](docs/features/common-validation-gate.md) | 설계 확정·구현 전 | exact subject/evidence binding과 validator guard |
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
- package/dependency 설치, system setting, 파일 삭제·대량 이동, commit/push/PR, publish/deploy/withdrawal/rollback과 외부 account effect는 action별 명시적 승인 없이는 실행하지 않는다.
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
| MCP release blocker | required core 13개 owning command handler·Schema와 current evidence | [독립 감사](docs/testing/mcp-independent-audit-2026-07-12.md)의 BLOCK 해소·재검증 |
| Windows 환경 | Windows 11 24H2 build 26100+ x64·native ARM64 clean environment | build/runtime/IPC/install/update/rollback/uninstall 실기 evidence |
| 배포 기술 | Inno Setup local transport 이후 CI/artifact registry/release/deploy provider | channel 정책 확정, adapter conformance와 비용 승인 |
| 공급망 | channel별 SBOM·provenance·signing 필요성 | 법적·조직 policy 판정, credential/PKI·비용 승인과 evidence |
| 원격 공개 | publish·deploy·withdrawal·rollback·account change | exact manifest/digest/destination/before snapshot별 사용자 승인 |

## 현재 작업

| ID | 상태 | 목표 | 근거·다음 조치 |
|---|---|---|---|
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

- PRECHECK PASS — 기존 사용자 변경을 보존했고, 로컬 `--check/`·루트 `manifest.json`은 생성물로 확인해 삭제 없이 Git 정본·commit 대상에서 제외했으며 `legacy/` diff·status는 0건이다.
- FULL PASS — `cargo fmt --all -- --check`, workspace all-target `check`, workspace `test`, all-target/all-feature `clippy -D warnings`, generated Schema check와 MCP matrix 170/170을 통과했다.
- PACKAGE PASS — x64·ARM64 stage는 각각 77개 파일의 exact set·PE machine·hash 검증을 통과했다. stage set SHA-256은 각각 `64b65da4f62e8c810c4d0a6577dc28566aa90130a586c5390aa540f0f863bf9b`, `189d646185d1998018d59ffde887732da88a7660111a50fa63a0f4a9bed6e049`다. Inno Setup 6.7.3으로 만든 두 `unsigned_local` installer의 x64/ARM64 SHA-256은 각각 `fcf092bd7d244a463d2c6295242c81207348a9529a5d35aac632e1982605a88f`, `ecc042ad0e712726b2e339498f66eb6ee56b425d7f32f264669903262ead78c7`다.
- INSTALL PASS — x64를 `D:\도구\Star-Control`에 repair 설치했고 program hash, installation record, 실제 경로가 렌더링된 MCP·Hook, exact-owned HKCU 자동 시작, Hook 성공/거부와 rendered Plugin validator를 확인했다. Codex 앱에 Plugin을 실제 설치해 `MCP 서버 1`과 활성 Skill을 확인했고 Hook은 신뢰하지 않은 `검토 필요` 상태로 보존했다. Store 앱 CLI 실행 제약을 뜻하는 제품 record의 `manual_action_required`는 임의로 바꾸지 않았다. 상세 값은 [Windows 설치·Codex Plugin 로컬 실증](docs/testing/windows-installation-evidence-2026-07-14.md)에 있다.
- DOC PASS — 현재 Markdown 72개, Markdown link 929개에서 local file target 오류 0건, `docs/README.md` 읽는 순서 1~55 연속, `git diff --check` PASS다.
- 미검증 경계 — native ARM64 실행·설치, 실제 uninstall·`/PURGEDATA`, code signing·공개 배포는 각각 환경·삭제·비용/외부 영향 승인이 필요한 별도 Gate다.

## 실제 구현을 시작할 첫 수직 Slice

1. 별도 제품 작업 승인과 새 P-ID를 만든다.
2. P0 `Project` v1→v2 checkout migration의 old/future/invalid fixture와 repository compatibility를 구현한다.
3. read-only Project discovery를 stable `ProjectCheckout`으로 attach하고 current `ProjectCatalogSnapshot`을 만든다.
4. 동일 `WorkspaceSnapshot`에서 minimal `CodeIndexSnapshot`과 file/symbol/query surface를 만든다.
5. source/manifest canonical, DB/index derived, Controller single writer와 stale/freshness fail-closed를 unit·repository·CLI-only E2E로 검증한다.
6. 이 M1 Gate가 통과하기 전 M2 planning, source rewrite, cross-project effect와 M10 release 구현을 시작하지 않는다.

## Archive References

- MCP 상세 감사·독립 판정·검증 행렬: [완료 감사](docs/testing/mcp-completion-audit.md), [독립 감사](docs/testing/mcp-independent-audit-2026-07-12.md), [검증 행렬](docs/testing/mcp-verification-matrix.md)
- P0 구현·통합 기준 commit: `0e94b23`
