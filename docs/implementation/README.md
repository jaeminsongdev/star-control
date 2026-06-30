# Star-Control 구현 문서 읽는 순서

이 디렉터리는 Star-Control 전체 구현을 위한 정본 구현 문서다. 목표는 장시간 Codex 작업으로 전체 시스템을 완성할 수 있도록 구현 경계, 데이터 계약, 실행 흐름, 산출물 위치를 고정하는 것이다.

## 현재 착수 큐 우선순위

실제 구현 착수 순서는 `codex-work-queue-current.md`를 우선한다. `codex-work-queue.md`는 장기 backlog이며, 두 문서가 다르게 보이면 현재 착수 큐인 `codex-work-queue-current.md`를 기준으로 한다.

구현자는 package 경계와 장기 구조를 이해할 때는 `repository-layout.md`를 보고, runtime stack 결정은 `docs/decisions/0002-runtime-stack.md`를 따른다. 지금 당장 어느 EPIC/TASK를 시작할지는 `codex-work-queue-current.md`와 해당 `briefs/E*.md`에서 확인한다.

## 빠른 착수 경로

Codex가 실제 구현 EPIC에 들어갈 때는 아래 순서를 먼저 따른다.

```text
AGENTS.md
README.md
docs/implementation/README.md
docs/implementation/codex-long-run-workflow.md
docs/implementation/codex-work-queue-current.md
docs/implementation/briefs/README.md
docs/implementation/briefs/E*.md
```

brief는 착수 범위와 검증 명령을 줄여 보여주는 entrypoint다. 상세 계약은 기존 implementation 문서와 schema/example을 따른다.

## 읽는 순서

Codex 또는 다른 구현자는 아래 순서로 문서를 읽는다.

1. `target-architecture.md`
2. `current-repository-map.md`
3. `repository-layout.md`
4. `../decisions/0002-runtime-stack.md`
5. `../decisions/0003-fake-provider-instance.md`
6. `../decisions/0004-star-sentinel-p0-scope.md`
7. `data-contracts.md`
8. `handoff-vocabularies.md`
9. `run-lifecycle.md`
10. `artifact-layout.md`
11. `artifact-naming.md`
12. `state-store.md`
13. `state-store-recovery.md`
14. `schema-validator.md`
15. `provider-system.md`
16. `config-system.md`
17. `router-decision-matrix.md`
18. `router-engine.md`
19. `execution-engine.md`
20. `validation-engine.md`
21. `star-sentinel-p0-contracts.md`
22. `star-sentinel-p0-implementation-split.md`
23. `star-sentinel-full-spec.md`
24. `approval-review-flow.md`
25. `policy-profiles.md`
26. `cli-command-reference.md`
27. `cli-daemon-api-ui.md`
28. `security-cost-observability.md`
29. `security-privacy-observability-contracts.md`
30. `testing-ci-release.md`
31. `ci-contract-validation.md`
32. `codex-long-run-workflow.md`
33. `codex-work-queue-current.md`
34. `briefs/README.md`
35. `codex-work-queue.md`
36. `codex-pr-template.md`
37. `codex-validation-report.md`

## 경로 해석 기준

- `current-repository-map.md`는 현재 repository에 실제로 존재하는 경로의 상태와 의미를 설명한다.
- `repository-layout.md`는 목표 package 경계와 장기 구조를 설명한다.
- 현재 경로와 목표 경계가 다르게 보이면 `current-repository-map.md`로 현재 상태를 먼저 확인하고, 실제 구현은 `repository-layout.md`의 package 책임을 따른다.
- 구현 순서가 다르게 보이면 `codex-work-queue-current.md`를 우선한다.
- EPIC별 착수 범위는 `briefs/E*.md`를 함께 확인한다.

## runtime stack 기준

- `docs/decisions/0002-runtime-stack.md`: Star-Control v0 구현 언어를 Rust, package manager를 Cargo, workspace model을 Cargo workspace로 고정한다.
- 이 결정은 구현 언어와 package manager 미정 상태를 해소한다.
- 실제 Cargo workspace 파일, crate, lockfile은 구현 PR에서 추가한다.
- Cargo 외 package manager나 새 production dependency는 별도 승인이 필요하다.

## fake provider 기준

- `docs/decisions/0003-fake-provider-instance.md`: v0 fake flow의 provider instance id를 `fake-default`로 고정한다.
- RouteSpec assignment, WorkSpec, ExecutionRequest, ProviderRunResult, provider output path는 이 기준을 따른다.

## handoff / vocabulary 기준

- `handoff-vocabularies.md`: RouteSpec, RouterDecision, WorkSpec에서 공유하는 canonical `change_types`, `forbidden_actions`, handoff required field 기준.
- 새 vocabulary를 추가하거나 rename하면 관련 schema, example, 문서를 같은 PR에서 갱신한다.

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

- `docs/decisions/0004-star-sentinel-p0-scope.md`: v0 P0 rule 범위와 P1 후보를 고정한다.
- `star-sentinel-p0-contracts.md`: P0 rule registry, ChangedLines input, fixture outcome 계약.
- `star-sentinel-p0-implementation-split.md`: E09a~E09d 구현 분할 기준.
- `star-sentinel-full-spec.md`: Star Sentinel 전체 command, output, profile 확장 목표.

## Codex 작업 큐 기준

- `codex-work-queue-current.md`: 지금 실제로 착수할 EPIC/TASK 순서, 허용 파일, 금지 파일, 입력/출력 artifact, handoff 기준.
- `briefs/`: 각 EPIC의 착수용 요약 entrypoint.
- `codex-work-queue.md`: 장기 backlog와 후속 확장 후보.
- `codex-long-run-workflow.md`: branch, PR, checkpoint, validation report 운영 규칙.

## 구현 문서와 schema의 관계

- `specs/schemas/`와 `builtin-tools/star-sentinel/schemas/`는 기계 검증 가능한 계약이다.
- 이 디렉터리의 문서는 schema가 표현하지 못하는 책임 경계, 상태 전이, 저장 규칙, 운영 기준을 설명한다.
- schema와 문서가 충돌하면 schema를 먼저 보존하고, 문서를 schema에 맞게 수정하는 PR을 만든다.
