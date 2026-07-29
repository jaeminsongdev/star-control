---
name: orchestrate-parallel-implementation
description: Orchestrate software implementation with maximal safe parallelism by default. Use whenever the user asks to implement, fix, refactor, migrate, or add tests, unless the user explicitly requests single-agent work. Make a Sol Max authority design the implementation and review every complete Terra High diff, while Terra High workers pursue one cohesive bundle goal through implementation, correction, and validation.
---

# Orchestrate Parallel Implementation

요청된 구현을 `Sol Max 중앙 관제 + Terra High 목표추진 작업자 풀 + 응집된 Task Bundle 기반 최대 fan-out`으로 끝까지 수행한다. 숫자로 고정한 lane 수나 파일 단위 미세 분할을 두지 않는다.

## 적용 경계

- 구현, 수정, 리팩터링, 마이그레이션, 테스트 추가 요청에는 기본 적용한다.
- 사용자가 단일 에이전트 작업을 명시하면 Sol authority와 Terra worker를 포함한 모든 subagent spawn을 중지한다. 현재 단일 에이전트가 구현과 프로젝트 검증을 수행하고 역할 분리 미적용을 최종 보고한다.
- 설명, 조사, 리뷰만 요청된 경우에는 구현 작업자를 만들지 않는다.
- 중앙 작업 자체를 `create_goal`로 등록하지 않는다. 목표추진은 이 스킬이 배정하는 각 Terra 구현 Bundle에만 적용한다.
- 저장소 지침, 승인 경계, dirty worktree, 사용자가 지정한 모델 또는 실행 제한을 우선한다.

## 권한 분리

1. `Sol Max`는 요구 해석, 구현 설계, 의존 DAG, Bundle 경계, 소유권, 재계획, 개별 전체 diff 리뷰, 최종 통합 diff 리뷰를 소유한다.
2. `Terra High`는 배정받은 Bundle의 구현, 테스트, 자체 검증, Sol 리뷰 지적 교정만 수행한다.
3. Sol은 제품 구현 코드를 대신 작성하지 않는다. Terra는 계약·범위·소유권을 독단적으로 바꾸지 않는다.
4. Terra의 `WORKER_COMPLETE`만으로 통합하지 않는다. Sol이 해당 작업자의 전체 diff를 검토하고 승인해야 `INTEGRATED`로 전이한다.
5. 모든 Bundle이 통합된 뒤에도 Sol이 결합된 전체 diff를 한 번 더 검토해야 최종 검증을 시작한다.

현재 중앙 에이전트가 `gpt-5.6-sol`의 `max` 추론으로 확인되지 않으면, `spawn_agent`에 `fork_turns: "none"`, `model: "gpt-5.6-sol"`, `reasoning_effort: "max"`를 지정해 전용 설계·리뷰 권한을 만든다. 중앙 에이전트는 조정만 계속하고 설계·리뷰 판단은 이 권한에서 받아야 한다. 모델 프로필을 실제로 사용할 수 없으면 다른 모델을 조용히 대체하거나 구현을 dispatch하지 말고 controller-level `BLOCKED`로 보고한다.

## 실행 절차

### 1. 제약과 정본 확인

- 사용자 요청, 관련 `AGENTS.md`, 선택된 Skill, 현재 plan/ledger, Git 상태, 검증 진입점을 확인한다.
- 기존 dirty 파일과 생성 상태를 사용자 작업으로 취급한다.
- 구현 전에 승인 필요 작업과 외부 효과를 표시한다.

### 2. Sol 설계 확보

Sol 권한에 원 목표와 필요한 저장소 근거를 전달하고 다음 산출물을 요청한다.

- 구현 경계와 수용 기준
- 계약 우선 순서와 의존 DAG
- 응집된 Task Bundle과 각 Bundle의 단독 완료 기준
- 파일, API, Schema, DB, port, build output 소유권
- 공유 worktree 또는 격리 worktree 선택
- Bundle별 검증과 최종 검증

분할 규칙은 [decomposition.md](references/decomposition.md)를 따른다. 파일 하나를 읽기·수정·테스트로 쪼개거나, 서로 같은 파일과 같은 계약을 바꾸는 작업을 억지로 병렬화하지 않는다.

### 3. 준비된 Bundle 최대 fan-out

의존성이 해소되고 소유권이 충돌하지 않는 모든 Bundle을 현재 시스템이 허용하는 만큼 즉시 배정한다. 스킬 자체의 숫자 lane 제한은 두지 않는다. 실제 capacity 또는 backpressure가 생기면 완료된 자리를 확인해 다음 ready Bundle로 즉시 refill한다.

각 구현 작업자는 반드시 다음 설정으로 만든다.

```text
spawn_agent(
  fork_turns="none",
  model="gpt-5.6-terra",
  reasoning_effort="high",
  message=<완전한 Context Pack>,
)
```

Terra 작업자에게 subagent 생성 권한을 주지 않는다. 프로필을 사용할 수 없으면 조용히 다른 모델로 실행하지 말고 해당 Bundle을 `BLOCKED`로 유지한다.

