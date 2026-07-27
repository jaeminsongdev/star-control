# PLANS.md

## 목적

이 문서는 현재 판단과 다음 실행만 보존하는 bounded snapshot이다. 세부 계약은 [문서 읽는 순서](docs/README.md), 구현 순서는 [최종 구현 로드맵](docs/roadmap/final-implementation.md), 실행 증거는 `docs/testing/`과 생성 validation artifact가 소유한다. historical `DONE`은 해당 revision의 seal이며 현재 source 완료 증거로 자동 승격하지 않는다.

## 확정 불변식

- source·manifest·Catalog가 canonical이고 DB/index/cache는 derived state다. 신규 scanner별 DB나 completion model을 만들지 않는다.
- `partial|stale|unsupported|unverified|not_run|flaky|outcome_unknown`을 pass로 바꾸지 않는다.
- Finding은 source를 직접 바꾸지 않으며 `ChangeRecipeV2 → isolated PatchSetV2 → pre/post Gate → exact approval` 경로만 사용한다.
- external analyzer·LSP·OpenRewrite·mutation engine·Scorecard는 registered adapter다. 설치되지 않은 provider는 설치하지 않고 `unavailable|unverified`로 보존한다.
- `legacy/`, `target/`, Codex runtime DB/cache 및 사용자 management/project data를 직접 수정·정리하지 않는다.
- remote push/PR/publish, dependency·executable 설치, Runtime 교체, signer·timestamp는 별도 승인 없이는 실행하지 않는다.

## 현재 상태

| 범위 | 상태 | 현재 판정 |
|---|---|---|
| P-0039~P-0059 | historical seal | 기존 core·23개 feature·16개 Profile의 현재 source 재감사 근거는 historical과 분리한다. |
| P-0060 Codex routing·delivery | deferred | 설치/package 후속은 본 Code Health source Slice와 분리하며 Runtime을 변경하지 않는다. |
| P-0061 PR0~31 final lock | held | CrossRepo bundle aggregate와 external gate를 보존한다. |
| P-0062 Code Health 정본 설계 | **READY_TO_COMMIT** | canonical design·reading order·roadmap·ledger와 current product-source evidence를 갱신했다. public-contract FULL과 strict self-review 뒤 intended files만 local commit한다. |
| P-0063~P-0070 | pending | SARIF → duplication/complexity/unused → Gate/Radar → history/ownership/debt → semantic provider → mutation/rule-pack/posture → evaluation/Profile → final audit 순서다. |
| public signed Stable | `blocked_external` | certificate/private key/trusted timestamp와 signed lifecycle/publish가 필요하다. |

## P-0062 활성 Slice

- 목표: 기존 `ProjectCatalog/CodeIndex → ScanRun/Finding/DiagnosticV2 → ValidationPlan/GateDecision → Radar/ReviewPack → PatchSet → EvaluationRun`을 확장하는 Code Health 정본을 확정한다. 문서 존재는 제품 구현 완료를 뜻하지 않는다.
- 변경 범위: `docs/contracts/code-health-and-maintainability.md`, `docs/README.md`, `docs/contracts/README.md`, `docs/roadmap/final-implementation.md`, `PLANS.md`.
- 선택 Profile: `project_understanding`, `change_planning`, `architecture_quality`, `test_correctness`, `ai_development_validation`; 전체 후보 closure fingerprint는 `sha256:5eb65d478ea0008d27e38ca788935b2777f78200b25e2c9db67fe5205c1a9e84`이다. 문서 Slice는 `docs_config_environment`도 적용한다.
- live 기준선: `main`/`origin/main` `00bd842f95541d23267cdf250d157c3f1864670d`, clean worktree. 설치는 x64 `0.1.0` verified/registered, management `normal`/`read_write`, built-in Profile 16개, Catalog fingerprint `sha256:240fc06b8d4843db32c66460030f92ff78bc7b83f8e90f39185727eb0268987e`.
- 검증: 우선 `pwsh ./scripts/validate.ps1 -Profile quick -Unit docs -OutputFormat json`; public-contract 승격이면 `pwsh ./scripts/validate.ps1 -Profile full -OutputFormat json`, strict self-review, intended files만 local commit. push/PR/publish는 금지다.

## 열린 위험과 보류

- R-0062: real external analyzer/provider executable과 network snapshot은 설치·승인 없이 실행할 수 없다. adapter fixture만 구현하고 실제 provider 결과는 `unavailable|unverified`로 남긴다.
- R-0063: `code_health_maintenance` 17번째 Profile은 EvaluationRun evidence와 제품 결정 전까지 추가하지 않는다. 기본은 기존 16 Profile 조합이다.
- R-0064: raw source, author name/email, secret 및 개인 absolute path는 ArtifactRef·fingerprint·Radar에 저장하지 않는다.

## Context Pack

- 현재 목표: P-0062 정본 문서 Slice의 final evidence/FULL 및 strict self-review를 확인하고 local commit으로 닫은 뒤 P-0063 SARIF vertical Slice를 시작한다.
- 먼저 확인할 파일: `docs/contracts/code-health-and-maintainability.md`, `docs/contracts/project-catalog-and-code-index.md`, `docs/contracts/validation-and-evidence.md`, `crates/foundation/star-contracts/src/index.rs`, `crates/control/star-validation/src/runner.rs`, `catalog/product-features.toml`.
- 먼저 실행할 명령: `git status --short --branch`; 문서 Slice 후 `pwsh ./scripts/validate.ps1 -Profile quick -Unit docs -OutputFormat json`; P-0063 source 변경 후 `pwsh ./scripts/validate.ps1 -Profile full -OutputFormat json`.
- 건드리면 안 되는 것: existing dirty/user changes, `target/`, `legacy/`, installed Runtime/cache, external account/remote state.
- 다음 완료 기준: P-0063은 generated Schema를 직접 편집하지 않고 declared `star-schema-gen` 경로로 생성하며, SARIF contract/public boundary 변경이므로 FULL·strict review·local commit이 필요하다.
