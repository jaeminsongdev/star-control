# Rust 코드 스타일 자동 교정 Profile

## 1. 상태와 문서 소유권

이 문서는 Star-Control의 16번째 C01 작업 Profile인 `rust_style_auto_fix`와 고정 pipeline `rust_style_v1`의 의미 정본이다. 현재 상태는 **M11 설계 확정 대상, 제품 구현 전**이다. 이 문서가 존재한다는 사실은 Rust type, Schema, Catalog TOML, fixture, Cargo/rustfmt/Clippy adapter, CLI command, Patch engine 또는 자동 적용 code가 구현됐다는 뜻이 아니다.

이 Profile은 다음 기존 정본을 조합한다.

| 책임 | 소유 정본 |
|---|---|
| Cargo Project·workspace·package·target·feature 발견과 source 분류 | [Project Catalog·Code Index](../contracts/project-catalog-and-code-index.md) |
| TaskSpec·package/workspace scope·영향·affected Check | [변경 계획·영향 분석](../contracts/change-planning-and-impact.md) |
| `AUTO_PASS`, current/complete evidence와 Patch 전·후 Gate | [공통 검증·품질 Gate](common-validation-gate.md) |
| immutable RecipeExecution·PatchSet·PatchApplication·recovery | [안전한 Patch·Refactor·codemod](../contracts/safe-patch-and-codemod.md) |
| Tool·Check·Profile·정책 metadata와 source provenance | [설정과 Catalog](../contracts/config-and-catalog.md) |
| cargo·rustfmt·Clippy typed process 실행 | [외부 Tool Registry](../contracts/external-tool-registry.md), [Manifest Reference](../contracts/tool-package-manifest-reference.md) |
| evidence binding·ValidationRun·EvidenceBundle·ReviewPack | [검증·증거](../contracts/validation-and-evidence.md) |
| stable reason code와 Diagnostic mapping | [오류·진단](../contracts/errors-and-diagnostics.md) |
| exact scope 자동 승인·trusted process 경계 | [승인·권한·안전](../architecture/security-and-permissions.md) |
| 격리 preview worktree와 live target 분리 | [Worktree·병합](../architecture/worktrees-and-merge.md) |
| Package·Catalog·Corpus의 물리 소유 위치 | [Repository·Package 구조](../architecture/repository-layout.md) |

M11은 새 최상위 persisted document나 별도 mutable run record를 만들지 않는다. `RecipeExecution`, `PatchSet`, `PatchApplication`, `ValidationRun`, `GateDecision`, `EvidenceBundle`과 `ReviewPack`에 이 문서의 nested versioned type을 연결한다. DB는 derived projection이며 Git source·Cargo/rustfmt/Clippy config·versioned Catalog policy가 계속 정본이다.

2026-07-20 현재 Star-Control 저장소 자체의 root `Cargo.toml`은 workspace, `edition = "2024"`, `rust-version = "1.96"`이지만 `rust-toolchain.toml`은 실제로 존재하지 않는다. 최종 구조 문서에 그 파일이 적혀 있다는 이유로 현재 구현 사실이나 pinned toolchain으로 간주하지 않으며, M11 문서 작업에서 해당 파일을 생성하지 않는다.

## 2. 목표와 사용자 효용

`rust_style_auto_fix`는 사용자가 AI 없이 `star.exe`에서 공식 stable Rust toolchain을 사용해 Rust style drift를 검사하고, 검증된 candidate만 안전하게 적용하게 한다.

목표는 다음과 같다.

1. Rust workspace의 rustfmt drift와 Clippy Diagnostic을 current source 기준으로 수집한다.
2. live checkout이 아닌 Star-Control-owned isolated preview workspace에서 rustfmt와 허용된 Clippy fix를 실행한다.
3. step별 diff와 complete filesystem diff를 수집해 `.rs` modify만 candidate로 허용한다.
4. exact lint ID·`MachineApplicable` suggestion·coverage matrix와 각 Clippy hunk의 대응을 증명한다.
5. M2 영향·검사 계획, M3 pre/post Gate와 M4 immutable PatchSet·reverse 자료를 재사용한다.
6. `safe_default`는 prepare까지만 자동 진행하고, 사용자가 명시적으로 선택한 `personal_auto`는 exact 정책 scope와 모든 검증의 `AUTO_PASS`가 있을 때 prompt 없이 적용할 수 있다.
7. toolchain·config·ToolDescriptor·coverage·source가 바뀌거나 결과가 partial/unverified이면 target source를 건드리지 않고 중단한다.
8. 실행·진단·Patch·Evidence projection을 source-of-truth로 승격하지 않고 Git source/config에서 재구축 가능하게 한다.

이 Profile은 단순히 `cargo fmt && cargo clippy --fix`를 live checkout에서 연속 실행하는 wrapper가 아니다. 고정된 도구를 호출하되 실행 전·후 subject, step별 side effect, coverage, policy와 Patch lineage를 Controller가 검증하는 application workflow다.

## 3. 선행 제품 Gate

M11 source mutation 구현은 다음 제품 Gate가 실제 code·Schema·fixture·CLI-only E2E로 통과한 뒤에만 시작한다.

| 선행 | 필요한 current 결과 | 미충족 시 허용 범위 |
|---|---|---|
| M1 | current ProjectCheckout·Cargo workspace/package/target graph, source class·generated ownership·freshness | static Catalog 조회와 제한된 `inspect` 설계만 가능 |
| M2 | accepted TaskSpec·ScopeRevision, ChangePlan과 `readiness=ready` ValidationPlan | candidate impact·affected Check를 확정하지 못하므로 `prepare` 금지 |
| M3 | exact EvidenceSubjectBinding, `AUTO_PASS`, `patch_pre_apply`·`patch_post_apply`, complete EvidenceBundle | SourceMutationPort permit 발급 금지 |
| M4 | RecipeExecution·PatchSet·PatchApplication, isolated preview, idempotence, reverse/recovery | live source apply 금지 |

제품 구현 순서는 `M1 -> M2 -> M3 -> M4 -> M11`이다. 문서용 fake input이나 historical P0 Patch/Validation result로 이 Gate를 통과한 것처럼 표시하지 않는다. M11 read-only discovery pure function을 먼저 구현할 수는 있지만 process 실행·candidate 생성·source mutation을 조기에 연결하지 않는다.

## 4. 제외 범위

M11은 다음을 지원하지 않는다.

- 별도 Rust formatter, parser, AST rewrite engine, LSP, compiler 또는 Clippy 재구현
- `star-rust-style.exe` 같은 다섯 번째 runtime executable
- local AI, 다른 AI provider, OpenAI API 직접 호출, embedding, browser UI와 자체 예약 실행·watcher
- 사용자 정의 shell pipeline, command string, PowerShell·`cmd` script 또는 PATH 첫 executable 실행
- nightly toolchain, unstable rustfmt option과 nightly-only style edition의 자동 적용
- `cargo fix`, `cargo fix --edition`, `cargo fix --edition-idioms`, edition/MSRV/platform migration
- `clippy::pedantic`, `clippy::restriction`, `clippy::nursery` 또는 다른 group 이름 자체를 자동 fix allowlist로 사용
- lint suppression·`#[allow]`·`#[warn]`·`#[deny]`의 자동 추가·삭제·이동
- `Cargo.toml`, `Cargo.lock`, `rustfmt.toml`, `.rustfmt.toml`, `clippy.toml`, `.clippy.toml`, `rust-toolchain.toml` 변경
- dependency·component·target 설치, network download와 toolchain update
- generated/vendor source, 문서·Schema·설정·migration file 수정
- file create/delete/rename, `.rs` 이외 file 변경, public API·edition·MSRV·dependency 변경
- live checkout에서 `cargo fmt`, `cargo clippy --fix` 또는 다른 external mutator 직접 실행
- 둘 이상의 Project를 한 PatchSet으로 수정하거나 merge·commit·push·release까지 완료로 묶는 기능

위 제외 대상 변경이 필요하면 별도 TaskSpec·ChangePlan과 해당 Profile을 사용한다. 예를 들어 edition 전환은 `language_platform_migration`, lint level 변경은 `api_contract_change` 또는 `docs_config_environment`, dependency 변경은 `dependency_upgrade`가 소유한다.

## 5. 공식 Rust 도구 조사 근거

공식 자료는 2026-07-14에 다시 확인했다. 설계에 반영한 결론은 다음과 같다.

