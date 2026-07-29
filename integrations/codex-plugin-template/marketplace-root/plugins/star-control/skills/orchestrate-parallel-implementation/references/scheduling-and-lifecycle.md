# 스케줄링과 생명주기

## 상태 기계

```text
PLANNED -> READY -> DISPATCHED -> GOAL_ACTIVE -> WORKER_COMPLETE
                                            -> BLOCKED
                                            -> SUPERSEDED_PENDING_GOAL_RESOLUTION
WORKER_COMPLETE -> SOL_REVIEW -> CORRECTION -> GOAL_ACTIVE
SOL_REVIEW -> GOAL_COMPLETE -> INTEGRATED
INTEGRATED(all) -> SOL_FINAL_REVIEW -> FINAL_VALIDATION -> VERIFIED
```

- `READY`: 의존성과 소유권이 해결됨
- `GOAL_ACTIVE`: Terra가 Bundle 전체를 `create_goal`로 등록하고 추진 중
- `WORKER_COMPLETE`: Terra가 구현·자체 검증을 끝냈지만 Goal은 Sol 리뷰를 위해 active로 유지됨
- `SOL_REVIEW`: Sol이 해당 Terra 작업자의 전체 diff를 검토 중
- `CORRECTION`: Sol 지적을 같은 Terra 작업자와 같은 목표로 교정 중
- `GOAL_COMPLETE`: Sol 승인 뒤 같은 Terra가 활성 Goal을 `update_goal(status="complete")`로 종료함
- `INTEGRATED`: Sol 전체 diff 승인과 Terra Goal 완료를 모두 확인한 Bundle
- `VERIFIED`: 결합 전체 리뷰와 최종 프로젝트 검증까지 성공
- `SUPERSEDED_PENDING_GOAL_RESOLUTION`: 사용자 scope 변경으로 원 목표가 무효화됐지만 Goal API에 cancel/replace가 없어 변경을 격리하고 소유권 재사용을 막은 상태

## Goal Pursuit 규칙

Terra 작업자는 dispatch 직후 다음 의미의 호출을 한다.

```text
create_goal({ objective: "<Bundle objective와 모든 completion criteria>" })
```

- 임의의 `token_budget`을 설정하지 않는다.
- 중간 턴 종료, 일부 파일 변경, 자체 테스트 일부 성공은 목표 완료가 아니다.
- 교정은 `followup_task`로 동일 작업자에게 보내며 같은 활성 목표를 유지한다.
- 구현·직접 검증 기준을 충족하면 Goal을 active로 유지한 채 `WORKER_COMPLETE`를 보고한다.
- Sol 전체 diff 승인 전에는 `update_goal({status:"complete"})`을 호출하지 않는다.
- Sol 승인 뒤 controller가 `followup_task`로 완료를 지시한 경우에만 같은 Terra가 `update_goal({status:"complete"})`을 호출한다.
- 같은 blocker가 Goal 도구의 규정상 충분히 반복되고 더 진행할 수 없을 때만 `blocked`를 사용한다.

## 탄력적 작업자 풀

- 고정 lane 수를 정의하지 않는다.
- ready 집합에서 충돌 없는 Bundle을 현재 agent capacity만큼 dispatch한다.
- `wait_agent`로 완료나 주의 필요 상태를 bounded하게 기다린다.
- 완료된 슬롯이 생기면 다음 ready Bundle을 즉시 dispatch한다.
- 단순 진행 확인 때문에 실행 중인 작업자를 interrupt하지 않는다.
- 범위 안 질문에는 `send_message`, 새 실행 턴이 필요한 교정에는 `followup_task`를 사용한다.
- agent spawn 실패는 Bundle을 `READY`에 보존하고 capacity 회복 뒤 재시도한다. 정확한 Terra High profile 부재는 재시도 가능한 capacity 부족과 구분해 `BLOCKED`로 보고한다.

## Sol 리뷰 루프

1. Terra 보고와 실제 전체 diff를 모은다.
2. Sol에 원 수용 기준, Bundle 계약, 전체 diff, 테스트 결과를 제공한다.
3. Sol이 승인하면 같은 Terra에게 `followup_task`로 활성 Goal 완료를 지시한다. Goal 도구의 complete 상태를 확인한 뒤 `INTEGRATED`로 전이한다.
4. 지적이 있으면 같은 Terra에게 교정시키고 1단계부터 반복한다.
5. 전 Bundle 통합 후 결합 전체 diff를 Sol이 다시 검토한다.
6. 최종 프로젝트 검증 성공 후에만 `VERIFIED`로 전이한다.
