# Provider Model

Star-Control provider는 제품명이 아니라 실행 능력과 연결 방식으로 모델링한다.

## 계층

- Provider Manifest: 종류와 기본 실행 형태.
- Provider Instance: 사용자 또는 프로젝트별 구체 설정.
- Transport: CLI, HTTP, process, manual.
- Adapter: WorkSpec과 provider 입출력 변환.
- Capability Profile: router가 provider를 선택할 때 쓰는 능력 선언.
- Provider Registry: builtin provider manifest와 capability profile의 색인.

## 구현 기준

구현자는 provider 관련 작업 전에 아래 문서를 함께 확인한다.

```text
docs/implementation/provider-system.md
docs/providers/provider-registry.md
docs/providers/provider-reference-snapshots.md
specs/schemas/provider-manifest.schema.json
specs/schemas/provider-instance.schema.json
specs/schemas/capability-profile.schema.json
specs/schemas/provider-registry.schema.json
specs/schemas/provider-run-result.schema.json
```

## 외부 자료 기준

`provider-reference-snapshots.md`는 builtin provider manifest에 적힌 capability가 어떤 공식 자료에 근거하는지 추적한다. Adapter 구현 전에는 해당 provider의 최신 공식 문서를 다시 확인해야 한다.
