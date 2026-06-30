# Star-Control 구현 문서 읽는 순서

이 디렉터리는 Star-Control 전체 구현을 위한 정본 구현 문서다. 목표는 축소판 데모가 아니라 장시간 Codex 작업으로 전체 시스템을 완성할 수 있도록 구현 경계, 데이터 계약, 실행 흐름, 산출물 위치를 고정하는 것이다.

## 현재 착수 큐 우선순위

실제 구현 착수 순서는 `codex-work-queue-current.md`를 우선한다. `codex-work-queue.md`는 장기 backlog이며, 두 문서가 다르게 보이면 현재 착수 큐인 `codex-work-queue-current.md`를 기준으로 한다.

구현자는 package 경계와 장기 구조를 이해할 때는 `repository-layout.md`를 보되, 지금 당장 어느 EPIC/TASK를 시작할지는 `codex-work-queue-current.md`에서 확인한다.

## 읽는 순서

Codex 또는 다른 구현자는 아래 순서로 문서를 읽는다.

1. `target-architecture.md`
2. `current-repository-map.md`
3. `repository-layout.md`
4. `data-contracts.md`
5. `run-lifecycle.md`
6. `artifact-layout.md`
7. `artifact-naming.md`
8. `state-store.md`
9. `state-store-recovery.md`
10. `schema-validator.md`
11. `provider-system.md`
12. `config-system.md`
13. `router-decision-matrix.md`
14. `router-engine.md`
15. `execution-engine.md`
16. `validation-engine.md`
17. `star-sentinel-p0-contracts.md`
18. `star-sentinel-full-spec.md`
19. `approval-review-flow.md`
20. `policy-profiles.md`
21. `cli-command-reference.md`
22. `cli-daemon-api-ui.md`
23. `security-cost-observability.md`
24. `security-privacy-observability-contracts.md`
25. `testing-ci-release.md`
26. `ci-contract-validation.md`
27. `codex-long-run-workflow.md`
28. `codex-work-queue-current.md`
29. `codex-work-queue.md`
30. `codex-pr-template.md`
31. `codex-validation-report.md`

이 문서 세트는 Codex 장시간 목표추진 구현의 기준점이다. Codex는 구현 작업 전에 이 문서와 `AGENTS.md`를 먼저 읽어야 한다.

## 경로 해석 기준

- `current-repository-map.md`는 현재 repository에 실제로 존재하는 경로의 상태와 의미를 설명한다.
- `repository-layout.md`는 목표 package 경계와 장기 구조를 설명한다.
- 현재 경로와 목표 경계가 다르게 보이면 `current-repository-map.md`로 현재 상태를 먼저 확인하고, 실제 구현은 `repository-layout.md`의 package 책임을 따른다.
- 구현 순서가 다르게 보이면 `codex-work-queue-current.md`를 우선한다.

## artifact / StateStore 기준

- `artifact-layout.md`: `.ai-runs/` directory 구조.
- `artifact-naming.md`: 파일명, attempt, tmp, approval, review pack naming.
- `state-store.md`: StateStore API와 저장 규칙.
- `state-store-recovery.md`: 손상된 artifact와 event log 처리 기준.

## provider / config / router 기준

- `provider-system.md`: provider manifest, instance, capability, registry, adapter 경계.
- `config-system.md`: config, policy, hook, role, renderer, skill 계약과 병합 기준.
- `router-decision-matrix.md`: size, risk, policy profile, approval, decision matrix.
- `router-engine.md`: RouteSpec 생성 책임과 RouterEngine 구현 계약.

## Star Sentinel 기준

- `star-sentinel-p0-contracts.md`: P0 rule registry, ChangedLines input, fixture outcome 계약.
- `star-sentinel-full-spec.md`: Star Sentinel 전체 command, output, profile 확장 목표.

## Codex 작업 큐 기준

- `codex-work-queue-current.md`: 지금 실제로 착수할 EPIC/TASK 순서, 허용 파일, 금지 파일, 입력/출력 artifact, handoff 기준.
- `codex-work-queue.md`: 장기 backlog와 후속 확장 후보.
- `codex-long-run-workflow.md`: branch, PR, checkpoint, validation report 운영 규칙.

## 용어 수준

- `MUST`: 구현자가 반드시 지켜야 하는 계약.
- `SHOULD`: 기본 구현 방향. 명시 사유가 있으면 변경 가능.
- `MAY`: 확장 가능 영역.
- `RESERVED`: 장기 목표로 예약하지만 현재 구현하지 않는 영역.

## 현재 구현 원칙

- `main` 직접 수정 금지.
- 작업마다 새 브랜치와 PR을 사용한다.
- 의존성, package manager, 배포, 릴리즈, 외부 계정 변경은 명시 승인 전까지 하지 않는다.
- Star-Control repository에는 실행 산출물을 저장하지 않는다.
- 실행 산출물은 대상 프로젝트의 `.ai-runs/` 아래에 저장한다.
- core package 이름에는 특정 provider 제품명을 넣지 않는다.
- Star Sentinel은 builtin tool 경계 안에 두고 Star-Control core에 직접 결합하지 않는다.

## 구현 문서와 schema의 관계

- `specs/schemas/`와 `builtin-tools/star-sentinel/schemas/`는 기계 검증 가능한 계약이다.
- 이 디렉터리의 문서는 schema가 표현하지 못하는 책임 경계, 상태 전이, 저장 규칙, 운영 기준을 설명한다.
- schema와 문서가 충돌하면 schema를 먼저 보존하고, 문서를 schema에 맞게 수정하는 PR을 만든다.

## 구현 전제

현재 repository는 설계와 계약을 먼저 고정하는 단계다. 실제 app, daemon, UI, runtime package, package manager 도입은 별도 승인과 별도 PR에서 진행한다.
