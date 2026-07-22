# P-0053 최종 출시 감사 — 2026-07-20 시작, 2026-07-23 clean 후보 재검증

## 판정

**P-0040~P-0053 구현은 clean local source revision으로 봉인됐고, 공개 출시 직전의 로컬·simulation Gate도 모두 닫혔다. public `v0.1.0`은 신뢰 Authenticode 인증서와 timestamp provider가 없으므로 계속 `blocked_external`이다.** unsigned x64 Stable로 낮추지 않으며 ARM64 Preview는 끝까지 `native_unverified`다. signed byte의 clean installer lifecycle, current Codex 통합과 GitHub 원격 digest reconcile이 없으므로 `ready|approved|published`로 승격하지 않는다.

원시 수치와 SHA-256은 [`benchmarks/p53-release-audit-x64-arm64.json`](../../benchmarks/p53-release-audit-x64-arm64.json)에 고정한다. `target/`과 `dist/` 산출물은 exact local evidence이며 source 정본이 아니다.

## 감사 subject

- branch: `codex/p0041-p0053-completion`
- source revision: `ac3ca70ff4c7c8827a892fad1b55ba22cb72bfac`
- candidate: `p0053-final-ac3ca70`
- source state at build and validation: clean
- host: Microsoft Windows 11 Pro `10.0.26200`, x64
- toolchain: Rust `1.96.0`, `rustfmt`·Clippy·`rust-analyzer`·`rust-src` pinned component

이 후보는 P-0040 정책 commit `416ed3e`, P-0041~P-0053 구현·Schema·fixture·문서 chain, clean-profile binding과 Windows CI에서 드러난 Rust semantic URI·project allowlist fixture·long-poll scheduler 경계 수정까지 포함한다. 다른 `D:\개발` 저장소, Codex runtime DB·plugin cache와 실제 설치 파일은 수정하지 않았다.

## required core와 MCP

source test는 required core 17개 각각에 대해 manifest action, concrete Controller handler, generated input/output Schema가 함께 있을 때만 `ready`임을 검사한다.

`goal.start`, `goal.answer`, `plan.get`, `plan.update`, `run.continue`, `status.get`, `goal.pause`, `goal.resume`, `goal.cancel`, `evidence.get`, `merge.status`, `handoff.get`, `doctor`, `project.list`, `project.status`, `validation.plan`, `validation.run`.

공식 MCP Inspector `0.22.0` cached exact package를 candidate stage의 `star-mcp.exe`와 `star-controller.exe`에 직접 연결했다. fixed Gateway tool 12개와 fully resolved Schema가 통과했고, release source·`ready` filter 검색은 위 17개를 정확히 반환했다. 이어 17개를 각각 describe해 `descriptor_hash`, `required_call_tool`, risk lane과 input/output Schema를 확인했다. 증거는 `dist/release-evidence/p0053-final-ac3ca70/mcp-inspector-core17.pre-sign.json`, SHA-256 `2bc1fe4f2dc3f6a27274a1e146c762289827edfd8a76d837a2a338032568413f`다.

이 Inspector run은 exact candidate binary와 release Catalog의 실행 증거지만 signed installer나 current Codex host 설치 증거는 아니다. 현재 Codex가 사용하는 기존 설치본은 registry revision 4와 ready action 6개를 유지한다. candidate 검사 동안 같은 SID의 Controller를 bounded하게 교체했고, 종료 후 설치된 `D:\도구\Star-Control` Controller를 verified start 경로로 복구해 `installation status verified=true`를 재확인했다.

## x64 Stable local evidence

`dist/stage/p0053-final-ac3ca70/x64`는 manifest 279파일, set digest `sha256:eec02f5539b79e320ed22b1f606bb881f46ba488491bc8b52a6135174fcdea70`, manifest digest `sha256:c16b93df40f58795b56f95c95aa6a6cc4cc726e4c6da856f670947e4969f2f5e`다. 네 root Runtime EXE의 PE machine은 모두 `0x8664`이고 package verifier가 source revision `ac3ca70...`와 file set을 재검증했다.

candidate를 `target/p0053-final-ac3ca70-lifecycle/program/x64`에 복제하고 격리된 `APPDATA`, `LOCALAPPDATA`, `USERPROFILE`에서 다음을 확인했다.

1. `installation finalize --architecture x64 --replace-existing --json` PASS
2. `installation bridge initialize --state-generation p0053_final_ac3ca70_bootstrap --json` PASS
3. `installation status --json`의 `verified=true`
4. active generation `rt_3fad4b7d6b66e83d`, Bridge contract v2
5. Runtime release manifest `sha256:8bd05f1c4ebd1d244184a4fda9a2bd8fba93f1bd7e40aa11d431f459594037e0`

Inno Setup model installer는 `target/p0053-final-ac3ca70-installer-model/star-control-windows-x64-0.1.0-setup.exe`, 14,704,281 bytes, SHA-256 `60c80abdee20f6146dbfd0045631e708bf74faba670bd208fe0f0cc56be67fe2`다. model과 Runtime은 모두 `NotSigned`이므로 public candidate가 아니다.

## ARM64 Preview simulation

exact Rust 1.96의 `aarch64-pc-windows-msvc` target으로 workspace cross-build, nested multi-crate Rust corpus check·Clippy, package verify, file manifest, installer model과 fake lifecycle을 실행했다.

