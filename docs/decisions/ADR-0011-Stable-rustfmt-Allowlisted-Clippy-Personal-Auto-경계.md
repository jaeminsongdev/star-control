# ADR-0011: Stable rustfmt·Allowlisted Clippy·Personal Auto 경계

## 상태

- 상태: 채택
- 결정일: 2026-07-14
- 적용 단계: M11 Rust 코드 스타일 자동 교정 Profile
- 구현 상태: P-0052 bounded 제품 Slice 구현. 실제 사용자 source에 대한 CLI orchestration·apply는 후속 경계

## 맥락

Star-Control에는 external formatter/codemod를 live checkout과 분리하고 immutable PatchSet·pre/post Gate로 적용하는 일반 계약이 있지만 Rust workspace의 toolchain/config/style edition, Clippy suggestion applicability와 package/target/feature/cfg coverage를 고정한 Profile은 없었다.

단순 `cargo fmt && cargo clippy --fix` wrapper는 다음을 증명하지 못한다.

- 어떤 cargo/rustfmt/Clippy executable과 style edition/config가 실행됐는지
- inactive feature, 다른 target/cfg와 mutually exclusive feature를 얼마나 검사했는지
- Clippy가 바꾼 hunk가 사전 허용된 exact lint의 `MachineApplicable` suggestion인지
- build script/proc macro가 source root를 추가로 수정하지 않았는지
- target checkout의 사용자 변경을 보존했는지
- prepare candidate와 실제 apply/post 검증이 같은 PatchSet인지

Rust Style Guide와 stable rustfmt는 기본 formatting 정본을 제공하지만 lint level은 Cargo `[lints]`/`[workspace.lints]`와 source attribute가 소유한다. `clippy.toml`은 lint parameter 정본이지 자동 수정 권한 목록이 아니다. Clippy lint group에는 project별 trade-off와 상호 모순이 있을 수 있고 `cargo clippy --fix` coverage도 active target/feature/cfg에 제한된다.

개인 사용자의 `personal_auto` 목표는 안전한 candidate를 반복 prompt 없이 적용하는 것이지만, 기존 M4의 exact PatchSet fingerprint 승인과 M3 Gate를 우회해서는 안 된다.

## 결정

1. C01의 최종 16번째 Profile ID를 `rust_style_auto_fix`, fixed pipeline ID를 `rust_style_v1@1`로 한다.
2. 공식 stable cargo fmt/rustfmt와 Clippy만 사용한다. nightly·unstable rustfmt option과 moving/unresolved toolchain은 자동 적용에서 거부한다.
3. automatic apply에는 project-pinned stable toolchain을 요구하고 cargo, rustc, rustfmt, clippy-driver version·opaque executable file identity·redacted locator·full hash, parsing/style edition, MSRV, host/target와 config source를 fingerprint한다. final absolute path는 process memory 밖에 저장하지 않는다.
4. formatting은 cargo fmt를 우선한다. style edition은 parsing edition과 별도로 resolve하며 resolved 값과 source를 PatchSet·Evidence에 bind한다.
5. lint level 정본은 Cargo manifest와 source attribute, lint별 parameter 정본은 `clippy.toml`/`.clippy.toml`이다. StarConfig·DB·Catalog가 이 값을 복제하거나 override하지 않는다.
6. Clippy automatic fix는 versioned project/user Catalog policy의 exact lint ID allowlist, `MachineApplicable` applicability와 actual hunk-to-suggestion byte 대응을 모두 요구한다. group·wildcard는 허용하지 않는다.
7. v1 built-in exact fix allowlist는 exact Clippy version과 Corpus evidence가 없으면 빈 list다. “일반적으로 안전하다”는 추측으로 lint를 code에 하드코딩하지 않는다.
8. `#[allow]`·lint level suppression을 자동 추가·삭제하지 않고 `clippy::pedantic`, `clippy::restriction`, `clippy::nursery` 등 group 전체를 자동 수정 대상으로 활성화하지 않는다.
9. package/target/feature/triple/cfg/required-feature/generated ownership별 coverage matrix를 만든다. `--all-features`를 범용 기본값으로 사용하지 않고 project Catalog가 compatible하다고 선언한 feature set만 stable order로 실행한다.
10. `cargo fmt`, `cargo clippy --fix`와 Clippy build script/proc macro 실행은 trusted Project의 Star-Control-owned isolated preview에서만 수행한다. live target에 external mutator를 실행하지 않고 network는 기본 거부하며 `CARGO_TARGET_DIR`은 source 밖 owned path를 사용한다.
11. fixed order는 resolve → current check → scope → preview → rustfmt → allowlisted Clippy fix → rustfmt → complete diff/side-effect → impact reconcile → full replay no-op → candidate Check → immutable PatchSet → exact policy approval → pre Gate/single-use permit → M4 apply → post Gate → Evidence다.
12. final PatchSet은 handwritten in-scope `.rs` modify만 허용한다. Cargo/lock/config/toolchain, generated/vendor/out-of-scope/public surface, create/delete/rename와 unmatched Clippy hunk는 candidate 전체를 거부한다.
13. `cargo clippy --fix --allow-dirty`는 staged byte 0과 직전 rustfmt dirty manifest가 byte-exact 일치한 isolated preview에서만 쓸 수 있다. `--allow-staged`, `--broken-code`, `--allow-no-vcs`는 사용하지 않는다.
14. full-pipeline idempotence replay는 expected-after의 새 preview에서 operation 0을 요구한다. partial/unverified coverage, conflicting feature/target suggestion, tool/config/source drift와 post failure를 success로 표시하지 않는다.
15. `safe_default`는 inspect/check/prepare 뒤 exact PatchSet 사용자 승인을 요구한다.
16. `personal_auto` standing grant는 Project/Profile/pipeline/style policy/scope/action/diff/Gate/expiry ceiling만 제공한다. prepare 뒤 policy evaluator가 exact PatchSet fingerprint와 evidence에 대해 기존 ApprovalRequest를 `decision=approved`, `resolved_by=policy_evaluator`로 해소한 뒤 M3 `patch_pre_apply=AUTO_PASS`와 single-use M4 permit을 거친다.
17. prepare와 apply는 별도 state transition·event·ID·evidence를 유지한다. `auto-apply`도 기존 SourceMutationPort·PatchApplication·post Gate·recovery를 사용한다.
18. 새 runtime executable, formatter/parser/AST/LSP engine, AI/OpenAI/browser UI, 자체 scheduler/watcher, raw shell pipeline, `cargo fix`·edition/MSRV/dependency migration을 M11에 추가하지 않는다.
19. 새 mutable Rust run truth나 DB source를 만들지 않고 existing RecipeExecution·PatchSet·PatchApplication·ValidationRun·EvidenceBundle에 4개 versioned nested type을 연결한다.

