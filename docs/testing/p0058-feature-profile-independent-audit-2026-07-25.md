# P-0058 기능·Profile 독립 전수 감사

## 판정

상태: `SOURCE_PASS / EXTERNAL_GATES_PRESERVED`

이 감사는 P-0056·P-0057의 완료 주장과 별개로 A01~D03 23개 기능, C01 16개 Profile과 공통 증거 Gate를 선행 관계 0~13 순서로 다시 확인한다. 과거 P-ID, 다른 revision, 문서 표, fixture-only 또는 in-memory 성공은 현재 구현 증거로 상속하지 않는다.

완료 판정의 machine-readable 입력은 `catalog/product-features.toml`, 생성 증거는 `catalog/product-source-evidence.json`, live release record는 `FinalProductAuditV2`다. source evidence는 다음 여섯 층을 기능별로 모두 요구한다.

1. 정본 owner 문서
2. 실제 contract/type와 generated Schema hash
3. repository·Controller owning handler marker
4. CLI/Controller 및 applicable MCP/Codex 경로
5. 서로 다른 positive·negative·failure·recovery executable test
6. 위 파일의 현재 byte에서 계산한 source fingerprint

## 독립 감사에서 발견한 문제와 수정

| 발견 | 영향 | 수정 |
|---|---|---|
| A02 StageSpec·StageGraph는 문서가 상세했지만 current generated contract와 owning product command가 없었음 | 뒤 Route·execution 증거가 어떤 plan revision을 실행했는지 고정할 수 없음 | `StageSpecV1`·`StageGraphV1`, cycle·parallel overlap 검증, completed-stage preserving replan, GoalStore CAS, `stage plan|show|replan` 추가 |
| A05는 실제 Codex capability snapshot을 생성하지 않았음 | 모델·effort·operation을 추측하거나 stale descriptor를 current로 오인 가능 | version-specific App Server Schema 관찰, real `model/list`/provider probe, `CapabilitySnapshotV1`, `RouteDecisionV1`, `codex capability`·`route` 명령 추가 |
| A05 probe의 `--version`·Schema subprocess에 deadline이 없었음 | hung executable이 Controller를 무기한 점유 | 30초 bounded wait, timeout kill, stdout bounded drain과 hang 회귀 test 추가 |
| A05 Schema probe가 generated Schema의 모든 문자열을 method/field 후보로 읽었음 | description/example에 이름만 등장해도 미지원 App Server method를 ready로 승격할 수 있음 | 실제 `properties.method.const|enum`과 `properties` key만 관찰하고 description/example spoof를 거부 |
| A05 real-process positive fixture가 parser 보강 뒤에도 비표준 최상위 `methods` 배열을 생성했음 | 좁은 adapter test는 통과하지만 workspace TARGET의 실제 child-process capability probe가 실패 | fake App Server Schema도 실제 generated Schema와 같은 `properties.method.const` 구조로 변경하고 real-process positive·negative·failure·recovery 4종을 재실행 |
| A06은 terminal 명령 표면만 있고 공식 App Server thread/turn lifecycle과 approval/effect binding이 없었음 | `ready`처럼 보이나 실제 start/resume/fork/interrupt가 증명되지 않음 | `CodexExecutionRecordV1`, App Server JSONL start/resume/fork/turn/interrupt, exact approval scope, terminal effect receipt, restart `outcome_unknown`, CLI/Controller 명령 추가 |
| capability probe가 reasoning effort `max`·`ultra`를 동명 execution mode로 승격하고 미구현 managed Ultra도 광고했음 | Router가 생성한 `ready` RouteDecision을 A06 owning executor가 실행 시 거부하는 false-ready | effort/mode 분리, 현재 구현된 `single/native`만 광고, managed Ultra 기본값 `false`, 명시적 활성화 요청은 `ROUTE_MODE_UNAVAILABLE`로 fail-closed |
| C01 descriptor가 activation/check merge는 했지만 unknown outcome·rollback 정책을 exact resolution에 넣지 않았음 | 실패·복구 정책이 Profile 밖에서 달라질 수 있음 | exact 16개를 schema v2/profile 1.1.0으로 올리고 두 정책과 parent version을 fingerprint에 포함 |
| C01 final audit가 Profile ID·version·resolution hash만 보고 current source/config/toolchain·required Check coverage·승인/복구/rollback 경로를 확인하지 않았음 | exact resolution이 성공해도 실제 환경과 공통 engine 경로가 stale이거나 우회된 false-ready 가능 | raw definition·activation/policy fingerprint, current Project/Workspace/Index/config/toolchain binding, 공통 경로 10개와 required Check exact coverage를 `ProductSourceEvidenceV1`/runtime status에 추가하고 incomplete status를 강제 downgrade |
| `FinalProductAuditV1`은 정적 owner/command 문자열과 runtime profile 결과만 확인했음 | 실제 Schema·handler·4종 test byte가 없거나 stale이어도 false-ready 가능 | fail-closed source inventory checker, `ProductSourceEvidenceV1`, embedded 재검증과 `FinalProductAuditV2`로 live `release.audit` 교체 |
| source checker가 generated Schema 수를 출력만 하고 정본 기대값을 검사하지 않았음 | 기능별 ref 밖의 public Schema가 사라져도 stale evidence 쓰기 단계까지 진행 가능 | inventory에 current 213을 선언하고 Profile 16·Runtime 4와 함께 exact count를 evidence 생성 전에 검사 |
| MCP/Codex 적용성을 모든 기능의 일반 gateway 존재로 통과시켰음 | 해당 기능에 등록 action이 없어도 MCP 경로가 있다고 오판 | 기능별 exact core backend/fixed MCP action과 Codex path marker를 inventory에 기록하고 등록 여부 검증 |
| B05 external snapshot이 caller가 준 provenance·freshness를 그대로 신뢰했음 | stale advisory·license/security 자료를 current로 위조해도 검증을 통과할 수 있음 | source revision·workspace·content·tool/config fingerprint와 provider observation을 current run에 결합하고 forged/stale 입력을 거부 |
| B06 `verified` failure가 실제 failed ValidationResult·Diagnostic 없이도 생성됐고 failure input byte를 보존하지 않았음 | 재현과 regression이 서로 다른 입력을 대상으로 해도 같은 실패의 recovery처럼 보일 수 있음 | `FailureRecord`·`ReproductionPackV2`에 `input_fingerprint`를 추가하고 current failed run, confirmed Diagnostic, reproduction/regression exact input을 owning handler에서 검증 |
| B07 docs/config handler가 임의 managed registry 선언을 받아들였음 | 실제 관리 symbol/error/config registry와 무관한 caller supplied 목록으로 drift check를 통과할 수 있음 | persisted current registry의 exact revision/fingerprint와 source-bound documentation snapshot만 허용하고 임의 등록은 fail-closed |
| 기존 v1 `DocumentationSnapshot`·`ExternalDataSnapshot`에 새 provenance 필드를 필수로 추가했음 | 저장된 구 JSON을 읽지 못하거나 기본값을 current evidence로 오인할 수 있음 | 구 필드는 conservative default로 읽되 `verify_documentation_snapshot`·`verify_external_data_snapshot`이 registry/project/normalized binding 없는 기록을 current evidence로 거부하도록 하고 회귀 test 추가 |
| C01 final audit가 source marker와 toolchain 문자열만으로 required Check를 covered로 채웠고 저장 Result의 full subject binding을 재검증하지 않았음 | 실제 ValidationPlan/Run/Result가 없거나 다른 Task/Scope/Change/Gate/Catalog binding이어도 16개 Profile coverage가 완료로 표시될 수 있음 | current TaskSpec·PlanningBundle·ValidationPlan, exact CheckPlan item, 재봉인한 complete stable pass Result와 Run의 full shared subject binding까지 연결된 family만 covered로 계산 |
| D03 lifecycle이 event 수만 확인하고 phase/digest 전이를 검증하지 않았으며 ARM64는 실제 native receipt도 수용할 수 없었음 | forged lifecycle history가 통과하고 향후 native 증거를 영구히 표현할 수 없음 | `ReleaseLifecycleEvidence`를 public generated contract로 이동하고 exact phase/digest/evidence transition을 검증, fake는 `native_unverified`, native effect receipt는 별도 수용. V1/V2 audit 모두 ARM64 Preview의 external reason을 보존하도록 회귀 기대값도 통일 |