| 공식 사실 | M11 적용 결정 |
|---|---|
| Rust Style Guide가 default Rust style을 정의하고 rustfmt가 이를 구현 기준으로 사용한다. | formatting 기본은 별도 사내 규칙이 아니라 stable rustfmt와 project rustfmt config다. |
| `cargo fmt`는 toolchain의 optional rustfmt component이며 Cargo project의 bin/lib source를 format한다. | source rewrite의 canonical entry는 direct `rustfmt`가 아니라 `cargo fmt`다. component가 없으면 설치하지 않고 unavailable이다. |
| rustfmt option은 version에 따라 달라질 수 있고 stable/unstable이 구분된다. | rustfmt version·config source·resolved option set을 fingerprint하고 unstable option은 auto apply에서 거부한다. |
| RFC 3338은 parsing edition과 style edition을 분리하며 `style_edition`이 styling에서 우선할 수 있다고 규정한다. | parsing edition, resolved style edition, 각 값의 source를 별도 field로 보존한다. |
| Clippy는 Cargo subcommand로 실행하고 project가 `#[allow(...)]`로 lint를 끄는 것을 정상적인 선택으로 인정한다. | source lint attribute를 정본으로 존중하며 suppression을 자동 교정하지 않는다. |
| Clippy `--fix`는 일부 suggestion을 자동 적용하고 `--all-targets`를 암묵적으로 사용한다. | `--all-targets`를 coverage complete로 해석하지 않고 feature·target·cfg matrix를 별도 기록한다. |
| `clippy::restriction` 전체 활성화는 서로 모순되는 lint가 있어 권장되지 않고 nursery는 불안정한 lint 집합이다. | group allowlist를 거부하고 exact lint ID만 사용한다. |
| rustc JSON Diagnostic은 `MachineApplicable`, `MaybeIncorrect`, `HasPlaceholders`, `Unspecified`를 구분한다. | exact `MachineApplicable` suggestion만 fix candidate가 될 수 있다. |
| Cargo `[lints]`와 `[workspace.lints]`는 package/workspace lint level을 선언하고 source attribute도 lint level을 가진다. | lint level 정본은 Cargo manifest와 source다. `clippy.toml`은 lint별 parameter source일 뿐 자동 fix 허가 source가 아니다. |
| Cargo feature와 target/cfg는 선택된 조합만 compile하며 `--all-features`는 모든 선택 package feature를 켠다. Cargo 문서도 상호 배타 feature가 드물게 존재할 수 있음을 설명한다. | 범용 기본값으로 `--all-features`를 사용하지 않고 project Catalog가 호환된다고 선언한 feature set만 실행한다. |
| `cargo fix`는 inactive feature·cfg를 고치지 못하고 edition migration option과 dirty/staged 우회 option을 제공한다. | M11에서 `cargo fix`와 edition migration을 제외하고 coverage 한계를 성공으로 숨기지 않는다. |
| Cargo build script는 compile 전에 실제 executable로 실행되고 proc macro는 compiler와 같은 file access를 가진다. | Clippy를 text-only read로 표현하지 않고 trusted project의 `process_run`으로 실행하며 source manifest side effect를 검사한다. |
| rustup은 가까운 `rust-toolchain.toml`/`rust-toolchain`을 따라 toolchain과 component·target을 선택한다. | auto apply에는 project source에서 resolve한 exact stable release pin을 요구하고, moving `stable`, environment/CLI override, directory override만으로는 pinned라 하지 않는다. |

M11은 공식 문서가 “안전한 lint 목록”을 보장한다고 추론하지 않는다. v1 built-in Clippy fix allowlist는 비어 있으며, exact Clippy version과 Corpus로 검증되고 lifecycle이 있는 policy entry만 별도로 추가할 수 있다.

## 6. CLI-only 사용자 흐름

public CLI 목표 surface는 다음 의미로 고정한다. exact parser spelling은 구현 Slice의 CLI fixture에서 동결하되 command 간 상태 경계는 바꾸지 않는다.

```text
star style rust inspect <project-id> [--json]
star style rust check <project-id> [--scope <package|workspace>] [--json]
star style rust prepare <project-id> --scope <package|workspace> [--json]
star style rust auto-apply <project-id> --scope <package|workspace> [--json]

star patch show <patch-set-id> [--diff] [--impact] [--json]
star patch status <patch-application-id> [--json]
star patch recover <patch-application-id> --strategy reverse-patch|discard-isolated [--json]
```

| command | target source effect | 핵심 output |
|---|---|---|
| `inspect` | 없음 | RustToolchainBinding, config source, package/target/feature inventory, coverage 후보와 limitation |
| `check` | 없음 | current fmt drift, Clippy Diagnostic, coverage matrix, ValidationRun과 Gate. source를 바꾸지 않음 |
| `prepare` | 없음 | isolated preview, step diff, immutable PatchSet, reverse 자료, 영향·검사 계획, pre-apply readiness |
| `auto-apply` | 조건부 `.rs` modify | `personal_auto` standing scope 확인 → prepare → exact PatchSet policy decision → 기존 apply command → post Gate |

`check`의 scope가 생략되면 workspace read-only check를 사용할 수 있다. `prepare`와 `auto-apply`는 `package` 또는 `workspace`를 반드시 명시한다. package scope에는 stable package ID 또는 M1이 resolve한 package selector가 추가로 필요하며 이름이 중복되면 중단한다. routine change는 M2 affected-package scope를 사용하고, 전체 workspace normalization은 사용자가 명시한 별도 ChangePlan으로만 수행한다.

`auto-apply`는 prepare/apply를 하나의 transaction이나 숨은 `--apply`로 합치지 않는다. CLI가 한 workflow처럼 보여도 다음 ID와 event는 별도다.

```text
RustStyle workflow request
  -> inspect/check operation
  -> RecipeExecution(preview + step executions)
  -> PatchSet
  -> policy ApprovalDecision recorded on exact ApprovalRequest
  -> patch_pre_apply GateDecision
  -> PatchApplication
  -> patch_post_apply GateDecision
  -> EvidenceBundle -> ReviewPack
```

human text와 `--json`은 같은 application result를 render한다. CLI handler는 toolchain을 resolve하거나 process·DB·Git·SourceMutationPort를 직접 호출하지 않는다. CLI-only dependency graph에는 Codex, App Server, AI provider와 OpenAI API client가 없다.

## 7. Cargo workspace·toolchain·config 발견

### 발견 순서

`star-project`의 read-only Rust discovery는 다음 순서를 사용한다.

1. M1 current ProjectCheckout과 protected root binding을 확인한다.
2. project-relative `Cargo.toml`을 찾아 workspace root와 member package를 결정한다.
3. fixed adapter의 read-only `workspace_discovery` operation이 Registry에 등록된 Cargo executable을 typed `cargo metadata --format-version 1 --no-deps` 인자로 실행해 JSON을 수집한다. 이 probe는 네 style check/mutator Tool role과 구분하고 descriptor ref·argument policy·parser version을 binding에 기록한다. network는 금지하고 output format version과 unknown field를 보존한다.
4. package ID, manifest, edition, rust-version, target kind/name, required-features와 declared feature graph를 source manifest 관찰과 대조한다.
5. `rust-toolchain.toml` 또는 legacy `rust-toolchain`, Cargo edition/rust-version, rustup override observation과 실제 cargo/rustc/rustfmt/clippy-driver identity를 분리해 수집한다.
6. 각 formatting unit에서 `rustfmt.toml` 또는 `.rustfmt.toml`의 실제 resolution chain을 계산하고 pinned rustfmt read-only config probe로 stable/unstable option과 parsing/style edition을 확인한다.
7. `Cargo.toml` `[lints]`, inherited `[workspace.lints]`, source lint attribute와 `clippy.toml`/`.clippy.toml` parameter source를 각각 수집한다.
8. project/user Catalog의 Rust style policy와 coverage declaration을 resolve한다.
9. 위 결과를 `RustToolchainBinding`, `RustStylePolicySnapshot`, `RustStyleCoverageMatrix` candidate로 materialize하고 completeness·limitation을 판정한다.

`cargo metadata`의 absolute path는 adapter 안에서 즉시 ProjectPathRef/opaque binding으로 바꾸고 raw 개인 path를 persisted document·DB·report에 넣지 않는다. metadata command가 source·lockfile을 바꾸거나 network를 요구하면 discovery를 complete로 만들지 않는다.

### pinned stable 판정

auto apply의 `toolchain_pin_state=pinned_stable`은 다음을 모두 만족해야 한다.

- project source의 `rust-toolchain.toml`/`rust-toolchain` 또는 동일 수준의 versioned project Catalog가 exact stable release를 지정한다.
- `stable`, `beta`, `nightly`, date 없는 moving alias와 environment/CLI/directory override만으로 선택된 toolchain이 아니다.
- cargo, rustc, rustfmt, clippy-driver가 같은 resolved toolchain lineage에 속한다.
- rustfmt와 Clippy component가 이미 설치돼 있고 requested target이 이미 available하다.
- actual executable identity·version·full SHA-256와 host triple이 probe 전·실행 전 동일하다.
- requested parsing edition, style edition과 config option을 해당 stable rustfmt가 지원한다.

