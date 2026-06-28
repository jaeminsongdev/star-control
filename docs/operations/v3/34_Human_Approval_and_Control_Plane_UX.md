> 흡수 출처: `star-control_design_v3/docs/34_Human_Approval_and_Control_Plane_UX.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 34. Human Approval과 Control Plane UX

## 목적

사용자는 진두지휘와 최종 확인만 한다. 따라서 Star-Control은 사용자가 봐야 할 결정을 압축하고, 승인할 것과 거부할 것을 명확하게 보여줘야 한다.

## 승인 요청 유형

| 코드 | 상황 |
|---|---|
| `DEPENDENCY_CHANGE` | 새 의존성 또는 버전 변경 |
| `FILE_DELETE` | 파일 삭제 |
| `PUBLIC_API_CHANGE` | 공개 API 변경 |
| `SCHEMA_CHANGE` | DB/파일/설정 스키마 변경 |
| `REMOTE_PUSH` | 원격 저장소 반영 |
| `GIT_COMMIT` | 커밋 생성 |
| `DEPLOY` | 배포/릴리즈 |
| `DANGEROUS_COMMAND` | 위험 명령 |
| `BUDGET_EXTENSION` | 예산 초과 후 계속 진행 |

## Approval Request 형식

```json
{
  "approval_id": "A-0001",
  "job_id": "J-0001",
  "type": "DEPENDENCY_CHANGE",
  "summary": "date-fns 의존성 추가 요청",
  "reason": "스톱워치 시간 포맷을 위해 사용 제안",
  "alternatives": [
    "표준 Date API로 구현",
    "직접 포맷 함수 작성"
  ],
  "recommended_decision": "reject",
  "risk": "MEDIUM"
}
```

## CLI UX

```powershell
star-control approvals list
star-control approvals show A-0001
star-control approvals approve A-0001
star-control approvals reject A-0001 --reason "표준 API로 구현"
```

## 승인 원칙

- 한 번 승인해도 전체 future approval을 허용하지 않는다.
- 승인 범위는 job/stage/command 단위로 좁게 둔다.
- 승인 결과는 audit log에 남긴다.
- 거부 시 Router는 대안을 생성하거나 BLOCKED로 종료한다.

## Control Plane 최소 화면

MVP는 GUI 없이 파일/CLI로 충분하다.

```text
approval-queue/
  A-0001.json
  A-0002.json
```

나중에 GUI에서 보여줄 핵심 카드:

- 현재 Job 상태
- 승인 대기 항목
- 변경 파일
- 검증 결과
- 리뷰 판정
- 남은 위험
- 비용/시간

## 최종 확인 보고서

사용자가 마지막에 봐야 하는 정보:

```text
- 무엇을 만들었는가
- 어떤 파일이 바뀌었는가
- 어떤 검증을 실행했는가
- 실패/미실행 검증은 무엇인가
- 사용자 승인이 필요한 것이 있었는가
- 남은 위험은 무엇인가
- 다음 추천 조치는 무엇인가
```
