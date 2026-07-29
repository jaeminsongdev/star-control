# Terra Worker Report

```markdown
## Bundle

- bundle_id:
- goal_id:
- goal_status: active (WORKER_COMPLETE)|complete (SOL_APPROVED)|blocked
- worker_profile: gpt-5.6-terra/high

## 완료 기준

- [ ] 구현
- [ ] 직접 테스트
- [ ] 지정 검증

## 변경

- 변경 파일:
- 전체 diff 요약:
- shared contract 영향:

## 검증

- 명령:
- exit code:
- 결과:

## 인계

- Sol 전체 diff 리뷰 상태: pending
- 열린 위험:
- 필요한 승인 또는 blocker:
```

중간 진행을 완료 보고로 바꾸지 않는다. 최초 `WORKER_COMPLETE` 보고의 `goal_status`는 반드시 `active`다. `goal_status: complete`는 Sol 전체 diff 승인 뒤 controller의 완료 지시를 받고 Goal 도구에서도 완료 처리한 최종 closure 보고에만 사용한다.
