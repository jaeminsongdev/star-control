# P-0053 최종 출시 감사 — 2026-07-20 시작, 2026-07-21 재검증

## 판정

**로컬 제품 구현과 비파괴 출시 감사는 완료됐지만 public `v0.1.0` release는 아직 시작할 수 없다.** 현재 P-0040~P-0053 변경은 dirty worktree라 local candidate Gate가 `BLOCK`이고, 이를 commit으로 봉인한 뒤에도 certificate·timestamp provider가 없어 release는 `blocked_external`이다. x64·ARM64 unsigned simulation 산출물은 공개 후보가 아니며 ARM64는 끝까지 `native_unverified`다. signed candidate의 clean x64 installer lifecycle, source candidate 설치 후 core 17/17 재감사, SBOM·provenance와 GitHub action별 승인이 없으므로 `ready|approved|published`로 승격하지 않았다.

원시 수치와 SHA-256은 [`benchmarks/p53-release-audit-x64-arm64.json`](../../benchmarks/p53-release-audit-x64-arm64.json)에 고정한다. `target/`과 `dist/` 산출물은 로컬 evidence이며 source 정본이 아니다.

## 감사 subject

- branch: `codex/p0040-release-policy-alignment`
- base HEAD: `4f01948b5b31198cd45f6f539ef5d1cb3a361e25`
- local candidate ref: `dirty:4f01948b5b31198cd45f6f539ef5d1cb3a361e25:p0053-refresh-20260721T060528Z`
- host: Microsoft Windows 11 Pro `10.0.26200`, x64
- toolchain: Rust `1.96.0`, commit `ac68faa20c58cbccd01ee7208bf3b6e93a7d7f96`

dirty subject는 P-0040~P-0053의 아직 봉인되지 않은 제품 변경을 포함한다. 따라서 이 stage는 구현·simulation 증거일 뿐 Git tag나 build-once public candidate가 아니다. 다른 `D:\개발` 저장소, Codex cache/runtime DB, 실제 사용자 설치와 원격 GitHub 상태는 수정하지 않았다.

## required core와 MCP

current source의 `required_release_core_package_declares_exactly_seventeen_owned_actions`와 `read_only_core_readiness_requires_manifest_handler_and_both_schemas`가 다음 17개 action의 manifest, owning handler, generated input/output Schema를 검사한다.

`goal.start`, `goal.answer`, `plan.get`, `plan.update`, `run.continue`, `status.get`, `goal.pause`, `goal.resume`, `goal.cancel`, `evidence.get`, `merge.status`, `handoff.get`, `doctor`, `project.list`, `project.status`, `validation.plan`, `validation.run`.

현재 Codex가 연결한 installed runtime은 registry revision 4, snapshot `sha256:2f5dfee8bf746dbc07320e7d2881cf4ff347ae3669f20328b4b9ab1a7b35ee90`이며 `doctor`, `evidence.get`, `project.list`, `project.status`, `validation.plan`, `validation.run` 여섯 action만 ready다. 여섯 action은 current Codex에서 search→describe→invoke를 통과했고 최신 `validation.run` FULL operation `opn_01KY1M9RNEM0052W730W3SN9TH`가 10/10 PASS했다. source의 새 11개 action을 이 과거 설치본의 실행 성공으로 추측하지 않는다.

공식 MCP Inspector `0.22.0`을 cached exact package에서 직접 실행해 installed `star-mcp.exe`의 `tools/list`를 다시 읽었다. fixed tool 12개, 모든 input/output Schema가 존재했고 canonical projection hash는 `sha256:b193ad92f3c5a85854e51788e11bba7b244102e557121a43d15930f86dafe690`이다. active installed Controller를 중단·교체하지 않았으므로 source candidate Inspector와 installed core 17/17 증거는 signed clean-install Gate에 남긴다.

## x64 Stable local evidence

M11 persisted pre/post Gate와 scope/config TOCTOU 보강 뒤 `cargo build --workspace --release --target x86_64-pc-windows-msvc --locked`와 package verifier를 다시 통과했다. immutable package 증거 stage `dist/stage/p0053-refresh-20260721T060528Z/x64-pristine`는 manifest file 279개, set digest `sha256:ee64250702fa053193cad7789b17da237bb8add9b0d04912cbb56ef23704b99c`, manifest digest `sha256:c7817459dd94516b56308907daf631d38e8fcd825f699da3a3bca169fe464dbf`이며 네 Runtime EXE의 PE machine은 모두 `0x8664`다.

pristine manifest가 byte-for-byte 같은 `target/p0053-x64-lifecycle-copy-20260721T060528Z/stage`를 완전히 격리된 `APPDATA`, `LOCALAPPDATA`, `USERPROFILE`에서 실행해 다음을 확인했다. Bridge가 Runtime Generation을 생성하는 mutable lifecycle copy와 immutable package stage를 섞지 않는다.

1. `installation finalize --architecture x64 --replace-existing --json` PASS
2. `installation bridge initialize --state-generation p0053_pristine_copy_20260721 --json` PASS
3. `installation status --json`의 `verified=true`
4. lifecycle 시작 전 release manifest hash `sha256:c7817459dd94516b56308907daf631d38e8fcd825f699da3a3bca169fe464dbf`
5. active Runtime Generation release manifest hash `sha256:9172673ed322edcefa9da9c90d227524a86d537ae14dc51e5641034b3ff5ecd4`와 activation record 선언이 일치

