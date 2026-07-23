# P-0055 비서명 외부 Gate·복구 Slice 봉인 감사

## 목적과 판정 경계

이 문서는 `main` 기준 P-0054 내부 제품 구현 위에서 서명을 제외한 외부 실행 경로를 실제 adapter·Operation·영수증·도메인 상태·원격 readback까지 닫는 P-0055의 진행·최종 증거 정본이다. 사용자는 package/dependency 설치, network, disposable install/update/repair/uninstall, GitHub branch/tag/draft/upload/readback/cleanup과 ARM64 교차 simulation을 승인했다.

다음 두 판정은 끝까지 분리한다.

- 비서명 제품/운영 경로: 코드·Corpus·실행 증거가 모두 닫히면 `DONE`으로 봉인할 수 있다.
- 공개 Stable: Runtime·installer Authenticode와 trusted timestamp가 없으므로 unsigned publish로 우회하지 않고 `blocked_external`을 유지한다.

ARM64는 `aarch64-pc-windows-msvc` 교차 빌드, PE architecture, file manifest, installer model과 fake/disposable lifecycle로 검증한다. 이 증거는 Preview `native_unverified`이며 실제 ARM64 장치 runtime 성공으로 표현하지 않는다.

## 기준선과 보존 범위

| 항목 | 값 |
|---|---|
| repository | `D:\개발\관제\Star-Control` |
| 시작 branch | `main` |
| 기준 HEAD / `origin/main` | `a93de7e68aff3ac02315d3a324aeaa497e1ede38` |
| 포함 변경 | 기존 P-0054 dirty implementation 전체와 그 위의 P-0055 외부 seal |
| 보존 | 사용자 dirty/untracked, `legacy/`, `target/`, Codex runtime DB/state, installed Plugin cache |
| 금지 | Authenticode signing, unsigned Stable publish, source 대신 `dist/`·`target/` 직접 수정, 검사 약화 |

## 최신 main 기준 전수조사 결론

P-0054 감사에서 남은 비서명 gap은 “등록 adapter가 필요하다”는 계획 상태와 실제 process effect 사이의 공통 증거 경로, M10 provider 구현, ARM64 교차 검증, clean lifecycle, installed Codex readiness, authenticated GitHub readback이었다. P-0055는 이 gap을 다음 구현 단위로 닫는다.

| 범위 | 구현 | fail-closed 조건 | 현재 증거 상태 |
|---|---|---|---|
| 공통 외부 effect | `DevelopmentEffectReceiptV1`, `RegisteredDevelopmentEffectAdapter`, durable `OperationSnapshot.permission_actions`, `development.effect.record` | exact Project/subject/descriptor/arguments/executable/approval/permission/Gate 불일치, stale/partial/unknown 거부 | focused unit·Controller·CLI PASS |
| M7 security | SecurityRefresh/LicenseScan 영수증과 exact input source SHA-256 소비 | succeeded 영수증 없는 available input, 다른 source/effect/Project 거부 | Controller path 구현·테스트 PASS |
| M7 dependency | DependencyPrepare/DependencyApply 영수증 status projection | 영수증만으로 canonical manifest/lockfile applied 판정 금지; M4 PatchSet 필요 | Controller path 구현·테스트 PASS |
| M8 migration | MigrationExecute 영수증을 attempt tool observation·permission·Gate와 교차 검증 | failed/partial/outcome_unknown, plan fingerprint/Operation 불일치 거부 | Controller path 구현·테스트 PASS |
| M8 performance | PerformanceRun 영수증을 exact workload subject와 결합 | process/subject evidence 없는 측정 결과 거부 | Controller path 구현·테스트 PASS |
| M8 language cutover | LanguageCutover 영수증과 equivalent report/current Gate를 검증해 `applied` 기록 | `ready_for_*`를 성공으로 사용하지 않음; unknown/partial 거부 | Controller path 구현·테스트 PASS |
| M9 remote recovery | RemoteRecovery 영수증과 exact RecoveryPlan·permission·Gate를 검증해 `applied` 기록 | provider effect 없거나 stale/partial/unknown이면 거부 | Controller path 구현·테스트 PASS |
| M10 GitHub publisher | `ReleaseAssetBindingV1`, draft-first create, no-clobber upload, publish, readback/reconcile, bounded streaming download | source/tag/asset digest·size mismatch, ambiguous timeout, unsigned Stable 거부 | fake provider tests PASS; authenticated draft 왕복 pending |
| Schema/fixture | 두 신규 management Schema와 minimal/full/invalid/future fixture | future/invalid 수용 금지 | generated manifest 188개; Schema gate pending |
| CLI | `tools call`, approval resolve, operation get/cancel, effect receipt record와 receipt-required domain syntax | CLI가 DB/tool/provider를 직접 열지 않음 | 19 unit + 1 evidence test PASS |

## 공통 외부 effect 실행 계약

```text
Tool Registry ready descriptor
  + fixed risk lane
  + exact bounded arguments
  + durable approval / PermissionDecision / GateDecision when required
    -> Controller tool.invoke
    -> terminal durable Operation
    -> development.effect.record
    -> canonical DevelopmentEffectReceiptV1
    -> domain apply/status command
```