## 우선순위 0~13 결과

| 순위 | 묶음 | 판정 근거 | 현재 상태 |
|---:|---|---|---|
| 0 | 증거 수용 Gate | feature inventory checker, generated Schema manifest hash, source evidence V1, final audit V2 tamper test | source Gate pass |
| 1 | 상태·저장·Writer 기반 | P0 SQLite/Controller writer, project/global store, revision/CAS, backup·restore·rebuild·import와 corruption/restart tests를 inventory ref로 current-byte 재검증 | source Gate pass |
| 2 | A07·A08·A10 | recovery/approval/registry owner와 fixed MCP 12·core action 17, descriptor/LKG failure tests | source Gate pass |
| 3 | A03 | multi-root·Git/non-Git·linked worktree identity, dirty-byte scan/index, Context Pack freshness tests | source Gate pass |
| 4 | A01·A02·A04 | Goal/Task CAS, Stage DAG/replan, evidence-bound `StageResultV1`·Goal completion, ChangeSet·impact·affected Check와 scope 확대 차단 | 수정·source Gate pass |
| 5 | A05·A06 | real child-process fake-Codex capability/thread lifecycle, exact route, approval, terminal receipt와 timeout/restart recovery; 공식 App Server Schema에 의한 runtime probe | source Gate pass, live installed Codex CLI evidence 없음 |
| 6 | B01·B02·B03 | actual diff/claim/evidence, test weakening, negative corpus, validator guard와 cache fingerprint | source Gate pass |
| 7 | B04·B07 | immutable PatchSet, dirty-overlap/rollback, managed registry, contract/config/docs/toolchain drift | 수정·source Gate pass |
| 8 | B05·B06 | ReproductionPack, effect receipt, recovery, security/dependency freshness와 Radar | 수정·source Gate pass |
| 9 | B08 | migration chain, backup/restore rehearsal, outcome unknown, performance cohort, equivalence, ARM64 boundary | source Gate pass, ARM64 native external |
| 10 | A09·D01 | worktree/merge queue, project별 Gate, partial hold/compensation, remote before/after와 approval | source Gate pass |
| 11 | C01 16개 | exact audit order, schema v2/profile 1.1.0, raw source·activation/policy hash, current project/config/toolchain, parent closure, required Check coverage와 permission/unknown/rollback 공통 경로 | 수정·16/16 source Gate pass |
| 12 | B09·D02 | build-once ReleaseManifest, Finding/Suppression, evaluation policy, provider cost/budget, source-bound 23/16 audit | V2·coverage 수정·source Gate pass |
| 13 | D03 | Runtime 4, installer/plugin/hook/updater lifecycle, user-data preservation, SBOM/provenance/signing/publish 경계 | lifecycle 수정·focused pass, 외부 Gate 분리 |

