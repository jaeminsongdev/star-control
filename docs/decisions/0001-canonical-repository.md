# 0001. Canonical Repository Structure

## Decision

Star-Control의 설계 기준은 이 repository 안의 정본 문서, schema, config, provider manifest, tool manifest로 유지한다.

## Canonical Areas

- `README.md`: repository 목적과 현재 상태.
- `docs/00_개요.md`, `docs/01_아키텍처.md`, `docs/02_구현로드맵.md`: 읽기 시작점.
- `docs/providers/`, `docs/operations/`, `docs/tools/`: 운영 및 구현 세부 문서.
- `specs/schemas/`: JSON schema.
- `specs/contracts/`: adapter, transport, capability, quality gate 계약.
- `configs/`: 정책, 역할, hook, template, registry.
- `builtin-providers/`: provider manifest와 capability profile.
- `builtin-tools/star-sentinel/`: Star Sentinel manifest, policy, schema, template, corpus.

## Consequences

- 장기 유지 문서는 현재 구조의 의미와 계약만 설명한다.
- 추적용 보조 산출물, 임시 조사표, 복사본 문서는 정본 tree에 보관하지 않는다.
- 구현 전 단계에서는 의존성, package manager, daemon, UI를 추가하지 않는다.
- 실행 결과는 대상 프로젝트 `.ai-runs/` 아래에 저장한다.
