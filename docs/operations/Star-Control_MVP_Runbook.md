> 흡수 출처: `star-control_design_v3/operations/Star-Control_MVP_Runbook.md`
> 정리 상태: 운영 runbook으로 흡수.

# Star-Control MVP Runbook

## 1. 목표

이 Runbook은 구현자가 처음 Star-Control MVP를 실행하기 위한 순서를 정의한다.

---

## 2. 준비

```powershell
mkdir D:\개발\Star-Control
cd D:\개발\Star-Control
```

필수 확인:

```powershell
git --version
python --version
codex --version
```

선택 확인:

```powershell
ollama list
```

---

## 3. 초기화

```powershell
star-control init --global D:\개발\Star-Control
star-control init --project D:\개발\프로젝트A
```

---

## 4. 설정 검증

```powershell
star-control validate schemas
star-control validate policies
star-control provider check codex
```

---

## 5. Fake Provider E2E

```powershell
star-control test e2e fake-success
star-control test e2e fake-invalid-report
```

---

## 6. Codex Smoke

```powershell
star-control render codex --dry-run
star-control render codex --apply
star-control provider check codex
```

---

## 7. 첫 작업 실행

```powershell
star-control run "스톱워치 만들어줘" --project D:\개발\프로젝트A --provider codex
```

---

## 8. 상태 확인

```powershell
star-control status J-0001
star-control report J-0001
```

---

## 9. 실패 시

```powershell
star-control status J-0001
cat runs\J-0001\run-state.json
cat runs\J-0001\events.jsonl
cat runs\J-0001\final-report.md
```

---

## 10. 완료 기준

- final-report.md 생성
- run-state DONE 또는 명확한 BLOCKED/FAILED
- 모든 report가 schema valid
- 위험 명령이 자동 실행되지 않음