`star-release` lifecycle reducer와 updater/adapter fault test는 install→first run→update stage→injected failure→rollback→repair→uninstall의 순서, 이전 artifact 복구와 user-data digest 보존을 검사한다. pristine stage에서 Inno Setup `6.7.3` model installer(`sha256:88bc4a9a47694142b3346c386c85e23b71f7ba011cfb18adb4db55c7a7cdf9a0`)도 생성했지만 unsigned이고 현재 Codex가 실행 중인 host에서 actual install/uninstall을 수행하지 않았으므로 clean signed installer lifecycle evidence가 아니다.

## ARM64 Preview simulation

이미 설치된 exact Rust 1.96 toolchain의 `aarch64-pc-windows-msvc` standard library를 사용했다. release cross-build 자체는 package나 target을 설치하지 않았다. `scripts/release/cross-build-arm64.ps1`의 전체 workspace cross-build와 nested multi-crate Rust corpus ARM64 `cargo check`·Clippy, package verify, file manifest, Inno installer model과 fake lifecycle이 통과했다.

- stage: `dist/stage/p0053-refresh-20260721T060528Z/arm64`
- manifest file count: 279
- set digest: `sha256:bbb659c44090b06f9b1fc612b166083f04b8ba1f5941c99965d843b501a90c94`
- 네 Runtime EXE PE machine: 모두 `0xaa64`
- installer model digest: `sha256:e2bdf0d7a80f7b649cc02f98a293e591fb7f3bda00160510e1c6c684ebb1f8b6`
- runtime verification: `native_unverified`

Inno Setup bootstrap 자체의 PE machine `0x014c`는 installer payload target 판정이 아니다. ARM64 판정은 manifest의 target architecture와 포함된 네 Runtime EXE의 `0xaa64`를 사용한다. ARM64 process·IPC·설치 성공은 실행하거나 주장하지 않았다.

## signing과 publication Gate

두 stage와 두 installer는 모두 `NotSigned`다. current-user code-signing private certificate는 0개, `signtool.exe`는 없고 timestamp provider도 설정되지 않았다. immutable stage를 별도 `x64-signing-negative`·`arm64-signing-negative`에 복제하고 FULL input fingerprint `d971198b7ee3f2c1afca8cb1db6b34271c188b584bf84473b0ef49eef2960af5`로 reseal한 뒤 `seal-signed`를 실행했다. 두 호출 모두 exit 2 `Authenticode verification failed`로 중단됐고 top/nested manifest hash는 바뀌지 않았다. 이 negative Gate는 unsigned byte를 `signed`로 거짓 봉인하지 않음을 증명한다.

제품 경계는 Runtime EXE 서명 → `seal-signed` 새 stage digest → installer build → installer 서명 → 최종 digest/release Gate 순서를 강제한다. `seal-signed`는 pre-sign file inventory와 nested source revision을 보존하며 Windows trust를 검사하지만 approved certificate·timestamp receipt는 별도 `signature_refs`가 소유한다.

Git tag, GitHub draft, asset upload, publish와 remote read-back은 실행하지 않았다. exact action 승인이 주어지더라도 publish timeout은 write 재시도하지 않고 read-only reconcile하며, remote asset digest가 일치하기 전에는 `published`가 아니다.

## 최종 검증

- workspace FULL: operation `opn_01KY1M9RNEM0052W730W3SN9TH`, `target/validation/20260721T060240092Z-3220/report.json`, 10/10 complete·stable PASS, 114.2초, `sha256:0606fd2f4adc684af04b2adf4d7a5d368a0f9d4f06525e1427ee13a8bd1c3d69`
- release profile: operation `opn_01KY1MVADFM1KX6XB5PASCCBCJ`, `target/validation/20260721T061214465Z-31000/report.json`, 15개 중 13개 PASS, 104.2초. `release-clean-worktree`는 dirty 변경 때문에 FAIL, `release-external-signing-publication`은 external evidence 부재 때문에 UNVERIFIED이며 report hash는 `sha256:0e53bc8c0ce48987c2ecdabe88f0d58d0a57fe8b07c40667694ed37ed330e13b`다.

release 결과의 실패·미검증을 통과로 포장하지 않는다. 첫 항목은 P-ID local commit seal이 해소하고, 둘째는 아래 외부 Gate가 해소한다.

## 제외한 로컬 산출물

- 첫 lifecycle 시도 경로 `dist/stage/p0053-refresh-20260721T042805Z/x64`는 Bridge가 runtime files를 생성해 inventory가 달라졌으므로 package 증거에서 제외했다. 삭제·덮어쓰기 없이 `x64-pristine`을 새로 만들었다.
- 격리 lifecycle process에서 `USERPROFILE`을 바꾼 뒤 cargo verifier를 호출해 exact Rust 1.96 toolchain이 `target/p0053-x64-lifecycle-copy-20260721T042805Z/userprofile/.rustup`에 bootstrap됐다. 177 files, 693,271,047 bytes이며 product·package·lifecycle 증거로 사용하지 않는다. `target/` 정리 금지와 삭제 승인 경계 때문에 제거하지 않았다. system toolchain과 source는 변경하지 않았다.

## 남은 외부 Gate

1. P-0040~P-0053 변경을 검증된 local commit으로 봉인해 clean immutable source revision 생성
2. certificate·timestamp provider와 비용 승인 후 x64·ARM64 Runtime 및 installer signed candidate 생성
3. signed x64 candidate의 disposable clean Windows install·first run·update·failure rollback·repair·uninstall/user-data 보존
4. source candidate 설치 후 current Codex와 Inspector에서 required core 17/17 search·describe·invoke 재감사
5. SBOM·provenance와 signature receipt 완성
6. exact manifest·digest·GitHub destination 승인, action별 tag/draft/upload/publish 승인과 remote digest reconciliation

이 항목은 문서 누락이나 구현 성공으로 가장할 수 있는 항목이 아니라 외부 certificate·clean environment·remote effect 승인에 종속된 출시 Gate다.
