# Run Lifecycle

## 목적

이 문서는 Star-Control job의 상태 전이와 stage 전이를 정의한다. 구현자는 RunState를 임의 문자열로 확장하지 말고 schema와 이 문서의 의미를 함께 따른다.

## canonical job states

```text
REQUESTED
ROUTING
ROUTED
PLANNING
PLANNED
WAITING_APPROVAL
IMPLEMENTING
IMPLEMENTED
VALIDATING
VALIDATED
REVIEWING
REVIEWED
POLISHING
POLISHED
REPORTING
DONE
FAILED
BLOCKED
CANCELLED
```

## canonical stages

```text
route
plan
design
implement
validate
review
polish
report
```

## 기본 흐름

```text
REQUESTED
  -> ROUTING
  -> ROUTED
  -> PLANNING
  -> PLANNED
  -> IMPLEMENTING
  -> IMPLEMENTED
  -> VALIDATING
  -> VALIDATED
  -> REVIEWING
  -> REVIEWED
  -> POLISHING
  -> POLISHED
  -> REPORTING
  -> DONE
```

모든 job이 모든 stage를 반드시 지나야 하는 것은 아니다. RouteSpec의 `stages`가 실제 stage 목록을 결정한다.

## approval 흐름

approval이 필요한 경우 다음 상태를 사용한다.

```text
PLANNED -> WAITING_APPROVAL -> IMPLEMENTING
VALIDATING -> WAITING_APPROVAL -> REVIEWING
```

`WAITING_APPROVAL`은 자동 진행을 멈춘 상태다. 사용자 또는 human handoff provider가 승인해야 다음 상태로 이동한다.

## 실패 흐름

```text
* -> FAILED
* -> BLOCKED
* -> CANCELLED
```

- `FAILED`: 실행 오류, validation command 실패, provider error 등으로 작업이 실패한 상태.
- `BLOCKED`: policy, scope, approval gate가 자동 진행을 차단한 상태.
- `CANCELLED`: 사용자 또는 운영자가 중단한 상태.

## 상태별 의미

### REQUESTED

JobSpec이 생성되었지만 아직 route가 시작되지 않았다.

### ROUTING

RouterEngine이 request를 분석하고 RouteSpec을 생성하는 중이다.

### ROUTED

RouteSpec이 생성되었다.

### PLANNING

RouteSpec을 기반으로 stage별 WorkSpec을 만드는 중이다.

### PLANNED

필요한 WorkSpec이 생성되었다.

### WAITING_APPROVAL

사람 승인이 필요한 상태다. 이 상태에서는 provider 실행을 새로 시작하면 안 된다.

### IMPLEMENTING

구현 provider가 WorkSpec을 실행 중이다.

### IMPLEMENTED

구현 stage의 provider output이 저장되었다.

### VALIDATING

ValidationEngine 또는 Star Sentinel이 검증 중이다.

### VALIDATED

검증이 완료되었고 다음 단계로 진행 가능하다.

### REVIEWING

리뷰 provider 또는 Star Sentinel review pack 생성이 진행 중이다.

### REVIEWED

리뷰가 완료되었다.

### POLISHING

후처리, 정리, 문서 보강, formatting 등 polish 작업이 진행 중이다.

### POLISHED

polish 작업이 완료되었다.

### REPORTING

최종 ReportSpec과 사용자 보고를 생성 중이다.

### DONE

작업이 성공적으로 끝났다.

### FAILED

작업 실패가 확정되었다.

### BLOCKED

policy 또는 approval gate에 의해 자동 진행이 차단되었다.

### CANCELLED

작업이 취소되었다.

## event 기록 규칙

상태가 바뀔 때마다 `events.jsonl`에 LedgerEvent를 append한다.

권장 event type:

```text
TASK_CREATED
POLICY_CHECKED
VALIDATION_RECORDED
GATE_DECIDED
REVIEW_PACK_CREATED
ARTIFACT_WRITTEN
ERROR_RECORDED
```

## stage 진입 조건

- `route`: JobSpec이 존재해야 한다.
- `plan`: RouteSpec이 존재해야 한다.
- `implement`: WorkSpec이 존재해야 하고 approval block이 없어야 한다.
- `validate`: provider output이 존재해야 한다.
- `review`: validation result가 존재해야 한다.
- `polish`: review result가 존재해야 한다.
- `report`: report source artifact가 존재해야 한다.

## 금지 전이

- `BLOCKED`에서 자동으로 `IMPLEMENTING`으로 이동 금지.
- `WAITING_APPROVAL`에서 승인 artifact 없이 다음 stage로 이동 금지.
- `FAILED`에서 원인 기록 없이 `DONE`으로 변경 금지.
- `CANCELLED`에서 provider 실행 재개 금지.

## resume 기준

resume은 다음 조건을 만족할 때만 가능하다.

1. 마지막 RunState가 terminal state가 아니다.
2. 필요한 artifact가 존재한다.
3. events.jsonl이 읽을 수 있다.
4. approval이 필요한 상태라면 approval artifact가 존재한다.

## terminal state

```text
DONE
FAILED
BLOCKED
CANCELLED
```

terminal state에서는 새 provider execution을 시작하지 않는다. 별도 job을 만들거나 명시적 resume 정책을 정의해야 한다.