pin이 없거나 complete하지 않아도 `inspect`와 source effect 없는 `check`는 limitation을 표시하며 실행할 수 있다. 그러나 `prepare` 결과는 기본 `HUMAN_REVIEW`, `auto-apply`는 금지한다. nightly·unstable config는 inspect 결과에는 표시하지만 candidate mutation에는 사용하지 않는다.

### `RustToolchainBinding` v1

`RustToolchainBinding`은 top-level document가 아니라 RecipeExecution·ValidationRun의 nested contract다.

| field | 의미 |
|---|---|
| `contract_version` | `1` |
| `workspace_root_ref`, `manifest_refs` | workspace root와 member Cargo manifest의 ProjectPathRef·content hash |
| `toolchain_source` | `rust_toolchain_toml|legacy_rust_toolchain|project_catalog|rustup_directory_override|environment_override|default`와 source ref |
| `toolchain_pin_state` | `pinned_stable|moving_stable|beta|nightly|custom|unresolved` |
| `channel`, `release`, `host_triple` | redacted resolved toolchain identity. release는 exact stable SemVer일 때만 값 존재 |
| `cargo`, `rustc`, `rustfmt`, `clippy_driver` | logical executable ID, opaque file identity, version, full SHA-256, component state |
| `parsing_editions` | package/target별 edition과 Cargo source ref |
| `style_editions` | formatting unit별 resolved value와 `explicit_config|cargo_edition_inferred` provenance |
| `msrv_bindings` | package별 Cargo rust-version, Clippy parameter/attribute source와 conflict |
| `host_target`, `requested_target_triples` | host와 declared coverage target set |
| `config_bindings` | rustfmt·Clippy config path ref, content hash, resolution root와 ambiguity state |
| `component_states`, `target_states` | `available|missing|unsupported|unverified`; 설치 action을 포함하지 않음 |
| `completeness`, `limitations` | `complete|partial|unverified`와 stable reason code |
| `binding_fingerprint` | 위 의미 field, executable hash, config hash의 canonical SHA-256 |

toolchain/config/executable 하나라도 바뀌면 기존 RecipeExecution, PatchSet, ValidationRun과 Gate는 `stale_tool` 또는 `stale_config`다. 새 version을 compatible하다고 추측해 기존 PatchSet에 다시 bind하지 않는다.

## 8. source·config·DB 정본 경계

### 정본 source

| 의미 | authoritative source |
|---|---|
| formatting rule | 가장 가까운 유효 `rustfmt.toml`/`.rustfmt.toml`; 없으면 resolved stable rustfmt default와 Rust Style Guide |
| parsing edition·MSRV | package `Cargo.toml`의 edition·rust-version과 workspace inheritance |
| lint level | package `[lints]`, inherited `[workspace.lints]`, source `allow|warn|deny|forbid` attribute |
| lint별 parameter | resolved `clippy.toml`/`.clippy.toml` |
| toolchain | project toolchain file 또는 versioned project binding + actual resolved executable identity |
| Clippy 자동 fix 허가 | versioned project/user Catalog의 exact lint entry 또는 exact Clippy version+Corpus에 묶인 enabled built-in policy |
| coverage | versioned project Catalog의 package·target·feature·triple matrix와 M1 actual inventory |
| source 결과 | Git-tracked/current workspace `.rs` byte |

StarConfig와 DB에 rustfmt option, lint level, `#[allow]`, Clippy parameter를 복제하지 않는다. StarConfig는 어떤 policy Profile을 선택했는지, resource/diff limit과 auto-apply grant처럼 Star-Control 행동만 소유한다.

### derived state

DB와 evidence에는 다음 projection만 저장한다.

- resolved toolchain/config/policy fingerprint와 completeness
- package·target·feature·target triple coverage cell과 실행 상태
- rustfmt drift, Clippy Diagnostic·suggestion applicability와 selection reason
- RecipeExecution·step execution·PatchSet·PatchApplication 상태
- ValidationRun·GateDecision·EvidenceBundle·ReviewPack ref
- stale/partial/unavailable reason과 run history

DB row를 수정해 Rust source, lint attribute, Cargo/rustfmt/Clippy config 또는 auto-fix allowlist source를 바꾸는 command는 만들지 않는다. source/config/Catalog가 DB와 다르면 source를 우선하고 projection을 stale로 만든 뒤 rebuild한다.

### 허용 source operation

M11 PatchSet v1은 다음 operation만 포함할 수 있다.

```text
operation = modify
path class = handwritten Rust source
extension = .rs
before and after bytes = complete
file mode = unchanged
existence = present -> present
```

create/delete/rename, mode change, symlink/reparse change, Cargo/config/toolchain/lockfile 변경, generated/vendor/out-of-scope `.rs` 변경은 `RUST_STYLE_SIDE_EFFECT_VIOLATION`이다. 필요한 변경을 candidate에서 빼고 나머지만 적용하는 것이 아니라 전체 candidate를 폐기한다.

## 9. rustfmt와 Clippy 교정 계층

M11은 두 교정 계층을 같은 PatchSet으로 수렴시키되 의미와 evidence를 섞지 않는다.

### rustfmt 계층

- formatting check·rewrite의 canonical entry는 `cargo fmt`다. direct `rustfmt`는 version/config probe에만 사용할 수 있다.
- workspace scope는 typed `--all`, package scope는 exact Cargo package ID에 bind해 adapter가 만든 `--package <resolved-package-spec>`을 사용한다.
- check invocation은 workspace에서 `cargo fmt --all -- --check`, package에서 `cargo fmt --package <resolved-package-spec> -- --check`로 고정한다. isolated rewrite는 같은 typed scope에서 마지막 `-- --check`만 제거한다.
- 실제 argument는 ToolDescriptor가 고정하며 user/project Catalog가 임의 flag를 추가하지 못한다.
- stable rustfmt가 resolve한 config와 style edition만 사용한다. `unstable_features=true`, nightly-only option 또는 unsupported style edition이 있으면 auto candidate를 만들지 않는다.
- rustfmt step은 formatting operation이다. semantic-preserving을 별도 증명하거나 public API 변화가 없다고 tool exit만으로 주장하지 않는다. M2/M3의 actual diff·public surface·build/test 검사를 그대로 적용한다.
- formatting unit별 config가 다를 수 있다. mixed style edition은 각 `.rs`가 한 unambiguous unit에 속하고 모든 unit이 current·complete하게 실행됐을 때만 허용한다.

### Clippy 계층

- check 단계에서 Cargo JSON message와 rustc Diagnostic JSON을 typed parser로 정규화한다.
- lint code가 null이거나 `clippy::<exact-lint-id>`로 정규화되지 않으면 자동 fix candidate가 아니다.
- source lint attribute와 Cargo lint level을 바꾸지 않는다. `#[allow(...)]`로 보이지 않는 lint는 “고쳐야 할 누락”이 아니다.
- `clippy.toml`은 lint parameter에만 사용한다. 그 파일에 lint가 등장하거나 parameter가 있다는 사실은 fix 승인으로 해석하지 않는다.
- auto fix는 exact allowlist entry, `MachineApplicable`, current suggestion fingerprint, allowed `.rs` scope를 모두 만족하는 suggestion만 선택한다.
- `cargo clippy --fix`가 실제 적용한 모든 hunk를 step-before JSON suggestion과 byte-exact 대응시킨다. 대응되지 않는 hunk가 하나라도 있으면 전체 candidate를 폐기한다.
- allowlist 밖 lint, `MaybeIncorrect`, `HasPlaceholders`, `Unspecified`, span/source hash 불일치는 수정하지 않고 Diagnostic으로 남긴다.
- public API 변화, generated file, Cargo/config 변화와 scope 밖 hunk는 suggestion applicability와 무관하게 차단한다.

rustfmt와 Clippy는 서로 다른 tool output을 낸다. rustfmt diff를 Clippy suggestion hunk로, Clippy hunk를 formatting diff로 다시 분류하지 않는다. 각 step의 before/after manifest와 final diff를 모두 보존한다.

## 10. Clippy allowlist와 suggestion applicability

### allowlist entry

`RustStylePolicySnapshot`의 effective Clippy fix allowlist entry는 최소 다음 field를 가진다.

| field | 규칙 |
|---|---|
| `lint_id` | canonical exact ID `clippy::<lint-name>`. group·prefix·glob·regex 금지 |
| `entry_version` | entry 자체 SemVer |
| `source` | `project_catalog|user_catalog|builtin_verified`와 source document ref |
| `clippy_release` | exact stable Clippy release와 executable hash constraint. open-ended range 금지 |
| `required_applicability` | 정확히 `MachineApplicable` |
| `allowed_scope` | handwritten `.rs` modify와 package/workspace ceiling |
| `public_api_policy` | M11 v1은 `deny` 고정 |
| `required_check_families` | lint별 최소 build/test/contract Check union |
| `corpus_ref` | positive/negative/conflict/idempotence case manifest와 expected hash |
| `lifecycle` | `active|deprecated|retired|rejected`; active만 선택 가능 |
| `definition_fingerprint` | 위 의미 field의 canonical SHA-256 |

