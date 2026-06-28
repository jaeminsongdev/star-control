> 흡수 출처: `star-control_design_v3/README.md`
> 정리 상태: v3 설계 패키지 개요를 정규 문서로 흡수.

# Star-Control 설계 패키지

작성일: 2026-06-28  
목표: Codex, Claude Code, Gemini CLI, Cursor, GitHub Copilot, Jules, Devin, 로컬 모델을 공통 규격으로 다루는 **멀티 AI 작업 오케스트레이터** 설계.

## 이 패키지의 결론

Star-Control은 특정 AI 도구의 설정 묶음이 아니다. Codex는 1차 지원 Provider일 뿐이고, 원본 체계는 Star-Control 자체의 공통 스펙과 엔진이어야 한다.

```text
사용자 요청
  ↓
Star-Control Router Engine
  ↓
JobSpec / RouteSpec / WorkSpec / ReportSpec / RunState
  ↓
Policy / Capability / Provider Feature Matrix
  ↓
Provider Adapter
  ├─ Codex
  ├─ Claude Code
  ├─ Gemini CLI
  ├─ Cursor
  ├─ GitHub Copilot
  ├─ Jules / Devin
  └─ Local Models
```

## 문서 읽는 순서

1. `docs/00_최종_개요.md`
2. `docs/01_용어와_핵심개념.md`
3. `docs/02_전체_아키텍처.md`
4. `docs/03_Provider_기능조사_및_공통기능_분해.md`
5. `docs/04_핵심_스펙_설계.md`
6. `docs/05_구현_로드맵.md`
7. `docs/06_MVP_구현_명세.md`
8. `docs/07_보안_권한_정책.md`
9. `docs/08_Codex_1차_Adapter_설계.md`
10. `docs/09_로컬모델_Adapter_설계.md`
11. `docs/10_검토_보강_필요지점.md`
12. `docs/99_참고자료.md`

## 실제 구현 시작점

처음에는 전체 기능을 다 만들지 말고 아래 MVP만 만든다.

```text
Star-Control MVP:
1. JobSpec / RouteSpec / WorkSpec / ReportSpec / RunState
2. Capability Registry
3. Codex Provider Feature Matrix
4. Risk / Model Routing / Approval / Command Policy
5. Codex Adapter
6. Router Engine MVP
7. Validation / Review Engine 최소판
8. Local Draft Adapter 최소판
```

최초 성공 기준은 다음이다.

```powershell
star-control run "스톱워치 만들어줘" --project "D:\개발\프로젝트A"
```

실행 후 `runs/J-0001/` 아래에 `request.md`, `route.json`, `workspec-impl.md`, `implement/report.json`, `review/report.json`, `final-report.md`가 생기면 MVP 성공이다.

## v2 보강 문서

v2에서는 구현자가 실제 개발을 바로 시작할 수 있도록 다음 상세 문서를 추가했다.

- `docs/11_Provider_Adapter_계약.md`: provider adapter 공통 인터페이스와 실패 정규화
- `docs/12_Run_Thread_State_Machine.md`: job/run/thread 상태 전이와 resume/fork 전략
- `docs/13_Capability_Feature_Registry_상세.md`: capability registry와 provider feature matrix 운영법
- `docs/14_Renderer_산출물_체계.md`: Star-Control 원본을 Codex/Claude/Gemini/Cursor/GitHub 산출물로 변환하는 체계
- `docs/15_Testing_Validation_Strategy.md`: schema/policy/adapter/golden/E2E 테스트 전략
- `docs/16_Observability_Budget_Cost.md`: event log, artifact index, budget, quota 설계
- `docs/17_Security_Threat_Model.md`: secret/scope/approval/checkpoint/command threat model
- `docs/18_CLI_Command_Spec.md`: MVP CLI 명령 명세
- `docs/19_Migration_and_Provider_Update_Playbook.md`: provider 기능 변화 대응 절차
- `docs/20_MVP_Acceptance_Tests.md`: MVP 완료 조건과 테스트 시나리오
- `docs/21_Provider_전체기능_매트릭스_확장.md`: provider별 기능 비교 확장표
- `docs/22_Engine_상세_구현_명세.md`: Core Engine 모듈과 error taxonomy
- `docs/23_부족부분_검토_및_v2_보강내역.md`: v1 검토 결과와 v2 보강 내역
- `operations/Star-Control_MVP_Runbook.md`: MVP 실행 순서

## v3 추가 보강

v3에서는 v2 검토 후 구현자가 첫 코드 파일을 만들 때 필요한 운영 세부를 추가했다.

추가 문서:

- `docs/24_Config_Merge_and_Project_Layout.md`
- `docs/25_Error_Taxonomy_and_Recovery.md`
- `docs/26_Context_Pack_and_Memory_Compaction.md`
- `docs/27_Workspace_Isolation_and_Transaction_Model.md`
- `docs/28_Adapter_Conformance_Test_Suite.md`
- `docs/29_Schema_Versioning_and_Migration.md`
- `docs/30_Local_Model_Evaluation_and_Calibration.md`
- `docs/31_Rendered_Provider_Artifacts_Examples.md`
- `docs/32_End_to_End_Stopwatch_MVP_Walkthrough.md`
- `docs/33_Implementation_Blueprint_Module_Boundaries.md`
- `docs/34_Human_Approval_and_Control_Plane_UX.md`
- `docs/35_Data_Privacy_Retention_and_Secrets.md`
- `docs/36_Provider_Docs_Refresh_Checklist.md`

추가 정책/예시:

- `policies/data-policy.yaml`
- `policies/error-taxonomy.yaml`
- `examples/fake/*`
