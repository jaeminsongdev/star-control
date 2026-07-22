# M2 Planning 구현 증거 — 2026-07-20

## 판정

P-0043의 CLI-only `TaskSpec → ScopeRevision → ChangeSet → ImpactAnalysis → ready ValidationPlan` 수직 Slice를 구현했다. 이 판정은 계획 생성·조회까지이며 Check 실행이나 source mutation을 포함하지 않는다.

## 구현 표면

- `star-contracts::planning`: 공개 계약, versioned fingerprint, cross-reference seal
- `star-planning`: deterministic seed/graph traversal, certainty·limitation, risk path, affected scope promotion과 Check 선택
- `star-project::observe_workspace_changes`: Git staged·unstaged·untracked·rename/delete와 HEAD/current content identity의 bounded read-only 관찰
- `star-state`: global planning bundle idempotency/input/bundle fingerprint projection
- `star-application`: target Checkout·current index 재검증과 publish 전 TOCTOU probe
- `star-controller`·`star-cli`: bounded project-relative task JSON의 `planning create/get`

## 검증 행렬

| 항목 | 기대 결과 |
|---|---|
| 같은 task·actor·descriptor·idempotency key | 같은 persisted bundle replay |
| 같은 key, 다른 task input | `IdempotencyConflict` |
| optional semantic partition unavailable, required text current | planning input current 유지 |
| required snapshot stale/changed | publish 전 `IndexNotCurrent` |
| required Check descriptor 누락 | `ValidationPlan.readiness=blocked` |
| graph node/edge limit | complete·confirmed-empty 승격 없이 limitation/block |
| complete current fixture와 trusted descriptor | non-empty required Check의 `readiness=ready` |

실행 근거는 P-0043 TARGET/FULL report가 생성된 뒤 `PLANS.md`에 고정한다.
