# 안전과 검증

## 승인 경계

사용자의 명시적 승인 없이는 다음을 실행하지 않는다.

- package 또는 dependency 설치·추가·제거·uninstall
- system setting, PATH, 외부 계정 변경
- 파일 삭제나 대량 이동
- 원격 push, PR, publish, deploy
- 유료 서비스 또는 외부 업로드

승인 대기는 Bundle의 구현 완료와 분리해 보고한다. 요청하지 않은 인접 결함은 근거만 남기고 현재 scope에 섞지 않는다.

## 검증 계층

1. Terra는 자신의 project worktree에서 Bundle에 직접 연결된 format, lint, unit, integration, build 또는 smoke를 실행한다.
2. Sol은 worker의 `baseline_sha..head_sha` 전체 diff, fingerprint, 테스트가 요구를 실제로 증명하는지 함께 직접 검토한다.
3. 모든 Bundle 통합 후 Sol이 combined 전체 diff와 thread interaction을 직접 검토한다.
4. 중앙 controller가 프로젝트 정본 검증 진입점을 실행한다.
5. 실패, 미실행, stale evidence를 pass로 바꾸지 않는다.

## 필수 forward scenario

스킬 또는 제품 계약을 바꿀 때 다음을 검증한다.

1. 독립된 5개 모듈을 충돌 없는 ready Bundle과 별도 Terra project worktree로 fan-out한다.
2. 같은 파일을 바꾸는 작업은 하나의 Terra thread/Bundle로 묶는다.
3. 공통 계약을 먼저 구현하고 Sol 승인·Goal complete 뒤 consumer Bundle을 dispatch한다.
4. 읽기·수정·테스트를 microtask나 별도 thread로 분리하지 않는다.
5. capacity backpressure 뒤 완료 thread slot에 ready Bundle을 refill한다.
6. 기존 dirty worktree를 reset·clean·restore하지 않고, worker worktree의 baseline SHA를 고정한다.
7. Terra가 shared contract 변경 필요를 보고하고 임의 수정하지 않는다.
8. `WORKER_COMPLETE` 뒤에도 Goal을 active로 유지하고 Sol 교정을 같은 Terra thread와 같은 활성 Goal에 `send_message_to_thread`로 돌려보낸다.
9. 사용자가 single-agent를 명시하면 새 Sol/Terra thread와 project worktree 병렬화를 비활성화한다.
10. `clientThreadId`만 있는 setup 상태에서는 wait/read/message를 하지 않고, 확인된 thread_id/host_id 뒤 `wait_threads`와 `read_thread`를 사용한다.
11. 승인 없이 dependency 설치, 삭제, push를 실행하지 않으며 사용자 scope 변경 시 유효 작업을 보존하고 Sol이 안전하게 재계획한다.
12. Sol worker별 전체 diff 리뷰, combined 전체 diff 리뷰와 final validation 전에 `VERIFIED`를 선언하지 않는다.

## 완료 증거

최종 보고에는 command, exit code, 핵심 결과, 각 `thread_id`/`host_id`, absolute worktree root, baseline/head SHA, diff fingerprint, Goal 상태, 미실행 gate와 잔여 위험을 구분해 남긴다. 과거 artifact나 worker 자체 주장만으로 현재 source를 검증 완료로 승격하지 않는다.
