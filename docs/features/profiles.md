# 개발 작업 Profile

상위 범위와 공통 선정 기준은 [구현 대상 기능](README.md)에서 확인한다.


## C01. 15개 작업 유형별 Profile

아래 항목은 서로 다른 제품 15개가 아니다. A·B 기능을 작업 성격에 맞게 조합하는 설정·템플릿이며, 대상 프로젝트가 가진 기존 도구를 adapter로 호출한다.

| Profile | 조합할 기능 | 기본 적용 경계 |
|---|---|---|
| `project_understanding` | Project Catalog, checkout·workspace, source 분류, text/syntax/semantic index, graph·freshness, Context Pack | 새 프로젝트 또는 큰 범위 작업을 시작할 때 |
| `change_planning` | 목표 계약, 영향 분석, 위험 경로, 단계 계획, 관련 검사 선택 | 여러 파일·계약에 걸친 변경 |
| `refactor_codemod` | 기준 동작 고정, 변환 범위, 반복 가능한 외부 codemod, diff·회귀 검증 | 넓은 기계적 변경이 필요한 경우 |
| `dependency_upgrade` | manifest·lockfile, 호환성·보안, 단계적 upgrade, rollback | dependency 또는 framework 변경 |
| `language_platform_migration` | 현재 동작 계약, 단계별 공존, 경계 adapter, equivalence와 전환 증거 | 언어·runtime·플랫폼 이동 |
| `data_config_db_migration` | version 사슬, rehearsal, invariant, backup·restore, 재개와 rollback | 데이터·설정·DB 형식 이동 |
| `api_contract_change` | 공개 계약 diff, 소비자 영향, 호환 기간, contract test, migration guide | API·CLI·Schema·파일 형식 변경 |
| `test_correctness` | 관련 테스트, 약화 탐지, 회귀 증거, 조건부 고급 테스트 | 버그 수정, 핵심 로직과 테스트 변경 |
| `architecture_quality` | layer·의존 규칙, cycle, 공개 경계, 예외와 ratchet | 구조 변경 또는 부채 정리 |
| `debug_recovery` | 실패 fingerprint, 재현 Pack, bisect·debug adapter, 수정 전후 증거 | 원인이 불명확한 실패와 복구 작업 |
| `performance_build` | workload, baseline, 반복 측정, profiler·build 분석, trade-off | 선언된 성능 경로나 build 병목 |
| `docs_config_environment` | 문서 명령·링크, 설정 계약, doctor, clean-room 재현 | 문서·설정·개발 환경 변경 |
| `ci_release_deploy` | CI 일치, package dry-run, artifact 신원, 배포·rollback 준비 | workflow, release 또는 배포 변경 |
| `security_supply_chain` | secret, dependency, 취약점·license, workflow, provenance | 보안 경로 또는 공급망 변경 |
| `ai_development_validation` | TaskSpec, scope·claim·evidence, test 약화, 검증기 보호, Review Pack | Codex가 생성하거나 수정한 모든 결과의 공통 마감 관문 |

다음 검사는 해당 프로젝트에 실제 대상이 있을 때만 Profile에 붙인다.

- GUI·웹 화면이 있으면 대상 프로젝트의 UI test 도구 연결
- DB가 있으면 migration 도구와 사본 환경 rehearsal 연결
- AI·RAG 기능이 있으면 prompt, retrieval, tool-use와 평가 자료 검증 연결
- 고위험 계산·parser·protocol에는 property·fuzz·mutation 도구 연결
- 여러 OS를 지원하는 대상 프로젝트만 해당 플랫폼 CI 결과 연결

`project_understanding`의 첫 실행은 사용자가 시작한 manual full scan이다. 이후 같은 Profile은 Git revision·file hash 기반 incremental scan을 우선하고, source·config·adapter fingerprint가 달라 재사용할 수 없을 때만 full scan을 요구한다. 출력은 [읽기 전용 Project Catalog와 Code Index](../contracts/project-catalog-and-code-index.md)의 ProjectCatalogSnapshot·CodeIndexSnapshot, tier별 coverage·limitation과 [ContextPack](../contracts/goal-and-stage.md)이다.

이 Profile은 CLI-only·source read-only다. project task·package script를 실행하거나 source를 수정하지 않으며 AI·embedding·LLM 의미 추론 자동화를 요구하지 않는다. semantic adapter가 없거나 parse가 실패하면 syntax·text fallback을 실제 tier로 표시하고, unsupported·partial·stale·no-result 이유를 ContextPack에서 보존한다.

위 Project Catalog·Code Index 동작은 현재 1단계 목표 설계이며 제품 scanner·parser·DB·watcher가 구현됐다는 뜻이 아니다.
