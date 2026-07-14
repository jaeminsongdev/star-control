# 새 Star-Control 설계 문서

이 폴더가 새 Star-Control의 유일한 설계 기준이다. legacy/의 문서나 코드를 읽지 않아도 현재 목표, 전체 구조, 기능 범위, 구현 순서를 이해할 수 있어야 한다.

## 읽는 순서

1. [프로젝트 헌장](product/vision.md)
2. [사용자 경험과 전체 흐름](product/user-flow.md)
3. [전체 구조](architecture/system-overview.md)
4. [데이터 계약 지도](contracts/README.md) — 공통 형식, 전체 Inventory와 구현 기준
5. [단계 분해와 실행 계약](contracts/goal-and-stage.md)
6. [모델·생각 깊이·실행 방식 배정](contracts/routing.md)
7. [설정과 Catalog 계약](contracts/config-and-catalog.md)
8. [공통 개발 관리와 로컬 관리 DB 계약](contracts/development-management.md) — 0단계 domain·repository·rebuild 정본
9. [읽기 전용 Project Catalog와 Code Index 계약](contracts/project-catalog-and-code-index.md) — 1단계 discovery·checkout·index·freshness·fallback 정본
10. [변경 계획·영향 분석·affected 검사 선택 계약](contracts/change-planning-and-impact.md) — 2단계 TaskSpec·scope revision·impact·risk·fallback·3단계 입력 정본
11. [이벤트와 상태 계약](contracts/events-and-state.md)
12. [검사·완료·증거](contracts/validation-and-evidence.md)
13. [3단계 공통 검증·품질 Gate 상세 설계](features/common-validation-gate.md) — B01~B07 실행·Diagnostic·ratchet·validator guard·4단계 Patch Gate
14. [4단계 안전한 Patch·Refactor·codemod 엔진 계약](contracts/safe-patch-and-codemod.md) — Recipe·selector·rewrite assurance·dry-run·single-project apply·복구 정본
15. [5단계 관리형 Symbol·상수·에러 코드 Registry 계약](contracts/managed-symbol-registry.md) — 관리 분류·Git 정본·lifecycle·binding·consumer 호환·6단계 인계 정본
16. [6단계 계약 호환성·문서·설정·개발 환경 관리](contracts/contract-compatibility-and-environment.md) — baseline/current·consumer migration·docs/config drift·read-only doctor·clean-room·7단계 입력 정본
17. [7단계 실패 재현·보안·의존성 유지보수](contracts/failure-security-and-dependency-maintenance.md) — failure identity·ReproductionPack·supply-chain freshness·dependency PatchSet·Radar 정본
18. [8단계 Migration·성능·언어·플랫폼](contracts/migration-performance-and-platform.md) — version chain·backup/restore·checkpoint, comparable measurement, equivalence·9단계 ChangeBundle 인계 정본
19. [9단계 CrossRepo ChangeBundle](contracts/cross-repo-change-bundle.md) — MultiProjectGoal·project별 worktree/merge/evidence·비원자적 partial/recovery·remote approval·10단계 인계 정본
20. [10단계 CI·Release·평가·최종 제품 완성](contracts/ci-release-evaluation-and-product-completion.md) — 검사 계층·build-once·release 상태·설치 수명주기·EvaluationRun·최종 소유권 감사 정본
21. [11단계 Rust 코드 스타일 자동 교정 Profile](features/rust-code-style-auto-fix.md) — stable rustfmt·exact allowlisted Clippy·coverage·isolated PatchSet·`personal_auto` 정본
22. [MCP 도구 계약](contracts/mcp-tools.md)
23. [외부 Tool Registry와 고정형 MCP Gateway](contracts/external-tool-registry.md)
24. [MCP 구현 동결 계약](contracts/mcp-implementation-contract.md) — 고정 tool·wire·hash·상태기계 정본
25. [ToolPackageManifest Reference](contracts/tool-package-manifest-reference.md) — TOML 전체 문법
26. [Windows Local IPC 계약](contracts/local-ipc.md)
27. [오류와 진단 계약](contracts/errors-and-diagnostics.md)
28. [Version과 Migration 계약](contracts/versioning-and-migrations.md)
29. [Codex 통합과 진입 통제](architecture/codex-integration.md)
30. [승인·권한·안전](architecture/security-and-permissions.md)
31. [Windows Tool Runtime](architecture/windows-tool-runtime.md) — watcher·identity·process·격리 정본
32. [상태 기록과 이어하기](architecture/state-and-artifacts.md)
33. [병렬 작업과 병합](architecture/worktrees-and-merge.md)
34. [기능 범위와 레거시 판정](product/scope.md)
35. [설치와 공개 배포](operations/installation.md)
36. [Windows 설치와 Codex 연동 계약](contracts/windows-installation-and-codex-integration.md) — 선택형 경로·manifest·Plugin 렌더링·설치 수명주기 정본
37. [최종 구현 로드맵](roadmap/final-implementation.md)
38. [MCP 구현 검증 행렬](testing/mcp-verification-matrix.md) — 실제 Codex same-session 완료 gate와 [현재 완료 감사](testing/mcp-completion-audit.md), [Windows 설치·Codex Plugin 로컬 실증](testing/windows-installation-evidence-2026-07-14.md)
39. [용어](product/glossary.md)
40. [구현 대상 기능](features/README.md) — 23개 구현 기능과 최종 16개 작업 Profile
41. [최종 Repository·Package·문서 구조](architecture/repository-layout.md) — 물리 위치, Package 의존 방향, 확장 절차
42. [레거시 기능 카탈로그](history/legacy-feature-catalogue.md) — 과거 자료의 사실 기록
43. [구현 대상 선정 근거](history/source-selection-record.md) — 외부 자료·레거시 대응 기록
44. [D0 최종 설계 결정](decisions/ADR-0001-최종-설계-기준.md)
45. [데이터 계약·설정 정본 결정](decisions/ADR-0002-데이터-계약과-설정-정본.md) — P1 기계 계약의 고정 기준
46. [외부 도구 Registry·MCP Gateway 과거 결정](decisions/ADR-0003-외부-도구-레지스트리와-MCP-Gateway.md) — ADR-0004로 대체됨
47. [무재시작 고정 MCP·Live Tool Registry 결정](decisions/ADR-0004-무재시작-고정-MCP와-Live-Tool-Registry.md)
48. [MCP 구현 계약 동결 결정](decisions/ADR-0005-MCP-구현-계약-동결.md) — Terra 구현의 현재 정본
49. [공통 개발 관리와 로컬 관리 DB 결정](decisions/ADR-0006-공통-개발-관리와-로컬-관리-DB-경계.md) — P0 source·DB·evidence·Writer 경계
50. [P0 하이브리드 저장소와 운영 정책 결정](decisions/ADR-0007-P0-하이브리드-저장소와-운영-정책.md) — global/project store와 user-confirmed 기본값
51. [P0 embedded relational backend 결정](decisions/ADR-0008-P0-embedded-relational-backend.md) — private `rusqlite` adapter와 검증 gate
52. [Git 정본 Managed Registry와 Patch·Gate 경계 결정](decisions/ADR-0009-Git-정본-Managed-Registry와-Patch-Gate-경계.md) — derived DB Index·lifecycle·9단계 적용 보류
53. [Build Once 승격과 Release·평가 Gate 결정](decisions/ADR-0010-Build-Once-승격과-Release-평가-Gate-경계.md) — artifact byte·ready/approved/published·평가 보호 metric 경계
54. [Stable rustfmt·Allowlisted Clippy·Personal Auto 경계 결정](decisions/ADR-0011-Stable-rustfmt-Allowlisted-Clippy-Personal-Auto-경계.md) — M11 toolchain·coverage·exact PatchSet 승인 경계
55. [선택형 Windows 설치와 Codex Plugin 연동 결정](decisions/ADR-0012-선택형-Windows-설치와-Codex-Plugin-연동.md) — Inno Setup·실제 경로 렌더링·소유권·보존 경계

