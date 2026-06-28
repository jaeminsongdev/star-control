> 흡수 출처: `star-control_design_v3/docs/25_Error_Taxonomy_and_Recovery.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 25. Error Taxonomy와 Recovery 설계

## 목적

AI worker는 실패한다. Provider CLI가 실패할 수도 있고, 모델 출력이 schema에 맞지 않을 수도 있고, 검증이 실패하거나 승인이 필요할 수도 있다. 이 문서는 Star-Control이 모든 실패를 같은 방식으로 기록하고 복구하기 위한 오류 분류 체계를 정의한다.

## 오류 분류

| 코드 | 의미 | 예시 | 기본 처리 |
|---|---|---|---|
| `PROVIDER_LAUNCH_FAILED` | Provider 실행 실패 | CLI 없음, profile 없음 | BLOCKED |
| `PROVIDER_TIMEOUT` | 실행 시간 초과 | worker가 멈춤 | RETRY or BLOCKED |
| `PROVIDER_RATE_LIMIT` | 한도/쿼터 제한 | API rate limit | RETRY_LATER |
| `MODEL_OUTPUT_INVALID` | schema 불일치 | JSON parse 실패 | REPAIR_ONCE |
| `WORKSPEC_VIOLATION` | 범위 밖 작업 | 금지 파일 수정 | BLOCK |
| `POLICY_DENIED` | 정책상 금지 | force push, 삭제 | BLOCK |
| `APPROVAL_REQUIRED` | 사용자 승인 필요 | 의존성 추가 | WAIT_APPROVAL |
| `VALIDATION_FAILED` | 검증 실패 | 테스트 실패 | FIX_LOOP |
| `REVIEW_BLOCKED` | 리뷰 차단 | BLOCKER 발견 | FIX_LOOP or STOP |
| `CONFLICT_DETECTED` | 파일 충돌 | 병렬 worker 충돌 | MANUAL_RESOLVE |
| `SECRET_EXPOSURE_RISK` | 비밀정보 위험 | API key 출력 | BLOCK |
| `BUDGET_EXCEEDED` | 예산 초과 | 토큰/시간 초과 | STOP |
| `UNKNOWN_ERROR` | 미분류 오류 | 예외 | BLOCKED |

## 복구 정책

```yaml
# policies/retry-policy.yaml
retry:
  PROVIDER_TIMEOUT:
    max_attempts: 1
    backoff_seconds: 30
  PROVIDER_RATE_LIMIT:
    max_attempts: 2
    backoff_seconds: 300
  MODEL_OUTPUT_INVALID:
    max_attempts: 1
    repair_prompt: true
  VALIDATION_FAILED:
    max_attempts: 2
    strategy: fix_loop
  REVIEW_BLOCKED:
    max_attempts: 1
    strategy: fix_loop

no_retry:
  - POLICY_DENIED
  - SECRET_EXPOSURE_RISK
  - WORKSPEC_VIOLATION
  - BUDGET_EXCEEDED
```

## ReportSpec 오류 필드

```json
{
  "status": "FAILED",
  "error": {
    "code": "VALIDATION_FAILED",
    "message": "cargo test --all failed",
    "stage": "validate",
    "retryable": true,
    "evidence": [
      {
        "type": "command",
        "command": "cargo test --all",
        "summary": "2 tests failed in stopwatch module"
      }
    ]
  }
}
```

## 복구 흐름

```text
worker 실행
  ↓
report.json 수집
  ↓
오류 코드 분류
  ↓
retry-policy 적용
  ↓
재시도 가능 → repair WorkSpec 생성
  ↓
재시도 불가 → BLOCKED/FAILED 상태 기록
  ↓
사용자 승인 필요 → approval request 생성
```

## Repair WorkSpec

실패 수정을 위한 WorkSpec은 원래 WorkSpec보다 더 좁아야 한다.

```yaml
stage: fix-validation
allowed_scope:
  - src/stopwatch/**
  - tests/stopwatch/**
forbidden_actions:
  - 테스트 삭제
  - 테스트 skip
  - 새 의존성 추가
context_pack:
  failed_command: cargo test --all
  failure_summary: stopwatch reset test failed
```

## 구현 체크리스트

- [ ] 모든 ProviderAdapter가 공통 error code로 변환한다.
- [ ] stdout/stderr 원문 전체가 아니라 summary와 artifact path를 저장한다.
- [ ] retry 횟수는 RunState에 누적한다.
- [ ] repair WorkSpec은 자동으로 범위를 좁힌다.
- [ ] `UNKNOWN_ERROR`는 final report에서 숨기지 않는다.