영수증은 다음을 fingerprint에 포함한다.

- Project와 exact subject ref/fingerprint
- OperationId, Tool ID, descriptor SHA-256, canonical arguments SHA-256, executable SHA-256
- approval, permission, Gate reference와 effect별 Permission Action
- started/completed 시각, `succeeded|failed|partial|outcome_unknown`, `source_effect_started`
- exact artifact refs, result fingerprint, limitation과 receipt fingerprint

Controller는 저장된 영수증을 읽을 때도 다시 seal하여 fingerprint를 비교한다. 따라서 저장 후 document 변조, stale arguments, 다른 Operation/effect/Project 재사용이 도메인 결과로 침투하지 않는다.

## M10 GitHub 게시 경계

`ReleaseAssetBindingV1`은 ReleaseManifest의 exact source revision/tag/destination과 모든 asset의 local path·remote name·size·SHA-256를 고정한다. GitHub adapter는 `gh` executable identity를 확인하고 다음 순서를 강제한다.

1. target commit에 draft release를 생성하거나 exact existing draft를 read-only reconcile한다.
2. remote asset name 충돌은 overwrite하지 않고 digest/size readback으로 동일 byte인지 확인한다.
3. upload stdout은 bounded capture, asset download는 bounded file streaming으로 처리한다.
4. publish 뒤 release와 모든 asset을 다시 조회하고 exact source/tag/digest/size가 일치할 때만 verified after-state를 반환한다.
5. timeout·partial response는 write 재시도가 아니라 read-only reconcile로 `publish_outcome_unknown`을 해소한다.
6. unsigned Stable은 signing policy에서 publish apply 전 차단한다.

## 검증 원장

| 시각/묶음 | 명령·대상 | 결과 | 증거 |
|---|---|---|---|
| 구현 focused | `cargo check --offline -p star-contracts -p star-execution -p star-controller -p star-cli` | PASS | current terminal log |
| 계약/Controller | `cargo test --offline -p star-contracts -p star-execution -p star-controller` | PASS | contracts 18+30+7+2, execution 3, Controller 86 |
| GitHub/CLI 회귀 | `cargo test --offline -p star-adapter-github -p star-controller -p star-cli` | PASS | GitHub 4, Controller 86, CLI 19+1 |
| Schema 생성 | `cargo run --offline -p star-schema-gen` | PASS | `specs/schemas/manifest.json` 188개 |
| format/Schema/TARGET/FULL/RELEASE | pending | 아직 최종 판정하지 않음 | 후속 실행에서 기록 |
| x64 build/package/lifecycle | pending | 아직 최종 판정하지 않음 | 후속 실행에서 기록 |
| ARM64 cross-build/simulation | pending | 아직 최종 판정하지 않음 | 후속 실행에서 기록 |
| SBOM/audit/provenance | pending | 아직 최종 판정하지 않음 | 후속 실행에서 기록 |
| current Codex 17/17 | pending | source와 installed/session 증거 분리 | 후속 실행에서 기록 |
| GitHub draft upload/readback/cleanup | pending | 공개 Stable과 분리 | 후속 실행에서 기록 |

Windows Cargo incremental finalization에서 간헐적 access-denied warning이 나타날 수 있다. test process의 종료 코드를 우선 기록하되 경고를 숨기지 않으며, 현재 정책상 `target/`을 삭제해 우회하지 않는다.

## 완료 조건

P-0055는 다음이 모두 충족될 때만 `DONE / non-signing external seal`로 바꾼다.

- focused test, generated Schema/fixture, format, lint, TARGET/FULL이 current immutable revision에서 통과한다.
- RELEASE는 failed 0이며 signing/publication 항목만 명시적 `unverified|not_run|blocked_external`이다.
- x64 build/package/disposable lifecycle와 installed/Codex 17/17이 source candidate와 exact digest로 연결된다.
- ARM64 cross-build·PE/manifest·installer model·simulation corpus가 통과하고 상태가 `native_unverified`로 남는다.
- SBOM·cargo audit·pre-sign provenance가 current artifact bytes에 결합된다.
- authenticated GitHub draft가 exact target commit/tag/asset을 upload/readback/hash 검증하고 cleanup 결과를 남긴다.
- branch/commit/push가 exact final source를 원격에서 readback하며 public Stable tag/release는 만들지 않는다.
- STRICT 자체 검토가 approval·permission·Gate 우회, partial/unknown 승격, unbounded IO, secret 출력과 문서 드리프트를 발견하지 않는다.

## 남는 외부 위험

위 조건을 모두 닫아도 Authenticode certificate·private key·trusted timestamp provider가 없으므로 signed Runtime·installer, signed clean install, signed provenance와 public Stable publish는 완료할 수 없다. 이것이 P-0055 종료 뒤 남겨야 하는 유일한 출시 blocker다. 서명 확보 뒤에는 같은 immutable source에서 signed byte를 새 candidate로 만들고 SBOM·provenance·설치·Codex 17/17·GitHub publish/readback을 다시 실행해야 한다.
