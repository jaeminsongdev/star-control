# 상태 기록과 이어하기

## 목표

앱을 닫거나 작업이 실패해도 처음부터 다시 조사하지 않도록 목표, 단계, 결과, 다음 행동을 안전하게 저장한다.

EventEnvelope, RunSnapshot, Checkpoint, Handoff와 전이 불변식의 상세 형식은 [이벤트와 상태 계약](../contracts/events-and-state.md)이 소유한다.

## 작업 상태

| 상태 | 의미 |
|---|---|
| draft | 목표가 처음 만들어짐 |
| clarifying | 필요한 질문을 확인 중 |
| planned | 단계와 배정 결과가 만들어짐 |
| approved | 계획이 실행 가능함 |
| running | 한 개 이상의 단계가 실행 중 |
| paused | 사용자가 일시 중단함 |
| validating | 검사 중 |
| reviewing | 독립 검토 중 |
| merging | 병렬 변경을 통합 중 |
| blocked | 사용자 결정이나 외부 상태가 필요함 |
| failed | 자동 복구 범위를 넘겨 실패함 |
| cancelled | 사용자가 취소함 |
| completed | 완료 조건과 증거가 충족됨 |

상태가 바뀔 때 시간, 이유, 관련 단계를 함께 기록한다.

## 저장 위치

### Controller 상태

배경 Controller가 다시 시작해도 필요한 내부 상태는 Windows 사용자 로컬 데이터 폴더에 저장한다.

    %LOCALAPPDATA%\Star-Control\

여기에는 여러 프로젝트를 연결하는 실행 목록, 잠금, Plugin 상태, App Server 작업 식별자를 둔다.

### 프로젝트 증거

프로젝트별 실행 증거는 대상 프로젝트에 둔다.

    <project>\.ai-runs\star-control\<run-id>\

Star-Control 저장소 자체가 아니라 실제 작업 대상 프로젝트에 기록한다.

### 여러 프로젝트 작업

전체 목표의 연결 정보는 Controller 상태에 두고, 각 프로젝트의 변경과 검사는 각 프로젝트 .ai-runs/에 둔다. 서로의 절대 위치를 복제하지 않고 안정적인 프로젝트 식별자와 상대 경로를 우선 사용한다.

## 실행 폴더 예시

    <run-id>\
      goal.json
      plan.json
      capability-snapshot.json
      events.jsonl
      stages\
        <stage-id>\
          stage.json
          route.json
          context-summary.json
          permission-plan.json
          validation-plan.json
          result.json
          checkpoint.json
      evidence\
        changes.json
        validations.json
        cost.json
        risks.json
        final-summary.md
      merge\
        merge-plan.json
        conflicts.json
        result.json

파일 이름은 backend 구현에서 달라질 수 있지만 각 파일이 담는 의미는 [데이터 계약 지도](../contracts/README.md)의 Schema ID를 따른다.

## 저장 원칙

- 중요한 상태 파일은 중간 상태가 보이지 않게 안전하게 교체한다.
- 이벤트 기록은 순서대로 추가한다.
- 동시에 쓰는 프로세스가 있으면 잠금을 사용한다.
- 잘못된 상태는 조용히 무시하지 않는다.
- 모르는 새 필드는 가능한 한 보존한다.
- 절대 경로와 사용자 이름을 불필요하게 공개하지 않는다.

## 이어하기 기록

이어하기 기록에는 다음만 남긴다.

- 현재 목표와 단계
- 이미 끝난 결과
- 실패 원인과 시도한 방법
- 아직 남은 일
- 건드리면 안 되는 범위
- 관련 파일
- 다음 검사
- 다음 단계에 필요한 모델과 실행 방식
- 현재 작업 복사본과 병합 상태

전체 대화와 전체 로그를 다음 Codex에 그대로 전달하지 않는다.

## 보관 기간

보관 정책은 설정할 수 있다.

- 실행 중 기록: 삭제하지 않음
- 완료 요약과 핵심 증거: 장기 보관
- 큰 원문 로그: 설정된 기간 후 정리 가능
- 임시 파일: 안전한 종료 뒤 정리
- 실패 재현에 필요한 기록: 문제가 닫힐 때까지 보관

설계 기본값은 완료 run의 큰 원문·중간 artifact 90일, 해결된 실패 재현 자료 180일이다. 최종 요약·manifest, 실행 중 자료, 보존 hold와 미해결 실패 자료는 자동 정리하지 않는다. 공개 배포 전 실제 사용량을 측정해 기본값 변경이 필요한지 검토한다.

## 비밀정보

- 상태와 증거에 인증키 원문을 넣지 않는다.
- 환경 변수 값은 이름과 사용 여부만 기록한다.
- 외부로 내보낼 보고서는 한 번 더 가림 검사를 한다.
