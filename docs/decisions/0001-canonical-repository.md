# 0001. Canonical Repository Structure

## Decision

Star-Control의 설계 기준은 이 repository 안의 정본 문서, schema, config, provider manifest, tool manifest로 유지한다.

현재 실제 repository map은 `docs/implementation/current-repository-map.md`에 기록하고, 목표 package 경계는 `docs/implementation/repository-layout.md`에 기록한다.

## Canonical Areas

- `README.md`: repository 목적, 현재 상태, 첫 읽기 경로.
- `AGENTS.md`: 작업 경계와 검증 기준.
- `.github/workflows/`: 현재 CI 기준.
- `docs/00_개요.md`, `docs/01_아키텍처.md`, `docs/02_구현로드맵.md`: 읽기 시작점.
- `docs/implementation/`: 구현자가 따를 책임 경계, 데이터 계약, 실행 흐름, 검증 기준.
- `docs/providers/`, `docs/operations/`, `docs/tools/`: 운영 및 구현 세부 문서.
- `docs/decisions/`: 장기 결정 기록.
- `specs/schemas/`: JSON schema.
- `configs/`: 정책, 역할, hook, template, registry.
- `builtin-providers/`: provider manifest와 capability profile.
- `builtin-tools/star-sentinel/`: Star Sentinel manifest, policy, schema, template, corpus.
- `examples/`: schema와 문서 검증을 위한 sample artifact.
- `scripts/ci/`: repository 계약 검증 스크립트.

## Scaffold / Reserved Areas

- `apps/`: CLI, daemon, UI entrypoint 후보. 초기에는 scaffold 또는 reserved 영역이다.
- `packages/`: 목표 implementation package 경계. 실제 runtime package는 별도 승인 후 추가한다.
- `integrations/`: 장기 연동 산출물 후보. 실제 연동 작업은 별도 승인 후 처리한다.

## Consequences

- 장기 유지 문서는 현재 구조의 의미와 계약만 설명한다.
- 현재 경로 상태와 목표 경계가 다르게 보이면 `current-repository-map.md`로 현재 상태를 먼저 확인한다.
- 추적용 보조 산출물, 임시 조사표, 복사본 문서는 정본 tree에 보관하지 않는다.
- 구현 전 단계에서는 의존성, package manager, daemon, UI를 추가하지 않는다.
- 실행 결과는 대상 프로젝트 `.ai-runs/` 아래에 저장한다.
- `examples/runs/`는 실제 실행 결과가 아니라 검증용 sample로만 취급한다.
