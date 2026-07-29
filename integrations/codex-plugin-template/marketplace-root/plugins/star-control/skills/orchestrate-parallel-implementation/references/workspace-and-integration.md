# 작업공간과 통합

## 공유 worktree

다음 조건을 모두 만족하면 공유 worktree를 사용한다.

- Bundle 소유 파일과 생성물이 겹치지 않는다.
- 한 작업자의 빌드·formatter가 다른 작업자의 파일을 대량 변경하지 않는다.
- 현재 dirty 변경을 정확히 식별하고 보존할 수 있다.
- 검증 명령이 병렬 실행되어도 동일 cache나 port를 파괴하지 않는다.

각 작업자는 자신의 소유 범위만 수정한다. 다른 작업자 변경을 reset, restore, clean, stash하지 않는다.

## 격리 worktree

다음 중 하나면 별도 worktree 또는 동등한 격리를 설계한다.

- 공통 formatter, codegen, lockfile, build output 충돌 가능성
- 동일 이름 port/service 또는 전역 cache 사용
- 대규모 refactor가 넓은 파일 집합을 건드림
- 현재 checkout의 dirty 상태와 안전하게 분리할 필요

기존 linked worktree를 폐기 가능한 cache로 보지 않는다. 필요한 격리 worktree의 생성·삭제·이동이 승인 경계인데 승인이 없으면 해당 Bundle을 `BLOCKED`로 두고 shared worktree로 강등하지 않는다.

## 소유권과 통합

- 파일뿐 아니라 public API, Schema, DB migration sequence, port, fixture namespace, build output도 단일 소유자로 지정한다.
- shared contract 변경은 선행 Bundle로 만들고 consumer는 그 결과에 의존시킨다.
- Terra는 다른 Bundle의 shared contract를 직접 고치지 않고 필요성을 보고한다.
- Sol 전체 diff 승인 전에는 commit 후보나 통합 완료로 간주하지 않는다.
- 통합 뒤 실제 combined diff, Git 상태, 생성 파일을 다시 확인한다.
- commit에는 요청 범위 파일만 stage한다. push, PR, publish는 사용자 승인 없이는 실행하지 않는다.
- Context Pack에는 baseline revision, dispatch 전 pre-existing dirty paths, Bundle owned paths를 기록해 기존 변경과 Terra 변경을 구분한다.