## 현재 exact inventory

- 기능: A01~A10 `10/10`, B01~B09 `9/9`, C01 `1/1`, D01~D03 `3/3`, 합계 `23/23`
- Profile: 지정된 선행 순서 `16/16`
- Runtime executable: `star`, `star-controller`, `star-mcp`, `star-updater`, `4/4`
- generated Schema manifest: baseline 202개에서 StageSpec/StageGraph/StageResult·Route/Capability/Codex execution 6개, source/final audit 2개, ContextPack/PermissionPlan 2개, release lifecycle 1개를 추가해 current `213`
- closed stable error catalog: `528`
- MCP verification matrix: `170/170`

Schema count는 generator가 열거한 public contract 수를 그대로 사용한다. 숫자를 맞추기 위해 새 contract를 manifest 밖에 두지 않으며 최종 판정은 count와 각 파일 hash, source evidence fingerprint를 함께 사용한다.

## 외부·플랫폼 경계

local source conformance와 실제 Stable publication은 별개다. Authenticode certificate/timestamp, public GitHub Release publish/readback과 ARM64 native verification이 실행되지 않았다면 `FinalProductAuditV2.status`는 이를 `blocked_external` 또는 limitation으로 보존해야 한다. Preview simulation을 ARM64 Stable native evidence로 승격하지 않는다.

현재 셸의 `Get-Command codex.exe`는 Codex Desktop WindowsApps package 내부 EXE를 반환하지만 `codex.exe --version`과 `app-server generate-json-schema` process 생성은 Windows `Access is denied`로 실패했다. 별도 실행 가능한 Codex CLI는 발견되지 않았고 package 설치는 수행하지 않았다. 따라서 fake child-process E2E는 source protocol/handler 검증으로만 사용하며, 현재 설치본에 대한 live capability·thread/turn 성공 증거로 승격하지 않는다. 해당 path는 접근 가능한 공식 Codex CLI가 제공된 뒤 새 capability fingerprint로 다시 검증해야 한다.

Codex adapter의 method와 effort 값은 실행 시점 App Server generated Schema와 `model/list` 관찰값만 사용한다. 구현 기준은 OpenAI의 공식 [App Server README](https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md)와 [v2 protocol model](https://github.com/openai/codex/blob/main/codex-rs/app-server-protocol/src/protocol/v2/model.rs)이다.

## 최종 검증 기록

- inventory default read: `pass`, 기능 `23/23`, Profile `16/16`, Runtime `4/4`, generated Schema `213`, stable error `528`, MCP matrix `170/170`.
- TARGET: `requested=target`, `required=full`, `effective=full`, 11개 check `pass`, 137.6초, `target/validation/20260725T142744343Z-14668/report.json`.
- FULL: `requested=full`, `required=full`, `effective=full`, 11개 check `pass`, 127.7초, `target/validation/20260725T143017384Z-16272/report.json`.
- RELEASE pre-commit: 16개 중 local build/cross-build/lifecycle을 포함한 14개 `pass`; 외부 signing/publication은 `unverified/not_run`, clean-worktree는 commit 전 변경 때문에 `fail`, `target/validation/20260725T143235093Z-5772/report.json`.

외부 signing/publication과 ARM64 native, 접근 가능한 installed Codex CLI가 없다는 사실을 source pass로 대체하지 않는다. clean-worktree RELEASE 재검증과 push SHA는 commit 이후 handoff evidence로 남긴다.