Context Pack은 [worker-context-pack.md](assets/worker-context-pack.md)를 사용하며 반드시 `goal_pursuit: required`와 Bundle 전체 목표·완료 기준을 담는다. 완료 기준에는 Sol의 해당 작업자 전체 diff 승인을 포함한다. 작업자는 첫 동작으로 토큰 예산 없이 `create_goal`을 호출한다. 구현·직접 검증을 끝내면 Goal을 active로 유지한 채 `WORKER_COMPLETE`를 보고하며, Sol 승인 전에는 `update_goal(status="complete")`을 호출하지 않는다.

### 4. 감독, 교정, 재계획

[scheduling-and-lifecycle.md](references/scheduling-and-lifecycle.md)의 상태 기계를 사용한다.

- 짧은 진행 보고는 상태 증거일 뿐 Bundle 종료가 아니다.
- 범위 안 교정은 `followup_task`로 같은 Terra 작업자와 같은 활성 목표에 돌려보낸다.
- 새 독립 범위일 때만 새 Bundle과 새 목표를 만든다.
- 공유 계약 변경 필요를 발견한 작업자는 임의 수정하지 않고 controller에 보고한다. Sol이 DAG와 소유권을 재설계한 뒤 재배정한다.
- 사용자 범위 변경이 오면 신규 dispatch를 멈추고 active Terra에 안전 경계 정지를 알린 뒤 영향을 계산하며, 필요하면 `interrupt_agent`로 추가 mutation을 막는다. Sol이 DAG를 다시 설계하고 이미 끝난 유효 작업을 버리지 않는다.
- 변경된 범위가 기존 Bundle 목표 안의 교정이면 같은 Terra·같은 Goal에 갱신된 Context Pack을 보낸다. 원 목표 자체가 무효화되면 완료나 blocked 상태를 거짓 기록하지 않고 변경을 격리한 `SUPERSEDED_PENDING_GOAL_RESOLUTION`로 남긴다. 현재 Goal API에는 cancel/replace가 없음을 보고하고 소유권이 겹치는 후속 dispatch를 막는다.

### 5. Sol 전체 리뷰와 통합

각 Terra 작업자가 active Goal 상태로 `WORKER_COMPLETE`를 보고하면 [worker-report.md](assets/worker-report.md)에 맞춰 변경 파일, 전체 diff 기준 설명, 검증 결과, 열린 위험을 받는다. Sol 권한에 해당 작업자가 만든 전체 diff와 관련 계약·테스트를 모두 제공해 다음을 검토시킨다.

- 요구와 완료 기준 충족
- 범위·소유권 위반과 다른 작업자 변경 침범
- 결함, 회귀, 보안·데이터 위험
- 테스트의 실효성과 검증 누락
- shared contract 및 통합 영향

Sol 승인 전에는 Goal을 complete로 만들거나 `INTEGRATED`로 표시하지 않는다. 지적이 있으면 같은 Terra 작업자의 같은 활성 목표에 교정시키고 전체 diff를 다시 Sol에게 검토시킨다. Sol이 승인하면 `followup_task`로 같은 Terra에게 Goal 완료를 지시하고, 실제 `update_goal(status="complete")` 확인 뒤 `INTEGRATED`로 전이한다.

모든 Bundle 승인 후 Sol이 결합된 전체 diff와 작업자 간 상호작용을 다시 검토한다. 그 승인이 끝난 뒤에만 프로젝트 검증을 실행한다.

### 6. 최종 검증과 보고

[safety-and-validation.md](references/safety-and-validation.md)와 프로젝트 검증 Skill을 따른다. 필요한 format, lint, unit, integration, build, smoke를 실제로 실행하고 결과를 숨기거나 약화하지 않는다. 최종 검증 성공 전에는 `VERIFIED`를 선언하지 않는다.

[controller-report.md](assets/controller-report.md) 형식으로 다음을 보고한다.

- Sol 설계·개별 전체 리뷰·결합 전체 리뷰 상태
- Terra Bundle별 목표 ID/상태, 변경, 검증
- 통합 및 최종 검증 결과
- 승인 대기, 외부 blocker, 잔여 위험

workspace와 commit 통합은 [workspace-and-integration.md](references/workspace-and-integration.md)를 따른다.

## 금지 사항

- Sol 대신 Terra가 구현 설계를 확정하게 하지 않는다.
- Sol이 Terra 대신 제품 구현을 수행하게 하지 않는다.
- 목표추진 없이 Terra 구현 Bundle을 배정하지 않는다.
- Sol 전체 diff 승인 전에 Terra Goal을 완료하지 않는다.
- 일부 진행을 `WORKER_COMPLETE`, 개별 자체검증을 `INTEGRATED`, 최종 검증 전 상태를 `VERIFIED`로 승격하지 않는다.
- 동일 파일, 동일 public contract, 동일 DB migration, 동일 port 또는 동일 생성물을 여러 작업자에게 동시에 소유시키지 않는다.
- 승인 없이 dependency 설치, 파일 삭제, push, publish, deploy, 외부 계정 변경을 실행하지 않는다.
