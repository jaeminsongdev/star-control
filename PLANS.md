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
| P-0062 Code Health 정본 설계 | **DONE / local commit** | `5751ab5a269774931ca22afc28cf86b9d98e8039`; FULL 11/11 complete/stable/pass, inventory 23/23·Schema 213·MCP 170/170·Profile 16/16. |
| P-0063 SARIF 2.1.0 normalizer | **DONE / local commit pending** | registered output contract가 `SarifV210`을 선택하며, parser→raw/normalized ArtifactRef→`ValidationRunV2`/`DiagnosticV2`→current Scan generation Finding/Occurrence→immutable import report 저장·조회 경로를 구현했다. FULL 11/11 complete/stable/pass와 strict self-review를 다시 봉인한 뒤 local commit만 남아 있다. |
| P-0064~P-0070 | pending | duplication/complexity/unused → Gate/Radar → history/ownership/debt → semantic provider → mutation/rule-pack/posture → evaluation/Profile → final audit 순서다. |
| public signed Stable | `blocked_external` | certificate/private key/trusted timestamp와 signed lifecycle/publish가 필요하다. |

## P-0063 활성 Slice

- 목표: registered external analyzer의 SARIF 2.1.0 output을 source/revision/tool identity에 bind하고 current `ValidationRunV2`/`DiagnosticV2`/Finding/Occurrence workflow로 fail-closed normalization한다.
- 구현된 경계: `CheckDescriptor.output_normalizer=SarifV210`만 parser를 선택한다. parser는 provider message·absolute path를 저장하지 않고, raw stdout/stderr와 safe normalized artifact를 별도 보존한다. application은 실행 전후 current CodeIndex를 재확인하고, current `ScanRun`/revision/workspace/source-entry에 맞을 때만 transaction으로 Finding/Occurrence·`StaticAnalysisImportReport`를 기록한다.
- fail-closed: malformed/future SARIF, result/location cap, traversal/cross-project URI, missing rule/tool/message, duplicate correlation, truncation, timeout/outcome_unknown, source-generation mismatch, artifact 부족은 pass 또는 Finding 생성으로 승격되지 않는다.
- 선택 Profile: `project_understanding`, `change_planning`, `architecture_quality`, `test_correctness`, `ai_development_validation`; resolution fingerprint `sha256:5eb65d478ea0008d27e38ca788935b2777f78200b25e2c9db67fe5205c1a9e84`.
- 현재 검증: `star-contracts` 104 tests, `star-validation` 40 tests, `star-application` 15 tests, `star-state` 13 tests 통과. 다음: schema generator→FULL→strict review→product evidence regeneration→FULL 재실행→local commit. push/PR/publish/install은 금지다.

## 열린 위험과 보류

- R-0062: real external analyzer/provider executable과 network snapshot은 설치·승인 없이 실행할 수 없다. adapter fixture만 구현하고 실제 provider 결과는 `unavailable|unverified`로 남긴다.
- R-0063: `code_health_maintenance` 17번째 Profile은 EvaluationRun evidence와 제품 결정 전까지 추가하지 않는다. 기본은 기존 16 Profile 조합이다.
- R-0064: raw source, author name/email, secret 및 개인 absolute path는 ArtifactRef·fingerprint·Radar에 저장하지 않는다.

## Context Pack

- 현재 목표: P-0063 validated SARIF slice를 intended files만 local commit으로 닫고 P-0064A structural clone Slice로 전환한다.
- 먼저 확인할 파일: `crates/control/star-validation/src/process_executor.rs`, `crates/control/star-validation/src/runner.rs`, `crates/control/star-application/src/lib.rs`, `crates/foundation/star-contracts/src/registry.rs`, `crates/foundation/star-contracts/src/evidence_v2.rs`, `catalog/tool-packages/star-control-core.toml`.
- 먼저 실행할 명령: `git status --short --branch`; `rg -n 'ExternalDiagnosticNormalizer|RegisteredProcessCheckExecutor|output_schema' crates apps catalog`; focused test; final `pwsh ./scripts/validate.ps1 -Profile full -OutputFormat json`.
- 건드리면 안 되는 것: existing dirty/user changes, `target/`, `legacy/`, installed Runtime/cache, external account/remote state.
- 다음 완료 기준: P-0063은 generated Schema를 직접 편집하지 않고 declared `star-schema-gen` 경로로 생성하며, SARIF contract/public boundary 변경이므로 FULL·strict review·local commit이 필요하다.
