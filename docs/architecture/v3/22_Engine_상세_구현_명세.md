> 흡수 출처: `star-control_design_v3/docs/22_Engine_상세_구현_명세.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 22. Engine 상세 구현 명세

## 1. 목적

이 문서는 Star-Control Core Engine들의 실제 구현 책임을 정의한다.

---

## 2. Core 모듈 구조

권장 내부 패키지:

```text
src/star_control/
  core/
    ids.py
    paths.py
    config.py
    errors.py

  schemas/
    loader.py
    validator.py

  state/
    job_store.py
    run_store.py
    artifact_store.py
    event_log.py

  policy/
    risk.py
    approval.py
    command.py
    scope.py
    budget.py

  routing/
    router.py
    model_selector.py
    stage_planner.py

  providers/
    base.py
    codex.py
    local_ollama.py
    fake.py

  renderers/
    codex.py
    local.py

  hooks/
    engine.py

  quality/
    validation.py
    review.py
    report_normalizer.py

  cli/
    main.py
```

---

## 3. ID Generator

ID 형식:

```text
J-0001 job
W-0001 worker run
E-000001 event
A-0001 artifact
APP-0001 approval
```

ID는 job directory 기준으로 증가한다.

---

## 4. Schema Validator

책임:

- JSON Schema 로드
- sample validation
- report validation
- invalid report error 생성

함수:

```python
validate_json(schema_name: str, data: dict) -> ValidationResult
validate_file(schema_name: str, path: Path) -> ValidationResult
```

---

## 5. State Store

책임:

- job 생성
- run-state.json 읽기/쓰기
- event append
- artifact index 관리

쓰기 원칙:

```text
atomic write
append-only event log
raw output 보존
```

---

## 6. Router Engine

책임:

- 사용자 요청을 JobSpec으로 변환
- router-low worker 실행
- route.json 검증
- route가 invalid면 repair 또는 failed
- stage별 WorkSpec 생성

---

## 7. Model Selector

입력:

```text
job request
risk
size
provider features
budget
quota
policy
```

출력:

```text
stage -> role/provider/profile
```

---

## 8. Provider Manager

책임:

- provider registry 로드
- adapter 선택
- capability check
- fallback provider 선택

---

## 9. Run Manager

책임:

- worker run 시작
- timeout 관리
- poll/collect
- report normalization
- retry
- status transition

---

## 10. Hook Engine

책임:

- hook config 로드
- event별 hook 실행
- hook 실패 정책 적용

hook 실패 처리:

```text
noncritical hook fail -> warning
critical hook fail -> BLOCKED
policy hook fail -> POLICY_VIOLATION
```

---

## 11. Policy Engine

책임:

- risk policy 평가
- approval policy 평가
- command policy 평가
- scope policy 평가
- budget policy 평가

정책 결과:

```text
ALLOW
PROMPT
FORBID
BLOCK
ESCALATE
```

---

## 12. Report Normalizer

책임:

- raw output에서 JSON 추출
- schema validation
- missing field 보정 가능 여부 판단
- invalid report 저장
- standard ReportSpec 생성

---

## 13. Validation Engine

책임:

- 프로젝트 검증 명령 탐색
- 명령 실행 권한 확인
- validation report 생성

탐색 순서:

1. 사용자 지정
2. 프로젝트 문서
3. CI 설정
4. package config
5. 언어별 후보 명령

---

## 14. Review Engine

책임:

- changed files 읽기
- diff 생성
- review worker 실행
- review report schema 검증
- BLOCKER 있으면 중단

---

## 15. Renderer Engine

책임:

- provider 산출물 생성
- dry-run diff
- apply
- backup

---

## 16. Error Taxonomy

표준 error code:

```text
SCHEMA_INVALID
PROVIDER_UNAVAILABLE
PROVIDER_AUTH_FAILED
WORKER_FAILED
REPORT_INVALID
POLICY_VIOLATION
APPROVAL_REQUIRED
BUDGET_EXCEEDED
TIMEOUT
SCOPE_VIOLATION
VALIDATION_FAILED
REVIEW_BLOCKED
```

---

## 17. Acceptance Criteria

- Core Engine이 provider-specific 코드를 직접 import하지 않는다.
- Provider Adapter 교체가 가능하다.
- 모든 상태 변화가 event log에 남는다.
- report schema invalid가 crash로 이어지지 않는다.
- fake provider로 전체 job이 실행된다.
