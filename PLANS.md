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
| P-0063 SARIF 2.1.0 normalizer | **DONE / local commit** | `eed833af5387c9d6367199829839419511ba6000`; registered `SarifV210` parser→raw/normalized ArtifactRef→`ValidationRunV2`/`DiagnosticV2`→current Scan generation Finding/Occurrence→immutable import report. FULL 11/11 complete/stable/pass. |
| P-0064A structural clone | **DONE / local commit** | `d04cb2d7b664d52dd6228dddae57fb6a525ce458`; Rust exact token clone, production/test cohort, source-bound Finding/Occurrence, incremental reuse, macro·fixture exclusion과 FULL 11/11 complete/stable/pass. |
| P-0064B complexity regression | **DONE / local commit** | `c59c3a8789c582d8a43d0884855e799a39613588`; Rust AST complexity metric, compatible previous-index baseline의 new/worsened/improved relation, source-bound Finding과 FULL 11/11 complete/stable/pass. |
| P-0064C unused surface | **DONE / local commit** | `17f56418d27ea6fd5738090cefb38fe5fcddc329`; Rust function/type/file/export/dependency candidate, read-only dependency snapshot과 manifest/lockfile disagreement, build-script frontier, FULL 11/11 complete/stable/pass. |
| P-0065 Gate·Radar | **DONE / local commit** | `db3cb92b3ae9c9dbf332ebf8c1dfdafb50dcf75e`; code-health shadow planning과 read-only Radar projection, current schema/evidence, FULL 11/11 complete/stable/pass. |
| P-0066 Git history·ownership·debt | **IN_PROGRESS** | read-only Git history adapter/port, exact range·relative churn/burst·component mapping, CODEOWNERS opaque distribution 및 debt-marker 후보를 source-bound observation으로 구현한다. |
| P-0067~P-0070 | pending | semantic provider → mutation/rule-pack/posture → evaluation/Profile → final audit 순서다. |
| public signed Stable | `blocked_external` | certificate/private key/trusted timestamp와 signed lifecycle/publish가 필요하다. |

## P-0066 활성 Slice

- 목표: Git-derived hotspot·ownership·debt marker를 read-only, source-bound observation으로 구현하고 Radar/Impact 입력 경계를 닫는다.
- 불변식: exact repository/range identity를 먼저 고정한다. raw author name/email, secret, absolute path는 artifact·fingerprint·Radar에 저장하지 않으며, shallow/rewrite/binary/generated/missing CODEOWNERS는 complete 사실로 합성하지 않는다.
- corpus: full/shallow/rewrite/rename/binary/generated/missing CODEOWNERS, non-UTF8·PII redaction, TODO/FIXME/HACK/deprecation structured metadata, deterministic ordering과 partial limitation을 고정한다.
- 선택 Profile: `project_understanding`, `architecture_quality`, `test_correctness`, `ai_development_validation`; 새 Profile은 추가하지 않는다.
- 검증: affected package TARGET 후 contract/schema/core 영향이므로 FULL, strict self-review, intended files만 local commit. push/PR/publish/install은 금지다.

## 열린 위험과 보류

- R-0062: real external analyzer/provider executable과 network snapshot은 설치·승인 없이 실행할 수 없다. adapter fixture만 구현하고 실제 provider 결과는 `unavailable|unverified`로 남긴다.
- R-0063: `code_health_maintenance` 17번째 Profile은 EvaluationRun evidence와 제품 결정 전까지 추가하지 않는다. 기본은 기존 16 Profile 조합이다.
- R-0064: raw source, author name/email, secret 및 개인 absolute path는 ArtifactRef·fingerprint·Radar에 저장하지 않는다.

## Context Pack

- 현재 목표: P-0066 Git history·CODEOWNERS·debt marker의 privacy-preserving read-only product path→Finding/Radar/Impact projection→corpus→FULL→review→local commit.
- 먼저 확인할 파일: `docs/contracts/code-health-and-maintainability.md`, `crates/foundation/star-contracts/src/maintenance_v2.rs`, `crates/control/star-application/src/lib.rs`, existing project/index adapter와 controller/CLI routing.
- 먼저 실행할 명령: `git status --short --branch`; `rg -n -i 'History|CODEOWNERS|Debt|Radar|Impact' crates apps specs`; focused tests; final `pwsh ./scripts/validate.ps1 -Profile full -OutputFormat json`.
- 건드리면 안 되는 것: existing dirty/user changes, `target/`, `legacy/`, installed Runtime/cache, external account/remote state.
- 다음 완료 기준: P-0066은 exact Git range와 redacted ownership/debt observations를 existing source-bound Finding/Radar/Impact 경계로 materialize하고 corpus·FULL·strict review·local commit이 필요하다.
