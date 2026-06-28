> 흡수 출처: `star-control_design_v3/docs/20_MVP_Acceptance_Tests.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 20. MVP Acceptance Tests

## 1. 목적

MVP는 모든 기능을 구현하는 것이 아니라, Star-Control의 핵심 루프가 실제로 끝까지 도는지 증명하는 단계다.

MVP 목표:

```text
사용자 요청 → route → workspec → worker 실행 → report → review → final report
```

---

## 2. MVP 범위

포함:

```text
Core schemas
Capability registry 최소판
Policy 최소판
Codex Adapter
Fake Provider
Run Engine
Validation/Review 기본
Final report
```

제외:

```text
GUI
모든 provider adapter
Extension package engine
Full Goal Engine
Full Hook Engine
Parallel workers
Automatic PR creation
```

---

## 3. Acceptance Test 1: Fake Provider Success

명령:

```powershell
star-control test e2e fake-success
```

기대:

```text
J-0001 생성
route.json valid
workspec valid
implement/report.json valid DONE
review/report.json valid APPROVE
run-state.json DONE
final-report.md 생성
```

---

## 4. Acceptance Test 2: Fake Provider Invalid Report

명령:

```powershell
star-control test e2e fake-invalid-report
```

기대:

```text
report schema validation 실패
run-state.json REPORT_INVALID
final-report.md에 실패 이유 기록
Core crash 없음
```

---

## 5. Acceptance Test 3: Policy Violation

입력:

```text
worker가 git reset --hard를 실행하려 함
```

기대:

```text
policy violation 감지
worker 중단
approval queue 또는 forbidden report 생성
run-state.json BLOCKED
```

---

## 6. Acceptance Test 4: Codex Smoke

명령:

```powershell
star-control provider check codex
star-control run "작은 README 문구 수정 계획만 세워줘" --project examples/projects/tiny --mode plan-only
```

기대:

```text
codex exec 호출 성공
output.md 생성
route/report 추출 성공
파일 수정 없음
```

---

## 7. Acceptance Test 5: Stopwatch Tiny Project

준비:

```text
examples/projects/tiny-stopwatch/
  package.json
  src/
  tests/
```

명령:

```powershell
star-control run "스톱워치 만들어줘" --project examples/projects/tiny-stopwatch
```

기대:

```text
구현 파일 생성 또는 수정
테스트 추가
검증 실행
리뷰 실행
final-report 생성
```

MVP에서는 실제 UI 완성도보다 pipeline completion을 우선한다.

---

## 8. Acceptance Test 6: Approval Required

입력:

```text
새 npm 패키지를 설치해서 스톱워치 만들어줘
```

기대:

```text
approval required 감지
npm install 자동 실행 금지
approval request 생성
run-state WAITING_APPROVAL
```

---

## 9. Acceptance Test 7: Local Draft Worker

명령:

```powershell
star-control run "스톱워치 테스트 케이스 초안 만들어줘" --provider local-ollama --draft-only
```

기대:

```text
repo 직접 수정 없음
draft.md 생성
report status DONE
changed_files 비어 있음
```

---

## 10. MVP Definition of Done

MVP는 아래 조건을 만족해야 완료다.

- [ ] schema validation 자동화
- [ ] fake provider E2E 통과
- [ ] policy violation test 통과
- [ ] Codex smoke test 통과
- [ ] final-report 생성
- [ ] 실패 시 상태가 안전하게 기록됨
- [ ] approval required 항목이 자동 중단됨
- [ ] docs/README만 보고 실행 가능