다음 ID는 allowlist entry로 거부한다.

- `clippy::all`, `clippy::correctness`, `clippy::style`
- `clippy::pedantic`, `clippy::restriction`, `clippy::nursery`, `clippy::cargo`
- `clippy::*`, 접두사·pattern·“현재 MachineApplicable인 모든 lint”
- Clippy version/hash 또는 Corpus가 없는 exact lint ID

v1 built-in allowlist는 빈 집합이다. built-in entry를 추가하려면 exact Clippy release, Windows x64·ARM64, representative multi-crate/feature/target Corpus에서 hunk mapping·idempotence·public API·side-effect case를 통과하고 Catalog lifecycle review를 받아야 한다. 제품 code에 “일반적으로 안전해 보이는 lint” 배열을 하드코딩하지 않는다.

project/user/built-in source의 entry는 provenance를 유지한 채 ID별로 resolve한다. project deny 또는 더 좁은 path/scope/check floor가 항상 이긴다. 같은 lint ID의 release·scope·required Check가 충돌하면 넓은 쪽을 추측하지 않고 entry를 unavailable로 만든다. user grant는 project 금지를 넓히지 못하고 built-in entry는 사용자가 해당 policy를 활성화한 경우에만 사용한다.

### suggestion selection

각 suggestion candidate는 다음 identity를 가진다.

```text
suggestion_fingerprint = SHA-256(
  clippy executable hash
  + lint_id
  + diagnostic code/message-independent identity
  + package/target/feature/triple coverage cell ID
  + project-relative file + before file hash
  + ordered byte ranges + replacement byte hashes
  + applicability
  + normalizer version
)
```

표시 message와 line/column만으로 suggestion을 식별하지 않는다. multipart suggestion은 모든 span이 같은 allowed Project/scope 안에 있고 전부 `MachineApplicable`이며 겹치지 않을 때 하나의 원자적 suggestion으로 선택한다. macro expansion, external dependency path, invalid UTF-8 boundary와 generated ownership이 포함되면 선택하지 않는다.

pipeline 2단계의 current Diagnostic은 baseline을 제공한다. 첫 rustfmt 뒤 span이 이동할 수 있으므로 Clippy fix step은 formatted preview에서 같은 coverage cell의 JSON Diagnostic을 다시 수집하고 새 before hash로 suggestion을 rebind한다. current source suggestion을 formatted preview에 offset 보정만 해 적용하지 않는다.

### fix 실행과 hunk 검증

각 coverage cell의 fix는 after-rustfmt base에서 독립된 child preview로 실행한다.

1. 해당 cell의 active project lint policy로 Clippy JSON Diagnostic을 수집한다.
2. allowlist와 applicability를 통과한 suggestion set을 byte-order로 정렬한다.
3. typed adapter가 exact lint ID set과 Cargo package/target/feature/triple arguments로 `cargo clippy --fix`를 실행한다.
4. 앞선 rustfmt change 때문에 child preview가 dirty이면 `--allow-dirty`를 사용할 수 있다. 이 사용은 Star-Control-owned preview, staged byte 0, complete dirty manifest가 직전 pipeline output과 exact 일치할 때만 허용한다.
5. `--allow-staged`, `--broken-code`, `--allow-no-vcs`는 사용하지 않는다.
6. Controller가 full before/after manifest와 byte diff를 독립 수집한다.
7. 각 actual edit가 선택 suggestion replacement와 exact 일치하는지 검증한다. Clippy가 unselected suggestion을 적용하거나 selected suggestion 일부만 적용하면 cell result는 실패다.
8. 여러 cell candidate를 after-rustfmt base에 3-way가 아닌 byte-exact operation set으로 reconcile한다. 같은 before range에 다른 after byte가 있으면 coverage conflict를 가진 `RUST_STYLE_COVERAGE_INCOMPLETE`로 전체 자동 적용을 차단한다.

CLI lint flag로 unselected suggestion이 절대 나오지 않을 것이라고 신뢰하지 않는다. 최종 안전 경계는 isolated diff와 suggestion-to-hunk exact mapping이다.

## 11. package·target·feature·cfg coverage

### coverage dimension

`RustStyleCoverageMatrix`는 다음 차원을 독립적으로 기록한다.

| 차원 | 값 |
|---|---|
| workspace/package | workspace root, Cargo package ID와 manifest hash |
| target | `lib|bin|test|example|bench|custom-build|proc-macro`, target name과 source root |
| feature set | stable Catalog feature-set ID, default on/off와 sorted exact features |
| required feature | Cargo target `required-features` 만족 여부 |
| host/target | host triple, requested target triple와 component availability |
| cfg/platform | declared cfg observation, selected target가 활성화한 known cfg와 미관찰 frontier |
| ownership | handwritten/generated/vendor/out-of-scope |
| phase | diagnostic check, isolated fix, candidate final check, actual-after post check |
| execution | `executed|skipped|unavailable|conflicted|invalidated`와 reason |

`cargo clippy --fix`가 `--all-targets`를 암묵적으로 사용해도 inactive feature, 다른 target triple, false cfg branch, target required-feature 미충족과 generated ownership을 검사했다고 표시하지 않는다.

### feature set 정책

- `--all-features`는 built-in 기본값이 아니다.
- project Catalog는 호환되는 feature set을 stable ID와 exact sorted feature list로 선언한다.
- default set, no-default set과 named set을 각각 독립 coverage cell로 사용할 수 있다.
- 서로 배타적 feature는 같은 cell에 넣지 않는다. 발견한 `compile_error!`·documented conflict 또는 project declaration과 모순되는 set은 실행하지 않고 invalid로 남긴다.
- 같은 source range에 feature set별 suggestion이 다른 replacement를 요구하면 자동 apply를 차단한다. 한쪽을 임의 우선하지 않는다.
- declared feature가 있는데 required matrix가 없거나 unknown cfg frontier가 auto policy ceiling을 넘으면 coverage는 partial이다.

### target와 cfg 정책

- `--all-targets`는 같은 selected package/feature/triple에서 Cargo가 선택하는 target 집합일 뿐 cross-target coverage가 아니다.
- target triple은 project Catalog가 required로 선언한 stable 순서대로 실행한다. target가 설치돼 있지 않으면 `RUST_COMPONENT_UNAVAILABLE`이며 자동 설치하지 않는다.
- build script와 proc macro도 target inventory와 execution evidence에 표시한다. 이 code가 source root를 수정하면 target dir 밖 여부와 관계없이 side-effect violation이다.
- generated source는 Diagnostic coverage에는 포함할 수 있지만 M11 Patch operation target은 아니다.
- coverage가 `complete`라는 말은 Catalog가 요구한 모든 cell이 current tool/config/source에서 `executed`되고 parser/output가 complete하다는 뜻이다. “발견하지 못한 cell 없음”을 complete로 추측하지 않는다.

### `RustStyleCoverageMatrix` v1

| field | 의미 |
|---|---|
| `contract_version` | `1` |
| `matrix_id` | owning RecipeExecution 안의 deterministic local ID |
| `scope` | ProjectId·CheckoutId, package/workspace selector와 accepted ScopeRevision |
| `inventory_fingerprint` | Cargo metadata·manifest·M1 source/target/feature inventory hash |
| `policy_ref`, `policy_fingerprint` | required feature/target/coverage source |
| `cells` | stable byte-order의 coverage cell array |
| `cell_relations` | same source, mutually exclusive, requires, conflict candidate edge |
| `required_cell_count`, `executed_cell_count` | 숫자와 산정 근거. missing 값을 0으로 채우지 않음 |
| `frontier` | inactive/unknown cfg, unavailable target/component, unsupported target와 이유 |
| `conflicts` | 같은 byte range·symbol에 대한 incompatible suggestion set ref |
| `completeness` | `complete|partial|unverified` |
| `matrix_fingerprint` | inventory·policy·ordered cell/result·frontier의 canonical SHA-256 |

cell은 package/target/feature/triple/cfg/ownership key, invocation ref, execution state, Diagnostic/suggestion set fingerprint, before/after manifest fingerprint, limitation과 selected/nonselected reason을 가진다.

## 12. 고정 pipeline과 idempotence

pipeline ID는 `rust_style_v1`이고 version은 `1`이다. ordered step ID, ToolDescriptor, argument policy, output normalizer와 side-effect validator version을 합친 `fixed_adapter_definition_fingerprint`를 모든 실행에 기록한다. 사용자가 step을 재배열·삭제하거나 command string을 주입할 수 없다.

### 고정 순서

