# E03 Artifact Layout Writer Brief

## 목표

StateStore 위에서 provider-output, tool-output, approvals, review-packs, tmp artifact path helper를 구현한다.

## 선행 문서

```text
docs/implementation/artifact-layout.md
docs/implementation/artifact-naming.md
docs/implementation/state-store.md
docs/implementation/security-cost-observability.md
```

## 수정 허용 파일

```text
packages/star-control-state/** 또는 선택된 artifact/path crate
관련 unit tests
필요한 최소 docs/example 업데이트
```

## 수정 금지 파일

```text
ProviderAdapter 실행 로직
RouterEngine 구현 파일
ExecutionEngine 구현 파일
ValidationEngine 구현 파일
CLI 구현 파일
local/cloud provider 구현 파일
```

## 핵심 작업

```text
artifact path resolver
provider-output directory resolver
tool-output directory resolver
approvals/review-packs/tmp writer helpers
relative ArtifactRef registry helper
path traversal guard
```

## 검증 명령

```text
python scripts/ci/run_all.py
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

## 완료 기준

모든 artifact path가 job directory 내부로 제한되어야 한다.

## handoff

E04/E05/E07이 사용할 provider-output path helper와 ArtifactRef 형식을 PR 보고에 남긴다.

## 중단 조건

absolute path 노출, path traversal 허용, 기존 artifact 덮어쓰기 정책이 필요해 보이면 멈춘다.
