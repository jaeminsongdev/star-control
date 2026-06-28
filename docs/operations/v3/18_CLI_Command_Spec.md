> 흡수 출처: `star-control_design_v3/docs/18_CLI_Command_Spec.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 18. Star-Control CLI 명세

## 1. 목적

초기 MVP는 GUI 없이 CLI로 운영한다.

목표:

```text
star-control run "스톱워치 만들어줘" --project D:\개발\프로젝트A
```

이 명령 하나로 Job → Route → WorkSpec → Worker → Report 흐름이 동작해야 한다.

---

## 2. 명령 목록

```text
star-control init
star-control run
star-control route
star-control worker
star-control resume
star-control status
star-control report
star-control render
star-control validate
star-control policy check
star-control provider check
star-control test
```

---

## 3. `init`

프로젝트 또는 전역 Star-Control 구조를 생성한다.

```powershell
star-control init --global D:\개발\Star-Control
star-control init --project D:\개발\프로젝트A
```

산출물:

```text
D:\개발\Star-Control\router.yaml
D:\개발\프로젝트A\PLANS.md
D:\개발\프로젝트A\.ai-runs\
```

---

## 4. `run`

사용자 요청을 받아 전체 job을 시작한다.

```powershell
star-control run "스톱워치 만들어줘" --project D:\개발\프로젝트A
```

옵션:

```text
--project <path>
--provider codex|claude-code|gemini-cli|local-ollama|auto
--mode auto|plan-only|implement|review-only
--risk LOW|MEDIUM|HIGH|CRITICAL
--dry-run
--require-approval
--no-local
--max-cloud-runs <n>
```

---

## 5. `route`

라우팅만 실행한다.

```powershell
star-control route runs/J-0001/request.md --project D:\개발\프로젝트A
```

산출물:

```text
runs/J-0001/route.json
runs/J-0001/workspec-*.md
```

---

## 6. `worker`

특정 WorkSpec을 실행한다.

```powershell
star-control worker run runs/J-0001/workspec-impl.md
```

옵션:

```text
--provider codex
--role worker-impl
--profile worker-impl
--output runs/J-0001/implement
```

---

## 7. `resume`

Job을 이어서 실행한다.

```powershell
star-control resume J-0001
```

동작:

- `run-state.json` 읽기
- 다음 stage 확인
- 필요한 worker 실행
- final report 생성

---

## 8. `status`

현재 job 상태를 표시한다.

```powershell
star-control status J-0001
```

출력:

```text
Job: J-0001
State: REVIEWING
Current stage: review
Last worker: worker-impl DONE
Next action: start worker-review
```

---

## 9. `report`

최종 보고서를 생성하거나 다시 생성한다.

```powershell
star-control report J-0001
```

---

## 10. `render`

Star-Control 원본 설정을 provider별 산출물로 변환한다.

```powershell
star-control render codex --dry-run
star-control render codex --apply
star-control render all --dry-run
```

---

## 11. `validate`

스키마와 설정을 검증한다.

```powershell
star-control validate schemas
star-control validate policies
star-control validate providers
star-control validate all
```

---

## 12. `policy check`

명령이나 작업이 정책에 걸리는지 확인한다.

```powershell
star-control policy check command -- git reset --hard HEAD
star-control policy check risk --workspec runs/J-0001/workspec-impl.md
```

---

## 13. `provider check`

Provider 사용 가능 여부 확인.

```powershell
star-control provider check codex
star-control provider check local-ollama
star-control provider list
```

---

## 14. `test`

테스트 실행.

```powershell
star-control test schema
star-control test policy
star-control test golden stopwatch-small
star-control test e2e fake-provider
```

---

## 15. Exit Codes

```text
0  success
1  generic failure
2  schema validation failed
3  policy violation
4  provider unavailable
5  approval required
6  worker failed
7  report invalid
8  budget exceeded
```

---

## 16. Acceptance Criteria

MVP CLI 완료 기준:

- `run`으로 job directory 생성
- `route`로 route/workspec 생성
- `worker run`으로 provider 실행
- `status`로 상태 확인
- `report`로 final report 생성
- `render codex --dry-run`으로 산출물 preview 가능
- `test fake-provider`가 통과