1. `resolve`: Project·workspace·toolchain·style config·lint source·Catalog policy를 resolve한다.
2. `current_check`: current exact byte의 mirror에서 fixed `cargo fmt <typed-scope> -- --check`와 Clippy JSON Diagnostic을 수집한다.
3. `scope_plan`: M1/M2로 package·target·feature·path scope와 coverage requirement를 확정한다.
4. `preview_create`: exact base와 필요한 current dirty byte를 가진 Star-Control-owned isolated preview workspace를 만든다.
5. `rustfmt_first`: scope-bound `cargo fmt`를 실행하고 full step diff를 수집한다.
6. `clippy_allowlisted_fix`: formatted preview 기준 Diagnostic 재수집, cell별 `cargo clippy --fix`, suggestion-hunk 검증과 deterministic candidate reconciliation을 수행한다.
7. `rustfmt_final`: reconciled candidate에 같은 config binding의 `cargo fmt`를 다시 실행한다.
8. `diff_collect`: 모든 step diff와 final complete filesystem diff를 수집한다.
9. `side_effect_validate`: `.rs` modify 이외 operation, generated/vendor/out-of-scope와 config/Cargo/toolchain 변화를 거부한다.
10. `impact_reconcile`: preview ChangeSet으로 M2 영향·risk·Profile closure·ValidationPlan을 재계산한다.
11. `idempotence_replay`: expected-after의 새 isolated workspace에서 전체 `rust_style_v1` mutation pipeline을 replay해 operation 0건인지 확인한다.
12. `candidate_validate`: final fmt check, 전체 required Clippy coverage check와 M2 selected build/test/contract Check를 실행한다.
13. `patch_finalize`: immutable PatchSet, forward/reverse artifact와 step evidence manifest를 생성한다.
14. `pre_apply`: exact policy decision과 M3 `patch_pre_apply`를 계산하고 single-use permit을 만든다.
15. `apply`: M4 SourceMutationPort로 target에 immutable PatchSet만 적용한다. cargo/rustfmt/Clippy를 live target에서 실행하지 않는다.
16. `post_apply`: target actual-after를 rescan한 뒤 그 exact byte의 isolated validation mirror에서 fmt/Clippy/affected Check를 실행하고 M3 `patch_post_apply`를 계산한다.
17. `evidence_finalize`: complete EvidenceBundle·ReviewPack과 terminal workflow state를 생성한다.

step 1~13은 target source effect가 없다. step 14는 approval/Gate state만 만들고 step 15만 target `.rs` byte를 바꿀 수 있다. step 15가 실행된 뒤 step 16/17이 실패하면 “자동 교정 성공”이 아니라 M4 recovery state다.

### idempotence

replay는 최종 PatchSet의 operation만 다시 적용해 보는 검사가 아니다. expected-after source, 같은 toolchain/config/policy/coverage와 새 target dir에서 step 5~9를 다시 실행한다.

다음이 모두 0이어야 한다.

- rustfmt first/final diff operation
- selected Clippy fix suggestion와 actual hunk
- final filesystem diff
- M2 change impact delta와 newly required Check

Diagnostic 자체는 계속 존재할 수 있다. allowlist 밖 lint나 project가 허용한 warning은 raw Diagnostic으로 남지만 새 mutation operation을 만들지 않아야 한다. replay의 tool/config/coverage가 원 실행과 다르면 no-op이 아니라 stale/unverified다.

## 13. isolated worktree·PatchSet·rollback

Clippy는 build script와 proc macro를 실행할 수 있고 rustfmt/Clippy fix는 file을 수정한다. 따라서 current/candidate/post의 모든 Clippy check, `prepare`와 idempotence replay는 해당 subject의 exact byte를 가진 disposable mirror 또는 isolated Git worktree에서 실행한다. target checkout이 clean이어도 external mutator나 project code를 동반하는 Clippy process를 live path에서 실행하지 않는다. post Gate는 mirror 결과를 target actual-after binding과 대조한다.

### preview 환경

- cwd는 ToolDescriptor의 `stage_worktree`만 사용한다.
- `CARGO_TARGET_DIR`은 source root 밖 Star-Control-owned per-run directory다.
- Cargo network는 기본 거부하고 offline mode를 사용한다. 필요한 dependency/tool/target download는 M11이 수행하지 않는다.
- staged byte는 0이어야 한다. preview dirty manifest는 base materialization과 앞 pipeline step output으로만 설명돼야 한다.
- tool process 전·후 source root 전체 manifest와 target-dir ownership manifest를 별도 수집한다.
- target dir output은 source PatchSet에 넣지 않지만 source root write가 발견되면 모두 side-effect 검사 입력이다.
- process는 current-user trusted code이며 Job Object는 resource/process-tree 통제일 뿐 filesystem sandbox라고 주장하지 않는다.

lockfile이 없는 preview에서 Cargo가 `Cargo.lock`을 만들거나 기존 lockfile을 갱신하면 그 사실을 숨기거나 cleanup으로 성공을 합성하지 않는다. `Cargo.lock`은 M11 candidate 대상이 아니므로 side-effect violation이다. project가 요구하는 dependency resolution을 offline에서 재현할 수 없으면 coverage unavailable로 남긴다.

### PatchSet 결합

M11 PatchSet은 기존 M4 v2 불변식 외에 다음 ref를 가진다.

- Profile ID/version/hash와 `rust_style_v1` fixed adapter fingerprint
- RustToolchainBinding·RustStylePolicySnapshot·RustStyleCoverageMatrix fingerprint
- 모든 RustStyleStepExecution ref와 ordered step diff artifact
- selected/nonselected suggestion manifest와 hunk mapping artifact
- final complete filesystem diff와 allowed `.rs` operation manifest
- M2 preview impact reconciliation과 candidate ValidationPlan
- idempotence replay evidence
- forward/reverse byte artifact와 actual before hash

PatchSet을 만든 뒤 source, dirty manifest, toolchain/config, Catalog policy, coverage inventory, ToolDescriptor 또는 selected Check가 바뀌면 stale다.

### 적용과 rollback

apply는 cargo/rustfmt/Clippy process를 다시 실행하지 않는다. exact PatchSet operation을 M4 SourceMutationPort가 before hash·mode·existence를 확인한 뒤 적용한다. partial receipt, after hash mismatch와 unexpected file은 `PATCH_PARTIAL_APPLY|PATCH_POSTCONDITION_FAILED`이며 실제 byte를 보존해 reconcile한다.

rollback은 성공 표시를 위한 숨은 동작이 아니다. reverse PatchSet은 별도 current precondition, PermissionPlan과 Gate를 요구한다. post Gate 실패만으로 사용자 기존 변경을 되돌리거나 `git reset --hard`, `checkout`, stash, broad cleanup을 실행하지 않는다. preview-only 실패는 evidence finalize 뒤 owned worktree를 `discard_ready`로 만들 수 있지만 삭제에도 ownership·retention·permission 검사가 필요하다.

## 14. `personal_auto` 자동 적용

### 정책 차이

| policy | 자동 가능한 최대 경계 | source apply |
|---|---|---|
| `safe_default` | inspect, check, isolated prepare와 PatchSet 표시 | exact PatchSet에 대한 사용자 `ApprovalRequest decision=approved` 뒤 기존 `patch apply` |
| `personal_auto` | inspect, check, prepare, exact policy evaluation | 아래 standing grant·candidate `AUTO_PASS`를 모두 만족하면 prompt 없이 policy approval 후 기존 `patch apply` |

`personal_auto`는 “모든 local write 허용”이 아니다. user config가 선택한 standing grant는 다음 exact 범위를 가진다.

| field | 의미 |
|---|---|
| `project_id` | 한 exact Project. path/name pattern만으로 대신하지 않음 |
| `profile_ref` | `rust_style_auto_fix` exact item version·definition hash |
| `pipeline_ref` | `rust_style_v1@1`과 fixed adapter fingerprint |
| `style_policy_fingerprint` | toolchain/config/allowlist/coverage policy snapshot ceiling |
| `scope_ceiling` | 허용 package ID set 또는 explicit workspace, handwritten `.rs` path set |
| `allowed_actions` | `process_run`, preview root `local_write`, target `.rs` `local_write`; delete/move/dependency/system/network action 없음 |
| `diff_limits` | 최대 file·hunk·changed byte/line, public surface delta 0 |
| `required_gate_phases` | permit 전 candidate·`patch_pre_apply` `AUTO_PASS`, 성공 terminal state 전 `patch_post_apply` `AUTO_PASS` |
| `expires_at`, `grant_fingerprint` | 유효 기간과 user-owned source hash |

standing grant는 exact PatchSet 승인이 아니다. prepare 뒤 policy evaluator가 다음 candidate를 다시 평가해 기존 `ApprovalRequest`에 exact `scope_hash`, PatchSet fingerprint, Project/Checkout, action set, toolchain/policy/coverage fingerprint, expiry와 evidence ref를 넣고 `decision=approved`, `resolved_by=policy_evaluator`인 ApprovalDecision을 기록한다. 이 decision은 사용자가 선택한 grant의 기계적 후속 결정이며 새 source-of-truth나 재사용 가능한 capability가 아니다.

