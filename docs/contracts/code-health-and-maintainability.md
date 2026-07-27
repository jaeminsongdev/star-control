# Code Health·장기 유지보수 계약

## 상태와 소유권

이 문서는 P-0062의 **정본 설계**다. 아래 Artifact, Rule, Check, adapter와 제품 경로는 명시적으로 구현 Slice가 닫히기 전까지 구현 전이며 문서 존재를 구현 완료로 표현하지 않는다. P-0064A의 Rust exact structural clone 후보만 이 문서의 계획 중 source 구현 대상이고, current evidence는 `PLANS.md`, source test 및 생성 validation report가 소유한다.

이 계약은 [Project Catalog·Code Index](project-catalog-and-code-index.md), [변경 계획·영향 분석](change-planning-and-impact.md), [검사·완료·증거](validation-and-evidence.md), [공통 Validation Gate](../features/common-validation-gate.md), [안전한 Patch·codemod](safe-patch-and-codemod.md), [실패·보안·의존성 유지보수](failure-security-and-dependency-maintenance.md), [설정·Catalog](config-and-catalog.md), [Profile](../features/profiles.md), [D02 평가](../features/operations.md#d02-비용평가규칙-개선)를 조합한다. 이 문서는 기존 type의 의미를 복제하지 않고 Code Health의 producer·coverage·판정·연결 규칙만 소유한다.

## 목표와 제외 범위

목표는 외부 정적 분석 결과와 source/Git-derived 관측을 기존 제품 흐름에 연결하는 것이다.

```text
Git source / CodeIndex
  → ScanRun → Finding / Occurrence → DiagnosticV2
  → ImpactAnalysis / ValidationPlan → Baseline / Suppression / GateDecision
  → MaintenanceRadarSnapshot / ReviewPack → (필요 시) ChangeRecipeV2 / PatchSetV2
  → post-apply Gate → EvaluationRunV2
```

다음은 제품이 재구현하지 않는다.

- 범용 SAST, CodeQL, PMD CPD, OpenRewrite, mutation engine, OpenSSF Scorecard, package manager, LSP server, compiler, profiler
- scanner별 별도 DB, 별도 Finding/Diagnostic/completion model, 단일 종합 품질 점수
- Finding에서의 hidden apply, live checkout에서의 raw global replacement, 자동 public API 삭제 또는 자동 extract-method
- raw author identity에 의한 개인 평가·성과 측정·결함 책임 귀속

외부 provider는 `ToolDescriptor`와 exact executable identity, declared output schema, bounded raw `ArtifactRef`, normalizer 및 Gate 연결만 제공한다. executable·account·network가 없으면 설치하거나 우회하지 않으며 결과를 `unavailable|unverified`로 남긴다.

## 기존 계약 재사용과 신규 Artifact 경계

| 책임 | 기존 canonical type 또는 경계 | P-0062 이후 추가할 계약 |
|---|---|---|
| source/index binding | `ProjectRevision`, `WorkspaceSnapshot`, `CodeIndexSnapshot`, `ScanRun` | observation의 revision/workspace/index binding과 source-class coverage |
| finding·판정 | `Finding`, `Occurrence`, `DiagnosticV2`, `RuleV2`, `DiagnosticEvidenceRefV2` | producer-specific Rule mapping과 limitation/evidence reference |
| validation | `ValidationPlan`, `ValidationRunV2`, `BaselineV2`, `SuppressionV2`, `DispositionV2`, `GateDecision` | compatible baseline, new/worsened ratchet 및 shadow Gate facts |
| maintenance | `MaintenanceRadarSnapshot`, `ReviewPack` | deterministic quality observation priority와 evaluation input refs |
| remediation | `ChangeRecipeV2`, `RecipeExecution`, `PatchSetV2` | reviewed semantic provider candidate만의 remediation metadata |
| effectiveness | `EvaluationRunV2` | false-positive/cost/utility/Profile promotion cohort |

추가 후보 Artifact는 각각 별도 source Schema와 valid/full/invalid/future fixture를 가진다. 생성 JSON은 `star-schema-gen`의 declared path로만 만든다.

| 계획 Schema | 최소 binding | 역할 |
|---|---|---|
| `static-analysis-import-report.schema.json` | ProjectRevision, WorkspaceSnapshot, ToolDescriptor/tool hash, Rule Pack digest, URI mapping policy | SARIF import의 accepted/rejected/truncated count, completeness, limitation과 raw/normalized ArtifactRef |
| `code-health-observation-set.schema.json` | Project/revision/workspace/CodeIndex, rule/config fingerprint, source-class coverage | duplication·complexity·unused·debt observation, budget, limitation, content fingerprint |
| `git-history-risk-snapshot.schema.json` | repository identity, inspected commit range/time window, CODEOWNERS source fingerprint | relative churn/change burst/component mapping/opaque ownership/PII redaction/validity boundary |
| `quality-rule-pack-manifest.schema.json` | pack id/version/source/digest/tool identity | supported language/source class, rule definitions, corpus, SARIF mapping, lifecycle, trust/freshness |

이 계약들은 기존 document에 field를 무단 추가하지 않는다. `serde(deny_unknown_fields)` reader, historical fixture 또는 명시적 새 version·migration이 먼저 필요하며, 새 schema가 구현될 때까지 Catalog/manifest count를 바꾸지 않는다.

## identity·fingerprint·artifact·privacy

- stable identity는 Project/revision, source relative location, symbol identity 또는 source-derived structural identity, Rule ID/version, normalized parameter, producer/config fingerprint 및 location contract로 만든다.
- timestamp, run ID, cache hit, ArtifactRef path, raw source, raw author identity, private absolute path는 content fingerprint에 넣지 않는다.
- line 이동은 identity를 바꾸지 않되 semantic/structural change, Rule version, source-class/cohort change, location contract change는 새 observation 또는 incompatible comparison을 만든다.
- raw external result와 large metric은 redacted, content-addressed `ArtifactRef`로 보존한다. secret-bearing message, credential, personal path, author name/email는 redacted shape와 bounded code만 보존하며 원문·hash를 저장하지 않는다.
- secret/sensitive source class, generated/vendor/binary/excluded source class, unsupported language와 resource-limit scope는 coverage/limitation에 명시한다. 관측하지 못한 영역의 빈 결과는 `confirmed_empty`가 아니다.

## 공통 판정 상태

| 상태 | 의미 | Gate/Radar 처리 |
|---|---|---|
| `candidate` | tool 또는 bounded structural detector가 발견했지만 semantic/owner policy 확인 전 | review/report-only; automatic defect 또는 PatchSet 금지 |
| `confirmed` | source binding, supported coverage, Rule contract와 required confirmation을 충족 | Baseline/ratchet과 ReviewPack input 가능 |
| `existing_unchanged` | compatible baseline에서 기존과 동일 | 보존·보고; new violation으로 집계하지 않음 |
| `new|worsened|improved|resolved` | compatible baseline comparison 결과 | new/worsened만 초기 policy의 관심 대상 |
| `unbaselined|incompatible` | compatible baseline이 없음/metric·Rule contract가 다름 | pass 또는 improvement claim 금지 |
| `partial|unverified|suspected` | resource, source class, semantic frontier 또는 external provider 한계 | clean pass 금지; limitation과 scope를 ReviewPack/Radar에 노출 |

초기 신규 Rule은 shadow/report-only다. 기존 architecture/test/security required set을 줄일 수 없고, history·ownership·aggregate score는 단독 `BLOCK`을 만들 수 없다. only complete·stable·compatible evidence가 ratchet 또는 evaluation comparison에 사용된다.

## Check·Rule family와 초기 정책

모든 Rule은 stable ID/version, definition fingerprint, supported language/source class, producer ref, parameter schema, location/evidence/remediation contract, Gate floor, fixture manifest, lifecycle, suppression policy, false-positive guard와 redaction contract를 가져야 한다.

| Check family | Rule family | 초기 제품 의미 |
|---|---|---|
| duplication | `code_clone` | Rust function/block exact structural clone부터; deterministic token, bounded index/hash, generated/vendor/cache/output 제외와 production/test cohort 분리, near clone은 후속 |
| complexity | `complexity_regression` | cyclomatic, nesting, token/line, branch/match-arm을 symbol-level metric으로 비교; threshold는 project config와 compatible metric version에만 적용 |
| unused_surface | `unused_symbol`, `unused_file`, `unused_dependency` | private symbol/file 및 dependency candidate; public consumer·macro·reflection·dynamic frontier는 confirmed deletion으로 바꾸지 않음 |
| history_hotspot | `relative_churn`, `change_burst` | exact commit range/component window의 read-only derived priority; shallow/rewrite history는 unverified |
| ownership | `ownership_concentration` | CODEOWNERS declared owner와 opaque contribution distribution 비교; advisory-only, raw identity 저장 금지 |
| debt_marker | `unowned_debt`, `expired_debt`, `stale_deprecation` | TODO/FIXME/HACK/TEMP/DEPRECATED/REMOVE_AFTER candidate와 structured owner/issue/expiry/replacement; plain marker는 candidate |

복잡도·hotspot·ownership correlation은 causal proof가 아니다. generated/vendor/test/source-class exclusion, component size/window, parser/index budget은 definition fingerprint에 포함하거나 evidence로 고정한다.

## SARIF import와 external output

P-0063은 SARIF 2.1.0만 registered `validation.run` output normalizer로 연결한다. 별도 CLI는 실제 consumer requirement가 증명될 때까지 만들지 않는다.

- tool identity/hash, Rule Pack identity/digest, source revision, workspace snapshot, URI/path policy를 input 전에 bind한다.
- Rule ID/version, severity/confidence, location/path/URI, partial fingerprint, duplicate correlation을 `Finding`/`Occurrence`/`DiagnosticV2`로 정규화한다.
- raw artifact preservation, result/location/size cap, malformed/future version rejection, truncated count 및 unsupported property limitation을 의무화한다.
- path traversal, cross-project location, stale source binding, secret-bearing message, timeout/crash/outcome unknown은 result를 clean finding set 또는 pass로 만들지 않는다.

## baseline·suppression·Gate·Radar 연결

Baseline은 Rule definition/config/source cohort/metric version/source class가 compatible할 때만 비교한다. incompatibility 또는 stale baseline은 existing/new/resolved를 합성하지 않는다. Suppression은 exact subject·reason·approval·expiry·scope를 요구하고 expired/stale/revoked suppression을 active로 보지 않는다.

`ValidationPlan`은 affected code-health Check를 기존 required family floor 위에 materialize한다. `validation.run`은 normalization과 limitation을 `DiagnosticV2` evidence에 연결하고, `GateDecision`은 shadow policy/coverage/compatibility를 분리해 기록한다. `MaintenanceRadarSnapshot`은 complete input refs와 fixed evaluation time에서 deterministic ordering으로 observation priority를 만드는 derived view이며, 별도 completion engine이 아니다. `ReviewPack`에는 Rule/version, source binding, baseline state, suppression state, coverage/limitation, redacted evidence와 reviewed remediation candidate만 넣는다.

## semantic remediation 경계

LSP rename/codeAction, structured analyzer fix, OpenRewrite는 P-0067에서 existing `RewriteTransformerPort`와 M4 경로로만 연결한다.

1. exact provider/executable identity와 capability를 확인한다.
2. isolated preview에서 provider를 실행하고 actual diff를 수집한다.
3. typed `PatchSetV2`로 정규화한 뒤 scope/public surface 검사, impact replan, idempotence replay, `patch_pre_apply` Gate를 수행한다.
4. exact durable approval과 single-use apply 뒤 actual ChangeSet 재수집, `patch_post_apply` Gate, rollback/recovery evidence로 닫는다.

provider suggestion은 검증 전 `MachineApplicable`가 아니며 Finding은 apply를 숨기지 않는다. live checkout 외부 mutator, raw literal global replacement, clone auto-extract-method, public API automatic deletion은 금지한다.

## P-0064A exact structural clone 경계

P-0064A는 Rust `function` body와 그 안의 nested `block`의 exact token leaf sequence를 즉시 hash하여 후보로 만든다. macro·module top-level block처럼 owning function을 확정할 수 없는 범위는 후보에서 제외한다. whitespace와 comment는 비교에서 제외하지만 identifier·literal·operator·구조가 달라지면 같은 후보로 합치지 않는다. 원문과 normalized token stream은 candidate, Finding, Occurrence, fingerprint 어디에도 저장하지 않는다.

candidate는 `Source`와 `Test`를 서로 다른 cohort로만 grouping한다. generated/vendor/cache/output과 그 밖의 source class는 후보를 만들지 않으며, token cap 또는 parser limit은 empty-clean 결과가 아니라 source-bound limitation이다. Finding identity는 normalized token fingerprint, structural kind, cohort, sorted owning symbol identity로 계산하고 location은 Occurrence에만 남긴다. 따라서 line 이동은 Finding identity를 바꾸지 않지만 one-member change와 cohort 이동은 새 비교 대상이다.

이 Finding의 severity는 `Info`, confidence는 `Medium`이고 자동 Gate block, PatchSet, 자동 extract-method를 만들지 않는다. identifier/literal normalization near clone, complexity와 semantic remediation은 P-0064B 이후의 별도 Slice다.

## P-0064B complexity regression 경계

P-0064B는 Rust function body에서 `rust-ast-v1` metric을 계산한다. cyclomatic base는 1이고 control-flow, match arm, `&&`/`||`가 증가분을 만들며 maximum nesting, token/line, branch와 match-arm count는 원문 없는 정수로만 저장한다. macro body, fixture, generated/vendor/cache/output 및 지원하지 않는 language는 metric 후보가 아니며 resource/parse limit은 clean 결과를 의미하지 않는다.

비교는 current CodeIndex와 이전 incremental CodeIndex의 metric contract version, language, source-class cohort 및 owning symbol identity가 모두 일치할 때만 한다. baseline 부재·version/cohort 불일치·같거나 개선된 cyclomatic은 regression Finding을 만들지 않는다. 증가 후보는 `Warning`/`Medium`이고 baseline/current 값만 redacted parameter로 남긴다. Finding은 자동 Gate block, ReviewPack priority 변경 또는 PatchSet을 만들지 않는다.

## P-0064C unused surface 경계

P-0064C는 Rust `function|struct|enum|type|trait`과 비-entrypoint source file, 그리고 read-only `DependencySnapshot`의 direct Cargo dependency를 `Info`/`Low` Finding으로만 투영한다. candidate/Occurrence에는 source SHA, relative path, range, symbol 또는 dependency identity만 남기며 manifest/source 원문은 저장하지 않는다.

- private symbol은 confirmed 또는 unresolved reference가 하나라도 있으면 후보가 아니며, test-only reference도 후보를 억제한다. public export는 consumer 관측이 불완전하므로 `public_export_unknown_consumer` 후보로만 남긴다.
- `lib.rs`, `main.rs`, `mod.rs`, declared module과 symbol/reference가 있는 file은 unused file로 만들지 않는다. fixture, test, build/config, generated/vendor/cache/output 및 macro definition은 private deletion 후보에서 제외한다. macro/reflection/dynamic frontier는 candidate의 `unverified` limitation이며 confirmed deletion으로 승격하지 않는다.
- complete dependency snapshot에서만 direct dependency의 Rust identifier reference를 비교한다. lockfile 부재·lock/manifest 불일치·snapshot parser 실패는 `dependency_snapshot_partial|unverified` limitation이며, dependency removal이나 자동 PatchSet을 만들지 않는다. dev/build/optional/workspace/shared dependency 및 external analyzer disagreement는 direct text reference만으로 확인하지 못한 frontier로 남긴다.
- registered SARIF import는 별도 source-bound Finding으로 보존한다. imported analyzer가 unused 후보와 일치하거나 불일치해도 P-0064C가 candidate를 confirmed deletion으로 합치지 않으며, provider completeness/stale binding은 P-0065 Gate의 input이다.

따라서 이 Slice는 deletion advice·BLOCK·automatic suppression을 만들지 않는다. owner/semantic/external confirmation과 allow policy의 결정은 P-0065 이후 existing Finding·ReviewPack·Gate 경로에서만 처리한다.

## mutation·Rule Pack·repository posture

Mutation은 changed code 중 parser/protocol/public contract/core calculation trigger에만 한정하고 mutation/time/survivor budget을 execution 전에 fix한다. line coverage와 별도 evidence이며 timeout/flaky/partial은 pass가 아니다.

Rule Pack은 versioned manifest, source/digest, language/source-kind, fixture corpus, query metadata, SARIF mapping, lifecycle/deprecation/replacement, signature/trust/freshness를 가진다. custom CodeQL-like output도 SARIF normalizer를 재사용한다.

OpenSSF Scorecard 같은 repository posture는 source URL/query/schema/tool version/fetched time/digest/coverage/validity를 bound한 external snapshot이다. aggregate score는 Gate하지 않으며 individual evidence만 `security_supply_chain` input이 될 수 있다. network/token이 필요하면 별도 승인을 요청한다.

구현은 이 세 evidence를 strict schema와 registered read-only port로 분리한다. mutation snapshot은 current scan/index와 changed path·trigger·고정 budget이 모두 일치할 때만 complete로 취급하며 timeout/flaky/partial은 Radar에서 advisory evidence로도 pass로 승격하지 않는다. Rule Pack은 trusted·현재 freshness·exact analyzer SHA-256이 모두 맞을 때만 기존 SARIF normalizer가 만든 import report에 digest를 결속한다. untrusted, expired 또는 ambiguous pack은 binding하지 않고 limitation을 남긴다. posture snapshot의 aggregate score는 저장하거나 Gate/Radar blocking에 사용하지 않는다.

## EvaluationRun과 Profile 전략

처음에는 16 built-in Profile을 유지하고 `project_understanding`, `architecture_quality`, `test_correctness`, `ai_development_validation`을 기본 조합으로 사용한다. 필요할 때만 `docs_config_environment`, `security_supply_chain`, `performance_build`, `refactor_codemod`을 추가한다.

`code_health_maintenance@1.0.0`의 17번째 Profile은 shadow/offline/replay `EvaluationRunV2`이 fingerprint stability, false-positive/suppression/flaky/partial rate, bounded duration and per-1,000-file cost, replan/retry/manual review time, PatchSet accept/reject/rollback, 그리고 기존 조합 대비 utility를 증명한 뒤에만 `accept`한다. 증거가 부족하면 `trial` 또는 `reject`가 정상 결과다. recommendation은 Catalog를 자동 변경하지 않는다.

P-0069의 현재 source corpus는 external provider 비용 또는 실사용 cohort가 없으면 `NeedsReview`를 `trial` catalog candidate로만 기록하며 built-in Profile 16개를 바꾸지 않는다. `Keep`/`Reject`는 rejected tombstone으로 남고, `Accept`는 `evaluation.profile.decision`에서 fail-closed한다. 17번째 built-in 승격은 사용자가 소유한 제품 결정과 descriptor·fixture·inventory·package closure를 같은 Slice에서 모두 갖춘 경우에만 별도로 수행한다.

17번째 Profile을 accept하면 built-in ID count, catalog descriptor/loader, resolution/schema fixtures, product inventory, Codex reference/package closed asset set, docs와 Profile conformance를 같은 bounded Slice에서 모두 갱신한다.

## implementation 순서와 최소 Corpus

| Slice | 종료 산출물 | 최소 negative/failure/recovery focus |
|---|---|---|
| P-0063 | SARIF contract, normalizer, ValidationRun/Diagnostic/Finding product path | malformed/future, URI traversal, secret/path redaction, cap/truncation, stale binding, timeout/crash |
| P-0064A | Rust structural clone | line move, one-member change, source-class exclusion, budget partial, raw source exclusion |
| P-0064B | complexity regression | nesting/match/macro, moved/renamed symbol, threshold edge, incompatible/stale baseline, language cohort |
| P-0064C | unused surface candidate | test/build/config/public/macro/dynamic/reference frontier, lockfile/dependency disagreement |
| P-0065 | scan→finding→planning→validation→baseline/suppression→Gate/Radar | deterministic ordering, ratchet, expiry, partial/unverified clean-pass denial |
| P-0066~P-0068 | history/ownership/debt, semantic provider, mutation/Rule Pack/posture | shallow/rewrite/non-UTF8 privacy, rollback/idempotence, external unavailable/freshness |
| P-0069~P-0070 | evaluation/Profile decision and complete audit | false-positive/cost comparison, 16/17 consistency, current inventory/Schema/FULL |

All slices include deterministic output, positive/negative/edge/regression/failure/recovery, timeout/cancel, resource limit, stale source/tool/Rule Pack, partial semantic frontier, baseline/suppression compatibility, redaction/secret/Windows path and replay/idempotence tests. Tests, severity, corpus and validators are never weakened to make a slice pass.

## P-0062 acceptance

- This document is in the docs reading order and states the exact reuse boundary for existing contracts.
- New Artifact/Rule/Check names are explicitly design-only until their source Schema, generated manifest, handler, CLI/MCP, fixture and product evidence exist.
- External provider and 17th Profile decisions remain fail-closed and approval-bound.
- 문서 Slice의 `quick -Unit docs`가 public-contract 변경으로 FULL로 승격되는 경우 FULL과 strict self-review를 통과한 뒤 P-0062 local commit을 만든다.
