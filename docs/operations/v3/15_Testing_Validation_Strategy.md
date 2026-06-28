> 흡수 출처: `star-control_design_v3/docs/15_Testing_Validation_Strategy.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 15. Star-Control 자체 테스트 / 검증 전략

## 1. 목적

Star-Control은 AI에게 코드를 맡기는 시스템이므로, Star-Control 자체가 불안정하면 위험하다.

따라서 Star-Control은 다음을 검증해야 한다.

1. 스키마가 정확한가?
2. 정책이 위험 행동을 차단하는가?
3. Provider Adapter가 결과를 표준화하는가?
4. Worker 실패 시 상태가 안전하게 전이되는가?
5. AI 출력이 불완전해도 시스템이 망가지지 않는가?

---

## 2. 테스트 계층

```text
Unit Tests
  ↓
Schema Tests
  ↓
Policy Tests
  ↓
Adapter Contract Tests
  ↓
Golden Run Tests
  ↓
End-to-End MVP Tests
  ↓
Provider Compatibility Tests
```

---

## 3. Schema Tests

대상:

```text
schemas/job.schema.json
schemas/route.schema.json
schemas/workspec.schema.json
schemas/report.schema.json
schemas/run-state.schema.json
```

검증:

- 정상 샘플 통과
- 필수 필드 누락 실패
- 잘못된 상태값 실패
- provider unknown 실패
- 위험도 enum 불일치 실패

샘플:

```text
tests/fixtures/valid/job-small.json
tests/fixtures/invalid/job-missing-project-root.json
```

---

## 4. Policy Tests

대상:

```text
policies/risk-policy.yaml
policies/approval-policy.yaml
policies/command-policy.yaml
policies/model-routing.yaml
```

검증:

- `git reset --hard`는 forbidden
- `git push`는 approval required
- `npm install`은 approval required
- `cargo test`는 자동 허용 가능
- CRITICAL risk는 user approval 필요
- local worker는 HIGH risk 배정 금지

---

## 5. Adapter Contract Tests

Provider Adapter가 공통 계약을 지키는지 확인한다.

필수 테스트:

```text
prepare_success
prepare_missing_cli
render_input_contains_workspec
start_run_creates_artifact_dir
collect_artifacts_reads_output
normalize_report_valid
normalize_report_invalid_schema
cancel_run_idempotent
```

---

## 6. Golden Run Tests

미리 저장된 입력과 기대 결과로 전체 routing을 검증한다.

예:

```text
tests/golden/stopwatch-small/
  request.md
  expected-route.json
  expected-workspec-impl.json
  expected-final-state.json
```

검증:

```text
star-control test golden stopwatch-small
```

---

## 7. Fake Provider

실제 AI를 쓰지 않고 테스트하기 위한 Fake Provider가 필요하다.

```text
providers/fake-success.yaml
providers/fake-failure.yaml
providers/fake-invalid-report.yaml
providers/fake-needs-approval.yaml
```

Fake Provider 역할:

- 항상 성공 report 반환
- 항상 실패 반환
- schema invalid report 반환
- timeout 시뮬레이션
- approval required 시뮬레이션

MVP 이전에 Fake Provider부터 구현해야 한다.

---

## 8. End-to-End MVP Test

최소 E2E 테스트:

```text
star-control run "스톱워치 만들어줘" --project examples/projects/tiny-web
```

기대 결과:

```text
runs/J-0001/request.md 생성
runs/J-0001/route.json 생성
runs/J-0001/workspec-impl.md 생성
implement/report.json 생성
review/report.json 생성
final-report.md 생성
run-state.json = DONE
```

---

## 9. AI Output Robustness Tests

AI 출력은 항상 깨질 수 있다.

검증해야 할 출력 오류:

- JSON 앞뒤에 설명문이 붙음
- 필수 필드 누락
- 상태값 오타
- changed_files가 문자열 하나로 나옴
- validation 결과가 자연어로만 나옴
- report가 markdown 코드블럭 안에 들어 있음
- 파일 경로가 OS별로 섞임

필요 기능:

```text
report_extractor
schema_validator
repair_prompt_generator
invalid_report_blocker
```

---

## 10. Provider Compatibility Tests

Provider별로 최소 smoke test를 둔다.

### Codex

- `codex --version`
- `codex exec --profile low-router` dry-run
- `--output-last-message` 파일 생성 확인
- command rules 적용 확인

### Local Ollama

- `ollama list`
- 모델 존재 확인
- prompt 입력 후 output.md 생성 확인

### Claude/Gemini/Cursor

초기에는 문서 검증만 하고, adapter 구현 후 smoke test 추가.

---

## 11. Regression Gates

아래 테스트는 release 전 필수.

```text
schema tests
policy tests
fake provider E2E
Codex adapter smoke
command policy dangerous examples
report normalization tests
```

---

## 12. Acceptance Criteria

- AI 없이 Fake Provider로 전체 E2E가 돈다.
- AI output이 깨져도 system crash 없이 `REPORT_INVALID`가 된다.
- 위험 정책이 테스트로 증명된다.
- Codex adapter smoke test가 통과한다.
- MVP release checklist를 통과한다.
