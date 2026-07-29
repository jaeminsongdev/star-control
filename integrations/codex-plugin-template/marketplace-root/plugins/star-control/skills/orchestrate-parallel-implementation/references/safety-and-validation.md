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

1. Terra는 Bundle에 직접 연결된 format, lint, unit, integration, build 또는 smoke를 실행한다.
2. Sol은 테스트가 요구를 실제로 증명하는지 전체 diff와 함께 검토한다.
3. 모든 Bundle 통합 후 Sol이 결합 전체 diff를 검토한다.
4. 중앙 controller가 프로젝트 정본 검증 진입점을 실행한다.
5. 실패, 미실행, stale evidence를 pass로 바꾸지 않는다.

## 필수 forward scenario

스킬 또는 제품 계약을 바꿀 때 다음을 검증한다.

1. 독립된 5개 모듈을 충돌 없는 ready Bundle로 fan-out한다.
2. 같은 파일을 바꾸는 작업은 한 Bundle로 묶는다.
3. 공통 계약을 먼저 구현하고 consumer Bundle을 후속 dispatch한다.
4. 읽기·수정·테스트를 microtask로 분리하지 않는다.
5. capacity backpressure 뒤 완료 슬롯에 ready Bundle을 refill한다.
6. 기존 dirty worktree를 reset·clean·restore하지 않는다.
7. Terra가 shared contract 변경 필요를 보고하고 임의 수정하지 않는다.
8. `WORKER_COMPLETE` 뒤에도 Goal을 active로 유지하고 Sol 교정을 같은 Terra 작업자와 같은 활성 목표에 돌려보낸다.
9. 사용자가 single-agent를 명시하면 병렬화를 비활성화한다.
10. 승인 없이 dependency 설치, 삭제, push를 실행하지 않는다.
11. 사용자 scope 변경 시 유효 작업을 보존하고 Sol이 안전하게 재계획한다.
12. Sol 결합 리뷰와 최종 검증 전에 `VERIFIED`를 선언하지 않는다.

## 완료 증거

최종 보고에는 명령, exit code, 핵심 결과, 적용 revision/worktree, 미실행 gate와 잔여 위험을 구분해 남긴다. 과거 artifact나 worker 자체 주장만으로 현재 source를 검증 완료로 승격하지 않는다.
