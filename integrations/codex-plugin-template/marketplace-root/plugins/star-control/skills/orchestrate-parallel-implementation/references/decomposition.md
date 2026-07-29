# 분해와 의존 DAG

## Task Bundle 계약

각 Bundle은 한 작업자가 별도 질문 없이 끝낼 수 있는 응집된 수직 단위다. 다음 필드를 고정한다.

- `bundle_id`: 안정적인 식별자
- `objective`: 사용자 가치 또는 기술 결과 하나
- `scope_in` / `scope_out`: 포함·제외 경계
- `depends_on`: 선행 Bundle ID
- `ownership`: 파일, directory, API, Schema, DB migration, port, build output
- `completion_criteria`: 구현·테스트·검증을 포함한 닫힌 체크 목록
- `validation`: 작업자 로컬 명령과 기대 결과
- `workspace_mode`: `shared` 또는 `isolated`
- `approval_boundary`: 실행 전 승인 필요한 효과
- `goal_pursuit`: 항상 `required`

## 분해 순서

1. 원 요청을 관찰 가능한 수용 기준으로 바꾼다.
2. public contract, Schema, migration, 공통 생성물처럼 downstream을 여는 선행 경계를 찾는다.
3. 선행 경계를 먼저 한 Bundle로 묶고 consumer Bundle을 DAG 후속으로 둔다.
4. 같은 파일이나 같은 의미적 계약을 바꾸는 작업은 한 Bundle로 합친다.
5. 독립된 모듈·adapter·fixture·플랫폼 구현은 소유권이 분리될 때 병렬 Bundle로 둔다.
6. 각 Bundle에 구현과 그 구현의 직접 테스트를 함께 둔다.
7. 준비된 Bundle을 모두 dispatch하고 완료 시 새 ready Bundle로 refill한다.

선행 Bundle에 의존하는 후속 Bundle은 선행 구현자의 자체 완료가 아니라 Sol 전체 diff 승인과 Goal complete를 거쳐 `INTEGRATED`가 된 뒤에만 `READY`로 전이한다.

## 크기 기준

좋은 Bundle은 구현, 직접 테스트, 자체 검증까지 한 작업자가 계속 소유한다. 다음은 지나치게 작다.

- 파일 읽기만 하는 작업
- 함수 하나 작성과 그 테스트를 서로 다른 작업자에게 배정
- 같은 파일의 import, 로직, 테스트를 따로 배정
- 보고서 작성만 떼어내 구현 완료로 간주

다음은 지나치게 크다.

- 서로 독립인 다섯 모듈을 한 작업자에게 몰아줌
- 공통 계약과 모든 consumer를 의존 순서 없이 한 Bundle에 넣음
- 승인 필요 운영 효과와 되돌릴 수 있는 source 구현을 함께 묶음

## 재계획

작업 중 shared contract 필요, 소유권 충돌, 사용자 범위 변경을 발견하면 신규 dispatch를 멈추고 active worker의 추가 mutation을 안전 경계에서 중지한다. Sol이 이미 유효한 결과를 보존한 채 DAG를 재작성하고, 기존 목표 안의 교정인 Bundle만 완료 기준과 Context Pack을 갱신해 같은 Goal로 계속한다. 원 목표가 무효화된 Bundle은 변경을 격리하고 `SUPERSEDED_PENDING_GOAL_RESOLUTION`로 남겨 Goal cancel/replace 지원이나 사용자 결정을 기다린다. Terra가 임의로 scope를 확장하지 않는다.