상세 type, pipeline, coverage, 오류, CLI, Corpus와 Package 소유권은 [M11 의미 정본](../features/rust-code-style-auto-fix.md)이 소유한다.

## 결과

- AI 호출 없이 `star.exe`에서 Rust style 검사·preview·apply·recovery를 수행할 수 있는 구현 기준이 생긴다.
- 공식 Rust 도구를 재사용하면서 실행 identity·coverage·suggestion·diff·Gate의 재현성을 Star-Control이 보증한다.
- formatting/lint/toolchain Git source와 Catalog policy, DB derived state의 경계가 유지된다.
- 사용자 dirty byte와 live checkout이 external mutator에서 분리된다.
- `personal_auto`가 prompt를 줄이되 exact PatchSet 승인·M3/M4 안전 경계를 약화하지 않는다.
- core 기능 23개와 runtime executable 4개 구조를 유지한다.

## 기각한 대안

- **live checkout에서 cargo fmt/Clippy fix 실행**: 사용자 byte·undeclared effect를 Patch candidate와 분리할 수 없어 기각한다.
- **항상 `--all-features` 사용**: mutually exclusive feature와 실제 target/cfg coverage를 왜곡해 기각한다.
- **Clippy group 전체 allowlist**: exact fix intent와 상호 모순 lint를 통제할 수 없어 기각한다.
- **MachineApplicable이면 모두 자동 적용**: project lint policy·scope·version·public/side-effect 판단이 빠져 기각한다.
- **Clippy suggestion text만 믿고 diff 생략**: actual tool hunk·build script effect를 검증할 수 없어 기각한다.
- **`clippy.toml`을 자동 적용 권한 정본으로 사용**: lint parameter와 product permission의 책임을 섞어 기각한다.
- **DB에서 lint/style 설정 편집**: Git source-of-truth와 rebuildability를 깨뜨려 기각한다.
- **standing grant를 reusable patch approval로 사용**: prepare 뒤 생기는 exact candidate를 bind하지 못해 기각한다.
- **`cargo fix`와 edition migration 포함**: style correction과 compiler/migration 책임이 달라 기각한다.
- **Rust 전용 executable/engine 추가**: 기존 Registry·M4·M3 경계를 중복하므로 기각한다.

## 관련 정본

- [Rust 코드 스타일 자동 교정 Profile](../features/rust-code-style-auto-fix.md)
- [개발 작업 Profile](../features/profiles.md)
- [설정과 Catalog](../contracts/config-and-catalog.md)
- [외부 Tool Registry](../contracts/external-tool-registry.md)
- [안전한 Patch·Refactor·codemod](../contracts/safe-patch-and-codemod.md)
- [공통 검증·품질 Gate](../features/common-validation-gate.md)
- [검사·완료·증거](../contracts/validation-and-evidence.md)
- [승인·권한·안전](../architecture/security-and-permissions.md)
