# Star-Control

Star-Control은 여러 AI coding agent, cloud API model, local model server, local process runner, fake provider, human handoff를 공통 규격으로 다루는 provider-neutral 작업 관제 시스템이다.

## 핵심 구성

- `docs/`: Star-Control 정본 설계와 운영 문서.
- `specs/`: JSON schema와 provider/tool 계약.
- `configs/`: 기본 설정, 정책, 역할, hook, template, registry.
- `packages/`: 구현 예정 package 경계. 현재는 스캐폴드만 둔다.
- `builtin-providers/`: 구체 provider manifest와 capability profile.
- `builtin-tools/star-sentinel/`: Star Sentinel 내장 도구 manifest, policy, schema, template, corpus.
- `examples/`: provider instance, sample project, sample run artifact.

## Provider 원칙

Star-Control은 provider를 이름이 아니라 다음 축으로 판단한다.

- provider kind
- transport
- adapter
- capability profile
- provider instance

구체 provider는 `builtin-providers/` 아래 manifest로 등록하고, core package 이름에는 특정 회사나 제품명을 넣지 않는다.

## Star Sentinel

Star Sentinel은 AI가 만든 변경사항을 diff, policy, evidence, validation 기반으로 검증하고 review pack과 approval gate를 생성하는 내장 도구다.

구현 코드는 `packages/star-sentinel/`, 등록정보와 정책은 `builtin-tools/star-sentinel/`에 둔다.

## 실행 결과 위치

Star-Control repository에는 실행 결과를 저장하지 않는다. 대상 프로젝트에 다음 형태로 저장한다.

```text
대상 프로젝트/.ai-runs/J-0001/provider-output/{provider-instance-id}/
대상 프로젝트/.ai-runs/J-0001/tool-output/star-sentinel/
```

## 현재 상태

이 repository의 `README.md`, `docs/`, `specs/`, `configs/`, `builtin-providers/`, `builtin-tools/star-sentinel/`가 Star-Control 설계 기준이다.

현재 단계는 스캐폴드와 설계 문서 상태다. 실제 앱, daemon, UI, package manager, runtime dependency는 아직 추가하지 않는다.
