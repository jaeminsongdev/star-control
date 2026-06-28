# Star-Control 구현 문서 읽는 순서

이 디렉터리는 Star-Control 전체 구현을 위한 정본 구현 문서다. 목표는 축소판 데모가 아니라 장시간 Codex 작업으로 전체 시스템을 완성할 수 있도록 구현 경계, 데이터 계약, 실행 흐름, 산출물 위치를 고정하는 것이다.

## 읽는 순서

Codex 또는 다른 구현자는 아래 순서로 문서를 읽는다.

1. `target-architecture.md`
2. `repository-layout.md`
3. `data-contracts.md`
4. `run-lifecycle.md`
5. `artifact-layout.md`
6. `state-store.md`
7. `schema-validator.md`
8. `provider-system.md`
9. `router-engine.md`
10. `execution-engine.md`

이 문서 세트는 이후 추가될 validation, Star Sentinel full spec, approval/review flow, CLI, daemon, UI, security, observability, Codex work queue 문서의 기준점이다.

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