- stage: `dist/stage/p0053-final-ac3ca70/arm64`
- manifest file count: 279
- set digest: `sha256:48080668afff473a30ee0f2090654c2b5265691331db5430b1b5e5ff52861560`
- manifest digest: `sha256:5d36281d0d7c1facd942e1b4106143c38fb14175e66a70185ae801160a7280f8`
- 네 root Runtime EXE PE machine: 모두 `0xaa64`
- installer model: 13,791,989 bytes, `sha256:067acf12f4cf9a31c48c1ee555b306fae64fc00edd8a818a7a6a38fcf79fdf21`
- runtime verification: `native_unverified`

Inno bootstrap 자체의 host PE machine은 payload target 판정이 아니다. ARM64 process·IPC·Controller·CLI·MCP·native install 성공은 실행하거나 주장하지 않았다.

## 공급망·보안 evidence

`syft 1.45.0`으로 x64·ARM64 candidate stage의 named SPDX JSON을 만들었다. 각각 7 packages를 식별했고 SHA-256은 `bfcc718de1771c9ad494d8cc3bf807db533300179e873b008f4d7a94aa6f4c8a`, `4112544d3d5fff27ff6cb44ca22f6ceb7b321ceb0c88ed85397d84cfba6b62f9`다. `cargo audit --deny warnings --json`은 current RustSec DB로 222 dependencies를 검사해 vulnerability 0, warning 0으로 끝났고 evidence SHA-256은 `828df9bf88877683e849f76f2b7145be94327b140f55adcfe24d87bf9fd91e60`이다.

`provenance.pre-sign.json`은 source revision, Cargo/toolchain/release-policy material, 두 stage set·manifest·installer model·SBOM digest와 clean FULL/release/PR Gate, GitHub Actions success를 연결한다. SHA-256은 `bc1209edab4eb0cedcd9b0f801148142b59330d11624451904acf57ab8cf5cb9`이며 PE/signature inventory SHA-256은 `55fd7ef8c78919ffb55d622ca30c77ffa76f2862500982a5eb27507f7c996dac`다. 이 자료는 명시적으로 `public_release_eligible=false`, `must_regenerate_after_signing=true`이며 signed final artifact provenance로 재사용하지 않는다.

## signing과 publication Gate

CurrentUser와 LocalMachine store의 code-signing private certificate는 각각 0개다. `signtool.exe`는 PATH에는 없지만 Windows SDK `C:\Program Files (x86)\Windows Kits\10\bin\10.0.28000.0\x64\signtool.exe` 등에서 확인했다. 그러나 승인된 signer identity·private key·timestamp provider가 없으므로 공개 서명은 실행할 수 없다.

두 pristine stage에서 `seal-signed`를 직접 실행하면 모두 exit 2 `Authenticode verification failed`로 중단되고 top/nested manifest hash가 그대로 유지된다. 제품 경계는 Runtime EXE 서명 → `seal-signed` 새 stage digest → installer build → installer 서명 → final digest·SBOM·provenance·release Gate 순서를 강제한다.

Git tag, GitHub draft, asset upload와 publish는 실행하지 않았다. unsigned artifact를 올려 Stable로 낮추는 대신 release status를 `blocked_external`로 유지한다. publish timeout은 write 재시도하지 않고 read-only reconcile하며 remote asset digest가 일치하기 전에는 `published`가 아니다.

## clean 검증

- FULL: `target/validation/20260722T183736373Z-23820/report.json`, 10/10 complete PASS, 74.2초, `sha256:059466b8c24911c70640192af8aed995933e0cde62840fc7e096fdc2050a4df4`
- release: `target/validation/20260722T183855005Z-11592/report.json`, 15개 중 14개 PASS, failed 0, 91.4초, `sha256:1ce7c517f815ddf60461f15eca3802b6f355de3ff0ce6efef6557ea4d44bab0d`
- PR 전체 변경 Gate: `target/validation/20260722T184037675Z-24808/report.json`, 11/11 PASS, `sha256:0f374a379e20d29f9bfcdc911a80077fa256a66a5d62cc1d51146ee5bc812a99`
- GitHub Actions: workflow run `29947378549`, job `89016087886`, run #24 `Native validation` PASS
- release의 유일한 non-pass는 `release-external-signing-publication`: `unverified/not_run`
- `release-clean-worktree`: PASS

source validation runner는 certificate, signed installer lifecycle과 GitHub publication을 만들지 않는다. 따라서 외부 evidence 부재를 통과로 포장하지 않는다.

## 남은 외부 Gate

1. 승인된 Authenticode certificate·private key·timestamp provider로 x64·ARM64 Runtime EXE를 서명하고 `seal-signed`로 새 candidate를 만든다.
2. 그 stage에서 installer를 한 번 만들고 installer 자체를 서명한 뒤 final digest·SBOM·provenance를 다시 계산한다.
3. signed x64 candidate의 disposable clean Windows install·safe-default first run·supported update·injected failure rollback·repair·uninstall과 user-data 보존을 검증한다.
4. signed candidate를 실제 Codex integration 경로에 설치해 current Codex에서 required core 17/17 search·describe·invoke를 재감사한다.
5. exact manifest·digest·GitHub destination과 각 tag/draft/upload/publish action을 승인한 뒤 remote asset digest를 read-back한다.

이 다섯 항목은 후속 제품 구현 누락이 아니라 신뢰 credential, signed byte, clean 외부 환경과 원격 효과에 종속된 release Gate다.
