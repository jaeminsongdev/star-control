# P-0044 M3 Gate·Goal/Plan/Run 구현 증거

## 구현 범위

- ready `ValidationPlan` v2의 exact CheckGraph를 위상 순서로 실행하고 cycle은 executor side effect 전에 거부한다.
- complete·stable·expected exit·observed tool을 모두 만족한 run만 required Check를 충족한다.
- timeout, cancel, partial, unverified, flaky, dependency `not_run`은 `AUTO_PASS`가 될 수 없다.
- ValidationRun·Diagnostic·GateDecision·EvidenceBundle을 한 Project SQLite transaction으로 저장하며 identity conflict는 기존 evidence를 덮어쓰지 않는다.
- Goal/Plan/Run 상태는 `%LOCALAPPDATA%/Star-Control/state/goals.v1.json`의 protected derived state로 저장한다. creation replay, optimistic revision, crash-safe reload와 future-version rejection을 검증한다.
- required core 17개 중 위 9개 Goal/Plan/Run과 기존 6개를 합친 15개만 current source에서 ready다. M9 소유 `merge.status`, `handoff.get`은 이 단계에서 ready로 올리지 않는다.

## 검증 행렬

| 영역 | 실제 검증 |
|---|---|
| runner success | complete stable graph만 `auto_pass` |
| failure truth | timeout·partial·flaky가 dependent `not_run`과 blocking Diagnostic 생성 |
| graph safety | cycle을 첫 executor 호출 전에 거부 |
| review | 모든 run pass여도 required review면 `human_review` |
| persistence | application E2E가 저장 뒤 같은 EvidenceBundle을 재조회 |
| Goal lifecycle | start replay/conflict, question answer, plan update, continue, pause/resume/cancel, stale revision |
| durability | atomic reload와 future format rejection |
| Schema | v2 5종과 GoalRecord generated Schema의 minimal/full/invalid/future fixture |

workspace Gate의 immutable report path는 `PLANS.md`의 P-0044 evidence 항목이 소유한다.
