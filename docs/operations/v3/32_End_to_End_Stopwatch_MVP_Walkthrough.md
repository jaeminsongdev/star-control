> 흡수 출처: `star-control_design_v3/docs/32_End_to_End_Stopwatch_MVP_Walkthrough.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 32. End-to-End Stopwatch MVP Walkthrough

## 목적

이 문서는 `star-control run "스톱워치 만들어줘"`가 실제로 어떤 파일을 만들고 어떤 단계를 거치는지 end-to-end로 보여준다.

## 1. 사용자 요청

```powershell
star-control run "스톱워치 만들어줘" --project "D:\개발\프로젝트A" --provider codex
```

## 2. Job 생성

```text
runs/J-0001/
  request.md
  job.json
  run-state.json
```

`job.json`:

```json
{
  "schema_version": "1.0.0",
  "job_id": "J-0001",
  "project_root": "D:\\개발\\프로젝트A",
  "request_text": "스톱워치 만들어줘",
  "state": "REQUESTED"
}
```

## 3. Route 실행

Router Engine이 `roles/router-low.md`와 `request.md`를 결합해 ProviderAdapter에 전달한다.

CodexAdapter 실행 예:

```powershell
codex exec --profile low-router --json --output-last-message runs/J-0001/router/output.md - < runs/J-0001/router/input.md
```

## 4. route.json 생성

```json
{
  "schema_version": "1.0.0",
  "job_id": "J-0001",
  "summary": "스톱워치 기능 구현",
  "size": "MEDIUM",
  "risk": "LOW",
  "stages": ["plan", "implement", "validate", "review", "report"],
  "assignments": {
    "implement": { "role": "worker-impl", "provider": "codex" },
    "review": { "role": "worker-review", "provider": "codex" }
  },
  "requires_user_approval": false
}
```

## 5. WorkSpec 생성

```text
runs/J-0001/workspec-impl.md
```

핵심 내용:

```md
# WorkSpec: J-0001 implement

## Goal
스톱워치 기능을 구현한다.

## Forbidden
- 새 의존성 추가
- 파일 삭제
- git commit
- git push
- 테스트 약화

## Required Output
- report.json
- changed_files
- validation_result
- risks
```

## 6. 구현 worker 실행

```powershell
codex exec --profile worker-impl --json --output-last-message runs/J-0001/implement/output.md - < runs/J-0001/workspec-impl.md
```

결과:

```text
runs/J-0001/implement/
  output.md
  events.jsonl
  report.json
```

## 7. Validation

Validation Engine이 report의 changed_files와 프로젝트 설정을 보고 명령을 선택한다.

예:

```text
npm test
```

결과:

```json
{
  "stage": "validate",
  "status": "DONE",
  "validation": [
    { "command": "npm test", "result": "passed" }
  ]
}
```

## 8. Review worker 실행

```powershell
codex exec --profile worker-review --json --output-last-message runs/J-0001/review/output.md - < runs/J-0001/workspec-review.md
```

Review report:

```json
{
  "stage": "review",
  "status": "DONE",
  "verdict": "APPROVE_WITH_NOTES",
  "blockers": [],
  "risks": []
}
```

## 9. Final report

```md
# Final Report: J-0001

## 결과
스톱워치 기능 구현 완료.

## 변경 파일
- `src/...`
- `tests/...`

## 검증
- `npm test`: passed

## 리뷰
- APPROVE_WITH_NOTES

## 남은 위험
- 없음
```

## 성공 조건

- [ ] `job.json` 생성.
- [ ] `route.json` schema 통과.
- [ ] `workspec-impl.md` 생성.
- [ ] worker output/report 수집.
- [ ] validation 결과 기록.
- [ ] review 결과 기록.
- [ ] final-report.md 생성.
- [ ] 실패 시 error taxonomy로 분류.