그 뒤 M3 `patch_pre_apply`가 `AUTO_PASS`이고 application이 current binding을 다시 확인해야만 `PatchApplyPermit(kind=automatic)`을 한 번 만든다. permit 소비는 기존 `patch apply` application command와 같은 SourceMutationPort 경로를 사용한다.

### 자동 적용 금지 조건

다음 중 하나라도 있으면 policy evaluator는 approval을 발행하지 않는다.

- candidate 또는 pre Gate가 `HUMAN_REVIEW|BLOCK`이거나 permit 전 required evidence가 incomplete
- diff limit 초과, public API 영향, create/delete/rename/mode change
- package/workspace selector가 standing grant와 다름
- toolchain 미고정, nightly/unstable option, component/target unavailable
- coverage `partial|unverified`, feature/target/cfg conflict 또는 required Check 미실행
- dirty overlap·unknown, preexisting byte 손상 가능성
- allowlist 밖 actual hunk, non-MachineApplicable suggestion 또는 hunk mapping 불완전
- fixed adapter, ToolDescriptor, executable/config/Catalog/source drift
- idempotence replay operation 1건 이상
- preview build script/proc macro/generated/vendor/out-of-scope side effect

`patch_post_apply`는 apply 뒤에만 존재하므로 policy evaluator가 exact PatchSet 승인을 발행할 때의 선행 입력이 아니다. 대신 post Gate가 `AUTO_PASS`가 아니거나 post evidence packaging이 incomplete하면 성공 terminal state를 만들지 않고 `recovery_required` 또는 해당 M4 상태로 전환한다.

`auto-apply`는 사용자가 terminal에서 명시적으로 시작한 workflow다. background watcher, daemon schedule, cron과 “source save 시 자동 apply”를 추가하지 않는다. post Gate 실패·partial apply·outcome unknown은 성공으로 표시하지 않는다.

## 15. Diagnostic·Evidence·report

### 기존 실행 record 재사용

M11은 별도 `RustStyleRun` 같은 mutable top-level record를 만들지 않는다. 한 workflow의 소유 관계는 다음과 같다.

1. `RecipeExecution`이 `rust_style_auto_fix` Profile, `rust_style_v1@1`, accepted TaskSpec/ScopeRevision과 resolve 결과를 소유한다.
2. 각 check/mutator process는 기존 `TaskInvocation`과 `ToolExecutionResult`를 사용하고 `RustStyleStepExecution`이 그 ref를 ordered step에 묶는다.
3. candidate가 생기면 기존 `PatchSet`이 final operation과 step evidence ref를 소유한다.
4. candidate와 actual-after 검증은 각각 기존 `ValidationRun`·`GateDecision`을 사용한다.
5. source 적용은 기존 `PatchApplication`과 receipt/recovery 상태를 사용한다.
6. `EvidenceBundle`과 `ReviewPack`은 위 immutable record의 hash/ref를 묶는 derived projection이다.

### `RustStylePolicySnapshot` v1

| field | 의미 |
|---|---|
| `contract_version` | `1` |
| `profile_ref`, `profile_definition_hash` | `rust_style_auto_fix` exact Catalog item |
| `pipeline_ref`, `fixed_adapter_definition_fingerprint` | `rust_style_v1@1`과 ordered adapter 계약 |
| `formatting_sources` | 선택된 rustfmt config path/hash, Style Guide/rustfmt default provenance와 shadowed candidate |
| `lint_level_sources` | Cargo `[lints]`/`[workspace.lints]`, package inheritance와 source attribute fingerprint. 값을 DB 설정으로 복사하지 않음 |
| `clippy_parameter_sources` | 선택된 `clippy.toml`/`.clippy.toml` path/hash와 shadowed candidate |
| `clippy_fix_allowlist` | exact lint ID별 decision source, required Clippy identity, optional Corpus evidence ref와 expiry |
| `coverage_policy` | package/target/feature/triple/cfg required matrix와 partial 허용 여부. auto apply는 partial 허용 불가 |
| `scope_policy` | package/workspace selector, handwritten `.rs` path ceiling과 generated/vendor classifier ref |
| `auto_policy` | `safe_default|personal_auto`, standing grant ref, diff limits와 required Gate phase |
| `forbidden_operations` | manifest/config/lockfile/attribute/public API/create/delete/rename 등 고정 금지 집합 |
| `policy_completeness` | `complete|partial|ambiguous|unsupported`와 reason ref |
| `policy_fingerprint` | 모든 ordered field와 source hash의 canonical SHA-256 |

allowlist entry는 `lint_id`, `decision=allow_fix`, `source_ref`, `clippy_identity_constraint`, `applicability=MachineApplicable`, optional package/path scope, Corpus evidence/expiry를 가진다. group ID, wildcard와 prefix match는 Schema validation에서 거부한다. built-in policy가 없다면 list가 비어 있는 것이 정상이며 rustfmt candidate만 만들 수 있다.

### `RustStyleStepExecution` v1

| field | 의미 |
|---|---|
| `contract_version` | `1` |
| `step_execution_id`, `ordinal`, `step_id` | owning RecipeExecution 안의 deterministic ID, `rust_style_v1` 순번과 stable step ID |
| `pipeline_ref`, `adapter_fingerprint` | 실행한 fixed adapter identity |
| `subject_before`, `subject_after` | Project/Checkout/worktree, complete source manifest와 relevant config/tool/policy fingerprint |
| `tool_descriptor_ref`, `task_invocation_ref`, `execution_result_ref` | Registry의 exact Tool, typed argv/env/cwd와 process 결과 |
| `coverage_cell_refs` | 이 step이 실행·건너뜀·invalidated한 matrix cell |
| `diagnostic_set_ref`, `suggestion_manifest_ref` | raw JSON artifact, normalized Diagnostic와 selected/nonselected suggestion |
| `diff_artifact_ref`, `filesystem_manifest_ref` | step별 complete diff와 before/after manifest |
| `side_effect_result` | `pass|violation|unverified`, violation operation ref |
| `result` | `succeeded|failed|blocked|stale|cancelled|outcome_unknown` |
| `started_at`, `finished_at` | 표시용 시각. fingerprint ordering 근거로 사용하지 않음 |
| `step_execution_fingerprint` | canonical body와 참조 artifact hash |

raw rustc/Clippy JSON은 artifact로 보존하고 normalizer가 `tool`, exact `code`, level, rendered message, primary/secondary span, child suggestion, applicability와 expansion origin을 Diagnostic으로 변환한다. parser가 모르는 schema/value를 만나면 Diagnostic을 버리지 않고 raw ref와 `unparsed` limitation을 남기며 coverage를 `unverified`로 낮춘다.

### Evidence binding

M11 candidate와 actual-after `EvidenceSubjectBinding`에는 최소 다음 exact ref가 있어야 한다.

- ProjectId·CheckoutId·base/current/expected/actual source manifest
- accepted TaskSpec·ScopeRevision·ChangePlan·ValidationPlan
- RustToolchainBinding fingerprint
- RustStylePolicySnapshot fingerprint
- RustStyleCoverageMatrix fingerprint
- ordered RustStyleStepExecution fingerprint list
- PatchSet/PatchApplication/ApprovalRequest/permit ref(해당 phase에서 존재할 때)
- ToolDescriptor·parser·fixed adapter definition hash
- current dirty manifest와 overlap decision

EvidenceBundle은 stdout/stderr만으로 complete가 될 수 없다. raw Diagnostic, normalized Diagnostic, selected/nonselected suggestion, hunk mapping, step/final diff, side-effect scan, impact reconciliation, replay, selected Check와 Gate input/output가 모두 current subject에 묶여야 한다.

### CLI report

text와 `--json`은 같은 application result를 렌더링한다. JSON은 최소 `schema_version`, command, workflow/recipe ID, state, project/scope, toolchain/policy/coverage summary, Diagnostic counts, candidate/PatchSet ref, Gate verdict, apply/recovery status, stable reason code와 evidence ref를 가진다.

text 출력은 다음 순서를 유지한다.

1. Project·Checkout·scope와 source freshness
2. pinned/resolved toolchain, style/parsing edition와 config source
3. coverage executed/required/partial cell과 limitation
4. rustfmt drift, Clippy Diagnostic와 fix selected/skipped 이유
5. file/hunk/line·byte diff, public/generated/out-of-scope 영향
6. candidate·pre/post Gate와 selected Check
7. PatchSet/application/recovery 상태와 다음 안전한 command

