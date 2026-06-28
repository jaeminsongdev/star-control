> 흡수 출처: `star-control_design_v3/docs/06_MVP_구현_명세.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 06. MVP 구현 명세

## 1. MVP 목표

최초 MVP의 목표는 **Codex Adapter 하나로 Job → Route → WorkSpec → Worker → Report 흐름을 끝까지 돌리는 것**이다.

목표 명령:

```powershell
star-control run "스톱워치 만들어줘" --project "D:\개발\프로젝트A"
```

기대 산출물:

```text
runs/J-0001/
  request.md
  route.json
  run-state.json
  workspec-impl.md
  implement/
    output.md
    events.jsonl
    report.json
  review/
    output.md
    events.jsonl
    report.json
  final-report.md
```

## 2. MVP 범위

### 포함

- Job ID 생성
- request.md 저장
- Codex low-router 실행
- route.json 생성/검증
- WorkSpec 생성
- Codex worker-impl 실행
- output/report 수집
- Codex worker-review 실행
- final-report.md 생성

### 제외

- GUI
- 다중 provider
- 병렬 worktree
- PR 자동 생성
- 완전한 Goal Engine
- MCP bridge
- extension marketplace

## 3. CLI 명령 설계

```powershell
star-control run "스톱워치 만들어줘" --project "D:\개발\프로젝트A"
```

옵션:

| 옵션 | 설명 |
|---|---|
| `--project` | 작업 대상 루트 |
| `--provider` | 강제 provider 지정, 기본 auto |
| `--dry-run` | route/workspec만 생성하고 실행하지 않음 |
| `--resume J-0001` | 기존 job 재개 |
| `--stage implement` | 특정 stage만 실행 |
| `--max-iterations` | 반복 제한 |

## 4. Router 출력

Router는 반드시 `route.json`을 만들어야 함.

최소 예시:

```json
{
  "job_id": "J-0001",
  "summary": "스톱워치 기능 구현",
  "size": "MEDIUM",
  "risk": "LOW",
  "stages": ["implement", "validate", "review", "report"],
  "assignments": {
    "implement": { "role": "worker-impl", "provider": "codex" },
    "review": { "role": "worker-review", "provider": "codex" }
  },
  "requires_user_approval": false,
  "approval_reasons": []
}
```

## 5. Worker 실행

Codex MVP 예시:

```powershell
codex exec `
  --profile worker-impl `
  --json `
  --output-last-message "runs\J-0001\implement\output.md" `
  - < "runs\J-0001\workspec-impl.md" `
  > "runs\J-0001\implement\events.jsonl"
```

Codex CLI는 자동화용 `exec`, JSON event, output-last-message를 지원하므로 MVP provider로 적합하다.

## 6. Report 추출

Worker에게 마지막 응답에 JSON 블록을 포함하게 하거나, 별도 `report.json`을 쓰게 한다.

권장:

```text
output.md = 사람이 읽는 보고
report.json = Router Engine이 읽는 표준 보고
```

AI output에서 `report.json` 추출 실패 시:

1. output.md에서 JSON 블록 파싱 시도
2. 실패하면 worker에게 repair 요청
3. 다시 실패하면 `FAILED` 기록

## 7. 검증 MVP

처음에는 validation engine이 실제 모든 검증을 실행하지 않아도 된다. 대신 worker가 실행한 검증을 report에 기록하게 하고, review worker가 이를 검토한다.

최소 필드:

```json
"validation": [
  {
    "command": "npm test",
    "result": "passed",
    "notes": "all tests passed"
  }
]
```

## 8. 성공 기준

MVP 성공 조건:

- 명령 하나로 `J-0001` 폴더가 생성됨
- route.json이 schema 통과
- workspec-impl.md 생성
- implement output/report 생성
- review output/report 생성
- final-report.md 생성
- 실패 시 run-state.json에 원인 기록
