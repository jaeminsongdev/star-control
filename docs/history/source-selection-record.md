# 구현 대상 선정 근거

이 문서는 외부 개발 도구 자료, 검증 플랫폼 자료와 레거시 기능 카탈로그가 현재 구현 대상 기능에 어떻게 반영됐는지 기록한다. 현재 제품 범위는 [구현 대상 기능](../features/README.md)이 소유한다.


이 표는 조사 자료가 별도 기능으로 복제된 위치가 아니라, 최종 구현 요소로 흡수된 위치를 보여준다.

## 개발 도구 01~15

| 조사 문서 | 반영 기능 |
|---|---|
| 01 프로젝트 이해·코드 탐색 | A03, A10, `project_understanding` |
| 02 변경 영향·작업 계획 | A01, A02, A04, `change_planning` |
| 03 자동 리팩터링·codemod | A04, B01, B02, `refactor_codemod` |
| 04 의존성·framework upgrade | A08, B05, B09, `dependency_upgrade` |
| 05 언어·플랫폼 migration | A02, B04, B06, `language_platform_migration` |
| 06 데이터·설정·DB migration | A07, B04, B06, `data_config_db_migration` |
| 07 API·계약·호환성 | A04, B04, B09, `api_contract_change` |
| 08 테스트·정확성 | B01, B02, B03, `test_correctness` |
| 09 코드 품질·아키텍처 | A04, B03, B04, `architecture_quality` |
| 10 debugging·incident·recovery | A07, B06, `debug_recovery` |
| 11 성능·자원·build 효율 | A10, B08, `performance_build` |
| 12 문서·설정·개발 환경 | A03, A10, B07, `docs_config_environment` |
| 13 CI·release·deployment | A08, B09, D03, `ci_release_deploy` |
| 14 보안·의존성·공급망 | A08, B03, B05, `security_supply_chain` |
| 15 AI 개발 작업 검증 | A01, B01, B02, B03, D02, `ai_development_validation` |

## 검증 플랫폼 v4 자료군

| 자료군 | 반영 기능 |
|---|---|
| 선별 기준·감산 결과·75개 기능 inventory | 선정 기준, A03·A04, B01~B09 |
| 전체 구조·공통 기반·구현 roadmap·ticket | A01, A07, A10, B01, B03 |
| AI 작업 통제·Review Pack 상세 | A01, B01, B02, C01 |
| 테스트·검증기 보호 상세 | B02, B03, D02 |
| 정적 분석·아키텍처·계약 상세 | A04, B04 |
| 보안·공급망·신뢰성 상세 | A08, B05, B09 |
| 조건부 후순위 판단·최종 요약 | B02, B08, B09, C01의 조건부 경계 |
| TaskSpec·policy·diagnostic·corpus·Review Pack Schema와 template | A01, A08, B01, B03, A10의 향후 계약 입력 |
| checklist·CSV·manifest·references | 선정 누락 확인, 자료 무결성·추적 근거 |

## 레거시 기능 카탈로그 20개 기능군

| 레거시 기능군 | 반영 기능 |
|---|---|
| 설계 기준·결정 기록·roadmap | A01, A02, A10 |
| 사용자 요청·Job 수명주기 | A01, A02, A07 |
| Schema·데이터 계약·설정 병합 | A10, B04 |
| StateStore·artifact·event | A07, B01 |
| Router·위험·단계 배정 | A02, A04, A05 |
| Provider·Registry·Capability·Readiness | A05, A10의 Codex 전용 기능 |
| Provider 실행 조정·결과 정규화 | A05, A06, B01의 Codex 전용 기능 |
| Fake·Local·Cloud·Human 경로 | A05, A06의 Codex 단일 실행 경로와 사용자 승인 |
| ValidationEngine·결과 전달 | B01~B09 |
| Star Sentinel 정책 검사 | A08, B03, B05 |
| 승인·Review Pack·최종 보고 | A08, B01 |
| CLI 실행·조회·제어 | A06, A07 |
| Daemon·Queue·Scheduler | A06, A07, A09의 Controller·대기열; 반복 예약 제외 |
| HTTP API 조회·제어 | A06의 Codex 공식 통합 경계 |
| Browser UI·Job 관제·Provider 설정 | A06의 터미널 관제와 Codex 설정으로 흡수 |
| 보안·Privacy Handoff·Redaction | A08, B05 |
| Audit·Event·Log·Cost·Budget | A07, B01, D02 |
| Recovery·Retention·Artifact 교체 | A07, B06 |
| Release Readiness·승인형 자동화 | B09, D03 |
| CI·E2E·GitHub·PR·Worktree | A09, B09, D01 |
