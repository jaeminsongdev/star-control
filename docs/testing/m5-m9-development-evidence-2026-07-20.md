# P-0046~P-0050 M5~M9 제품 Slice 증거

## 범위

- M5: Git source manifest 기반 `ManagedRegistrySnapshot`과 exact `ConsumerMigrationPlan`
- M6: API·Schema·config·docs comparator와 non-mutating `CleanRoomDoctorReport`
- M7: normalized failure reproduction, dependency/security/supply-chain limitation과 `MaintenanceRadar`
- M8: checkpoint/resume/rollback migration, comparable p95 120% budget, ARM64 `native_unverified`
- M9: GoalId-bound `ChangeBundle`, DAG merge queue, owned logical worktree, remote reconcile, `merge.status`와 `handoff.get`

## 구현 위치

- 공개 계약: `crates/foundation/star-contracts/src/development.rs`
- pure engine: `crates/control/star-development/src/`
- durable projection: `apps/star-controller/src/coordination_store.rs`
- core handler·Schema: `apps/star-controller/src/main.rs`, `catalog/tool-packages/star-control-core.toml`, `catalog/tool-packages/schemas/`
- generated Schema·fixture: `specs/schemas/v1/`, `specs/fixtures/management/v1/`

## 판정 불변식

- source manifest가 canonical이며 derived snapshot이 source를 역으로 수정하지 않는다.
- missing/stale/partial/unverified external observation은 pass가 아니다.
- migration은 exact completed prefix에서만 resume하고 rollback은 checkpoint 역순이다.
- 서로 다른 performance binding은 보정해 pass로 만들지 않는다.
- ARM64 실기 부재는 `native_unverified`로 남긴다.
- publish timeout은 write 재시도 없이 read-only reconcile 한 번만 수행한다.
- ChangeBundle/Handoff는 GoalId와 immutable revision/fingerprint가 일치해야 저장·조회된다.

## 검증

- `cargo test --locked -p star-development -p star-controller -p star-contracts`
  - 결과: PASS
  - `star-development`: 11/11
  - Controller source registry: required core 17/17 handler+input/output Schema readiness PASS
- 공개 Schema는 `star-schema-gen`으로 재생성했고 minimal/full/invalid/future fixture 검증을 통과했다.
- 원격 write, push, PR, merge, publish는 실행하지 않았다.