`inspect`는 apply 가능 여부를 예측해 보여 주되 Gate를 만들지 않는다. `check`는 source effect 없이 Diagnostic과 coverage만 보고한다. `prepare`는 PatchSet을 만들 수 있지만 적용하지 않는다. `auto-apply`는 prepare/apply ID와 phase 전환을 각각 출력한다. no-op은 `succeeded_no_change`이고 source write/PatchApplication이 없어야 한다.

## 16. failure·partial·stale 처리

### Rust 전용 stable reason code

| code | 발생 조건 | 기본 판정과 source effect |
|---|---|---|
| `RUST_TOOLCHAIN_UNRESOLVED` | project-pinned stable channel 또는 executable identity를 완전히 resolve하지 못함 | inspect/check limitation, prepare `HUMAN_REVIEW`, auto apply `BLOCK`; source 불변 |
| `RUST_COMPONENT_UNAVAILABLE` | rustfmt/Clippy/required target가 설치·실행 가능하지 않음 | 해당 cell unavailable, coverage partial, auto apply `BLOCK`; 설치 시도 없음 |
| `RUST_STYLE_CONFIG_AMBIGUOUS` | 같은 precedence에 config 후보가 둘 이상이거나 Cargo/Project 경계가 결정 불가 | `BLOCK`; 임의 선택 없음 |
| `RUST_STYLE_UNSTABLE_OPTION_UNSUPPORTED` | nightly 또는 unstable rustfmt option/style contract가 필요함 | auto candidate `BLOCK`; option 무시 후 성공 금지 |
| `RUST_STYLE_COVERAGE_INCOMPLETE` | required coverage cell skipped/unavailable/invalidated 또는 cfg frontier unverified | `HUMAN_REVIEW|BLOCK`, `AUTO_PASS` 금지 |
| `RUST_CLIPPY_FIX_NOT_ALLOWED` | exact lint ID가 allowlist에 없거나 version/scope 제약 불일치 | Diagnostic 유지, 해당 suggestion 수정 없음; 자체로 check 실패는 아님 |
| `RUST_CLIPPY_SUGGESTION_NOT_MACHINE_APPLICABLE` | applicability가 `MachineApplicable`가 아님/누락/unknown | Diagnostic 유지, 수정 없음; 자동 승격 금지 |
| `RUST_STYLE_SIDE_EFFECT_VIOLATION` | `.rs` modify 이외 operation, generated/vendor/out-of-scope/public/config/lockfile write 또는 unmatched hunk | candidate 전체 `BLOCK`, PatchSet finalize 금지 |
| `RUST_STYLE_NON_IDEMPOTENT` | replay에서 mutation/impact delta가 남거나 결과가 수렴하지 않음 | candidate `BLOCK`, target source 불변 |
| `RUST_STYLE_AUTO_SCOPE_MISMATCH` | candidate Project/Profile/policy/package/path/action/diff가 standing grant ceiling 밖 | automatic ApprovalDecision/permit 없음; safe_default review로만 전환 가능 |

한 execution에 여러 reason이 있을 수 있다. primary reason은 pipeline에서 가장 먼저 target mutation을 불가능하게 만든 stable code이고 나머지는 ordered `related_reasons`로 보존한다. CLI exit code는 기존 category mapping을 따르고 reason code를 process exit code로 재해석하지 않는다.

### 공통 오류 재사용

- process spawn/timeout/non-zero/invalid JSON은 외부 Tool Registry의 공통 execution 오류를 쓴다.
- ToolDescriptor·Catalog·source/config/dirty drift는 공통 stale binding 오류를 쓴다.
- dirty overlap/unknown은 M4 `PATCH_DIRTY_OVERLAP` 계열을 쓴다.
- before hash mismatch, partial apply, postcondition 실패와 recovery는 M4 오류를 쓴다.
- candidate/post ValidationRun의 failed/incomplete은 M3 공통 Gate reason을 쓴다.
- network 요청·permission 부족·untrusted project는 security/permission 공통 오류를 쓴다.

Rust reason을 generic error 대신 중복 발행하지 않는다. 예를 들어 Clippy process timeout은 `RUST_STYLE_COVERAGE_INCOMPLETE`의 원인 ref가 될 수 있지만 primary execution error는 공통 timeout이다.

### terminal state 원칙

- prepare 전 실패: target 불변, RecipeExecution `failed|blocked|stale`와 incomplete evidence를 그대로 보존한다.
- preview mutation/검증 실패: target 불변, candidate를 applyable로 표시하지 않고 owned preview retention/discard 상태를 기록한다.
- apply 직전 drift: permit을 소비·폐기하고 PatchApplication을 시작하지 않는다.
- partial/outcome-unknown apply: 현재 byte를 재scan하고 `recovery_required`; 자동 rollback 성공으로 덮지 않는다.
- post Gate 실패: PatchApplication은 applied 사실을 보존하고 workflow는 `post_validation_failed|recovery_required`; 사용자에게 reverse PatchSet과 실패 evidence를 제시한다.
- evidence packaging 실패: source가 적용됐더라도 success terminal state가 아니며 evidence recovery가 필요하다.

retry는 새 TaskInvocation/ValidationRun을 만들고 같은 실패 record를 overwrite하지 않는다. source/tool/config/policy/coverage가 바뀌면 기존 candidate를 재사용하지 않고 새 RecipeExecution 또는 명시된 retry child에서 resolve부터 수행한다.

## 17. Package·adapter 소유권

| Package/정본 | M11 소유 책임 | 소유하지 않는 책임 |
|---|---|---|
| `star-project` | `cargo metadata` 기반 workspace/package/target/feature discovery, manifest/config/toolchain 후보와 source ownership 분류 | process 실행, fix 선택, Patch apply |
| `star-application` | `rust_style_v1` state machine, scope/coverage orchestration, candidate reconciliation, policy evaluator와 기존 M2/M3/M4 command 연결 | raw child process/DB 직접 접근, filesystem 직접 write |
| `star-execution` | registered cargo/rustfmt/Clippy probe와 typed argv/env/cwd, child process tree·timeout·output capture | lint policy 결정, Diagnostic 의미 판정 |
| `star-validation` | rustc/Clippy JSON normalization, coverage completeness, suggestion/hunk·side-effect·idempotence 검사와 Gate input | source mutation |
| `star-contracts` | RustToolchainBinding, RustStylePolicySnapshot, RustStyleCoverageMatrix, RustStyleStepExecution의 최소 versioned nested type | mutable runtime state나 formatter logic |
| `star-state` | 기존 Recipe/Patch/Validation/application projection과 query | Git/config source-of-truth 복제 |
| `star-evidence` | raw/normalized Diagnostic, diff, manifest, binding artifact와 ReviewPack 조립 | approval·apply 실행 |
| `star-cli` | `star style rust ...` parse, application command 호출과 text/JSON render | cargo invocation 조립, policy 판단, source write |
| `catalog/profiles` | `rust_style_auto_fix` Profile/Recipe version과 fixed workflow metadata | project rustfmt/lint 값 |
| Tool package Catalog | stable Tool/Check role, descriptor identity, argument/output parser policy | 임의 shell command, 사용자별 source policy |

Rust-specific 처리는 위 기존 package 안의 bounded module/adapter로만 둔다. 네 번째 executable, shared mutable singleton 또는 별도 Rust daemon을 만들지 않는다.

### Tool role와 stable ID

| role | stable Tool ID | mutation | 핵심 typed policy |
|---|---|---|---|
| rustfmt check | `star.rust.style.rustfmt.check` | 없음 | cargo fmt check, scope selector, stable config binding |
| rustfmt isolated rewrite | `star.rust.style.rustfmt.rewrite` | preview `.rs` | cargo fmt, isolated cwd only, complete manifest diff |
| Clippy Diagnostic check | `star.rust.style.clippy.check` | source 없음 | cargo clippy JSON, exact package/target/feature/triple cell |
| Clippy allowlisted isolated fix | `star.rust.style.clippy.fix` | preview `.rs` candidate | exact selected lint/suggestion manifest, isolated cwd, hunk validator |

toolchain discovery는 Registry probe 계약으로 구현할 수 있으나 임의 command Tool을 추가하지 않는다. stable ID를 추가한다면 `star.rust.style.*` namespace와 exact adapter definition을 사용한다. descriptor fingerprint에는 executable trust identity, typed argument policy, fixed env/network/cwd policy, output parser와 side-effect validator version을 포함한다.

Manifest protocol은 이미 executable/argv/env/cwd/output/permission을 표현할 수 있으므로 M11만을 위한 shell field나 Rust-specific top-level field를 추가하지 않는다. 필요한 typed constraints는 host-side adapter/Catalog conformance metadata가 검증한다.

## 18. 구현 순서와 최소 Corpus

### 구현 순서

M1→M2→M3→M4 제품 Gate가 실제로 통과하기 전에는 M11 source mutation slice를 시작하지 않는다. 그 뒤 다음 순서로 구현한다.