## 문서 상태 표현

- 확정: 사용자가 결정했고 구현 기준으로 사용할 내용
- 설계: 구현 전이지만 최종 제품 범위에 포함된 내용
- 보류: 외부 기능 변화나 실사용 검증 뒤 세부 방식을 확정할 내용
- 제외: 새 Star-Control에서 만들지 않을 내용

이 문서 세트는 최종 제품을 설명한다. 작은 시험판만 만들고 끝내는 계획이 아니며, 구현은 위험과 의존관계에 따라 여러 단계로 나누어 진행한다.

## 기준 관리

- 같은 규칙을 여러 문서에 복사하지 않는다.
- 한 문서가 기준을 소유하고 다른 문서는 그 문서로 연결한다.
- 현재 되는 것과 앞으로 만들 것을 구분한다.
- 제외한 기능은 다시 들어오지 않도록 이유와 함께 기록한다.
- Codex의 공개 기능은 실행 시점에 다시 확인한다.
- 레거시 기능 카탈로그는 현재 설계 계약이 아니라 과거 자료의 설명과 근거를 모은 참고 기록이다.
- 구현 대상 기능 문서는 상세 기능 범위를 소유하며, 기술과 외부 도구는 구현 직전에 최신 자료로 다시 조사한다.
- Repository 구조 문서는 기능을 구현할 물리 위치와 Package 소유권을 정하며, 새 Package나 정본 위치를 바꾸려면 이 문서를 먼저 갱신한다.
- MCP 구현은 개념 문서의 선택 표현보다 MCP 구현 동결 계약·Manifest Reference·Windows Runtime·검증 행렬의 exact 값을 우선한다.
- 현재 문서 폴더는 이 읽는 순서의 경로를 정본으로 사용하며, 과거 번호 파일은 남기지 않는다.

## 공식 Codex 근거

- Customization: https://developers.openai.com/codex/concepts/customization
- MCP: https://learn.chatgpt.com/docs/extend/mcp
- Hooks: https://developers.openai.com/codex/hooks
- Plugins: https://developers.openai.com/codex/build-plugins
- App Server: https://developers.openai.com/codex/app-server
- Models: https://developers.openai.com/codex/models