1. read-only Cargo workspace/toolchain/config discovery
2. `RustToolchainBinding`과 coverage contract
3. rustfmt check와 Diagnostic normalization
4. Clippy check와 exact lint/suggestion 수집
5. isolated rustfmt PatchSet 수직 Slice
6. Clippy exact allowlist fix와 hunk-to-suggestion 검증
7. `rustfmt -> clippy fix -> rustfmt` convergence/idempotence
8. candidate pre-validation과 affected build/test
9. exact PatchSet apply와 post Gate
10. `personal_auto` policy approval
11. CLI-only E2E
12. Windows x64·ARM64, multi-crate, feature/target Corpus
13. release ToolDescriptor/Catalog/Schema와 독립 검토

각 slice는 contract/Schema migration, fixture/Corpus, unit·integration·CLI conformance와 negative case를 함께 추가한다. 단계 1~4의 read-only 결과가 불완전하면 이를 숨기지 않고 mutation slice의 blocker로 남긴다.

### 최소 Corpus와 수용 시나리오

| scenario | 기대 결과 |
|---|---|
| 이미 compliant한 workspace | `succeeded_no_change`; PatchApplication/source write 0 |
| rustfmt drift만 존재 | formatting-only immutable PatchSet, replay operation 0, pre/post check 통과 |
| 허용된 `MachineApplicable` Clippy suggestion | exact hunk mapping 뒤 적용, final fmt/Clippy/affected Check 통과 |
| allowlist 밖 lint | Diagnostic과 skip reason만 남고 source/Patch hunk 없음 |
| `MaybeIncorrect`/`HasPlaceholders`/unknown suggestion | `RUST_CLIPPY_SUGGESTION_NOT_MACHINE_APPLICABLE`, 자동 수정 없음 |
| feature set별 같은 code에 충돌 suggestion | conflict edge와 `BLOCK`; candidate 적용 없음 |
| inactive feature/cfg/다른 target | coverage partial, `AUTO_PASS`와 personal_auto 금지 |
| mutually exclusive feature | `--all-features` 없이 Catalog matrix cell별 실행 |
| explicit/inferred/mixed style edition | resolved source를 구분하고 parsing/style edition fingerprint가 다름 |
| toolchain/rustfmt/Clippy identity drift | PatchSet/Gate stale, resolve부터 새 실행 |
| rustfmt/Clippy component·target 없음 | unavailable, 설치/network 시도 없음 |
| dirty checkout overlap/unknown | auto apply 차단, target 기존 byte 보존 |
| generated/vendor file 변경 | complete diff에서 side-effect violation, PatchSet finalize 없음 |
| Cargo build script가 source root 수정 | side-effect violation; cleanup으로 성공 합성하지 않음 |
| 허용 suggestion과 대응하지 않는 Clippy hunk | candidate 전체 거부 |
| pipeline 두 번째 실행에서 diff | `RUST_STYLE_NON_IDEMPOTENT` |
| preview 검증 실패 | target source 불변, failed evidence 보존 |
| post Gate 실패/partial apply | success 아님, actual-after/reverse 자료와 recovery 상태 |
| `safe_default`와 `personal_auto` | 같은 PatchSet에 사용자 승인 대 policy exact ApprovalDecision 차이 증명 |
| Windows x64·ARM64 | path/case/process-tree/target coverage와 같은 contract 결과 |
| CLI-only graph | `star.exe` E2E dependency graph의 AI/OpenAI/browser dependency 0 |

추가 Corpus는 virtual workspace, root package가 있는 workspace, package `[lints] workspace = true`, nested config 후보, no lockfile/offline failure, required-features target, build script, proc macro, non-UTF-8 tool output, invalid/truncated JSON과 process timeout을 포함한다. exact Clippy version용 built-in allowlist를 출하하려면 그 version identity와 모든 positive/negative Corpus 결과, hunk mapping과 expiry를 Catalog item에 묶어 독립 검토한다. Corpus 없이 “일반적으로 안전한 lint”를 추측해 built-in allowlist를 만들지 않는다.

## 19. 설계 수용 조건

M11 구현은 다음을 모두 증명할 때만 완료다.

- `rust_style_auto_fix`가 C01의 16번째 Profile이고 core 기능 23개·runtime executable 4개가 유지된다.
- `star.exe` CLI만으로 inspect/check/prepare/auto-apply와 기존 patch 조회·복구 흐름을 완료하며 AI/OpenAI/browser/scheduler 호출이 없다.
- Git의 `.rs`/Cargo/rustfmt/Clippy/toolchain source와 versioned Catalog policy가 정본이고 DB는 삭제 후 재구축 가능한 derived projection이다.
- stable project-pinned toolchain, exact executable/config/style edition/policy/coverage가 PatchSet·Gate·Evidence에 bind된다.
- `--all-features`를 범용 기본값으로 사용하지 않고 package/target/feature/cfg coverage의 missing/limitation을 사실대로 보고한다.
- Clippy fix는 exact lint ID·`MachineApplicable`·exact span/replacement와 actual hunk 대응을 모두 증명하며 group/suppression은 자동 변경하지 않는다.
- external mutator가 live checkout에서 실행되지 않고 candidate의 complete filesystem diff가 handwritten `.rs` modify로만 구성된다.
- `rust_style_v1` replay가 operation 0이며 candidate fmt/Clippy/affected Check가 complete `AUTO_PASS`다.
- `personal_auto`가 standing grant만으로 apply하지 않고 exact PatchSet에 대한 ApprovalDecision, M3 pre Gate와 single-use M4 permit을 거친다.
- apply는 immutable PatchSet만 SourceMutationPort로 실행하며 post Gate/partial apply/evidence failure를 성공으로 표시하지 않는다.
- 모든 최소 Corpus와 Windows x64·ARM64 CLI-only conformance가 current release evidence로 통과한다.
- ToolDescriptor/Catalog/Schema/adapter fingerprint drift가 stale을 만들고 missing component/target를 설치하지 않는다.
- 독립 검토자가 새 문서와 referenced contract만으로 첫 수직 Slice의 type, state transition, failure와 test를 추가 결정 없이 구현할 수 있다.

문서 수용은 제품 완료가 아니다. 현재 M11 상태는 설계 확정·구현 전이며 P9 공개 배포 Gate는 M11 conformance evidence가 생기기 전까지 완료될 수 없다.

## 20. 공식 자료

다음 공식 자료를 2026-07-14에 재확인했다.

1. [Rust Style Guide](https://doc.rust-lang.org/stable/style-guide/) — default Rust style과 rustfmt 관계
2. [`cargo fmt`](https://doc.rust-lang.org/cargo/commands/cargo-fmt.html) — Cargo package/workspace formatter command와 rustfmt component
3. [rustfmt configuration](https://rust-lang.github.io/rustfmt/) — config option, stable/unstable 구분과 style edition
4. [RFC 3338: Rust Style Evolution](https://rust-lang.github.io/rfcs/3338-style-evolution.html) — parsing edition과 style edition의 독립 resolution
5. [Clippy usage](https://doc.rust-lang.org/stable/clippy/usage.html) — lint level, `#[allow]`, group 성격과 `cargo clippy --fix`
6. [Clippy configuration](https://doc.rust-lang.org/stable/clippy/configuration.html) — `clippy.toml` parameter와 lint-level source 경계
7. [Cargo manifest의 `[lints]`](https://doc.rust-lang.org/cargo/reference/manifest.html#the-lints-section) — manifest lint level/priority 정본
8. [Cargo workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html) — `[workspace.lints]`와 package inheritance
9. [`cargo fix`](https://doc.rust-lang.org/cargo/commands/cargo-fix.html) — suggestion 적용 한계, target/feature/cfg coverage와 edition migration option
10. [rustup toolchain file](https://rust-lang.github.io/rustup/overrides.html#the-toolchain-file) — `rust-toolchain.toml` resolution, component와 target pin
11. [Cargo features](https://doc.rust-lang.org/cargo/reference/features.html) — `--all-features`, additive feature 원칙과 mutually exclusive feature 제약
12. [Cargo build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html) — build 중 child code 실행과 output ownership
13. [Procedural macros](https://doc.rust-lang.org/reference/procedural-macros.html) — compile-time execution과 동일 resource 접근 위험
14. [rustc JSON output](https://doc.rust-lang.org/rustc/json.html) — Diagnostic span과 suggestion applicability 값
15. [`cargo metadata`](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html) — versioned machine-readable workspace/package/target/feature discovery

이 출처는 도구 동작의 근거이고 자동 수정 허가 목록 자체가 아니다. lint allowlist, coverage matrix와 auto-apply scope는 versioned Catalog/user policy가 별도로 명시하며, 공식 문서가 갱신돼도 기존 PatchSet의 captured identity와 fingerprint를 소급 변경하지 않는다.
