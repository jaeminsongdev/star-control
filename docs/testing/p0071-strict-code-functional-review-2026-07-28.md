# P-0071 STRICT 코드·기능 리뷰

## 범위와 판정 기준

- 기준 범위: `origin/main`의 `00bd842f95541d23267cdf250d157c3f1864670d`부터 P-0070 local HEAD `e5efc1138e76007efb4f7138388c01f261b24b41`까지 11개 commit과 P-0071 수정이다.
- 리뷰 모드: `STRICT`, risk `HIGH`.
- 확인 층: canonical contract, source/port/adapter, Controller·CLI route, positive/negative/failure corpus, generated inventory, 실제 source-built CLI와 validation Gate.
- 완료 의미: source 구현과 current evidence가 일치한다는 뜻이다. installed Runtime 갱신, external provider 등록, public signing·publish 완료를 뜻하지 않는다.

## 닫힌 Finding

| 심각도 | 영역 | 재현된 문제 | 수정과 회귀 증거 |
|---|---|---|---|
| Major | registered process | seal 뒤 args·budget을 바꾼 invocation도 실행됐고 host environment 전체를 상속했다. | 실행 직전 reseal exact 비교, 1일/64MiB 상한, bounded nonsecret environment allowlist와 fingerprint를 추가했다. post-seal mutation과 과대 budget corpus가 실행 전 거부를 확인한다. |
| Major | process output·SARIF | stderr가 truncated여도 stdout SARIF를 import했고, pipe descendant가 handle을 유지하면 reader join이 무기한 대기할 수 있었다. | 어느 stream이든 truncation/read failure면 import를 금지하고 bounded drain grace 뒤 `unverified`로 반환한다. timeout·unknown·stdout/stderr truncation corpus를 모두 고정했다. |
| Major | SARIF trust/privacy | byte cap이 없고 provider partial fingerprint를 raw correlation key로 유지했으며 drive-relative/file URI가 invalid `ProjectPathRef`로 투영될 수 있었다. | 8MiB/run/result/location/rule cap, 즉시 SHA-256 correlation, contract path validation, Windows·POSIX file URI corpus를 추가했다. |
| Major | complexity | chained `&&`/`||`를 ancestor마다 중복 집계하고 nested function의 branch를 outer metric에 합쳤으며 new/improved metric도 regression Finding으로 만들었다. | AST node별 operator 1회, nested function 분리, compatible baseline의 cyclomatic 증가만 `Warning` Finding으로 제한했다. exact 4/2/2 metric과 new/equal/improved/incompatible negative corpus를 고정했다. |
| Major | Git history | repository identity가 raw `.git` 문자열이라 서로 다른 저장소가 충돌했고, 고정 날짜로 debt expiry를 판단하며 commit cap truncation도 `complete`였다. | canonical common-dir opaque hash, caller RFC 3339 evaluation time, cap 도달 `partial`, 1..10,000 CLI/adapter 상한과 서로 다른 repo/limited history corpus를 추가했다. |
| Major | Git scan safety | `.git`와 link/reparse point를 따라 읽을 수 있었고 CODEOWNERS를 component마다 다시 읽었으며 debt recursion 전체 budget이 없었다. | `.git`·link/reparse skip, safe bounded CODEOWNERS single-read, depth 64/entry 100,000/marker 10,000 limitation을 추가했다. invalid expiry raw 값도 저장하지 않는다. |
| Major | mutation·Rule Pack | survivor budget 초과인 complete adverse evidence를 invalid/unknown으로 버렸고 retired Rule Pack도 trusted digest 후보가 됐다. | evidence completeness와 policy regression을 분리하고 count/budget coherence를 검증했다. active lifecycle만 exact tool digest에 bind한다. survivor 초과 current/complete/rank 3 및 retired negative corpus가 통과한다. |
| Major | registered effect | subject fingerprint를 `TaskInvocationV2`가 소유한 invocation fingerprint와 비교해 정상 `RegisteredToolExecutorAdapter` 호출을 전부 거부했다. | subject hash는 idempotency binding에 포함하고 invocation fingerprint는 `seal()`이 계산하도록 분리했다. 실제 current test executable을 통한 adapter 실행이 성공한다. |
| Major | repository posture | `starts_with("https://")`만 검사해 credential userinfo, 빈 authority, malformed port와 unbounded query를 허용했다. | 2KiB credential-free HTTPS authority, bounded query/schema와 DNS/IPv6/port corpus를 추가했다. |

테스트·정책·fixture를 약화하거나 실패를 skip한 수정은 없다.

## 실제 기능 증거

- `cargo test -p star-validation`: 47/47 pass.
- `cargo test -p star-adapter-rust-index`: 13/13 pass.
- `cargo test -p star-project`: 26/26 pass.
- `cargo test -p star-execution`: 4/4 pass.
- `star-application`: 19/19 pass(그 안의 Rule Pack, mutation, posture corpus 포함).
- `star-cli` bounded payload parser와 `star-controller` route ownership: 각 1/1 pass.
- `cargo clippy` affected packages/all targets with `-D warnings`: pass.
- `cargo run --locked -p star-cli -- --help`: source-built CLI가 maintenance, validation, release, evaluation, Profile route를 노출하며 exit 0.

current product inventory는 feature 23/23, Schema 217, MCP 170/170, Profile 16/16, Runtime executable 4/4로 통과했다. preliminary `FULL`은 `target/validation/20260727T163051012Z-25012/report.json`에서 174,588ms, 11/11 complete/stable/pass, partial/unverified/flaky 0으로 통과했다. 이 문서 seal을 포함한 source evidence를 다시 생성한 뒤 같은 Gate를 final delivery evidence로 한 번 더 실행한다.

## 남은 경계와 위험

- registered executor는 timeout 뒤 direct child를 종료하고 bounded drain으로 `unverified`를 반환하지만 Windows descendant process tree를 Job Object에 넣는 launcher는 아직 Controller runtime에만 있다. child-spawning validator의 side-effect containment는 공통 launcher 추출 또는 새 dependency 변경이 필요한 별도 승인 범위이며, 그 전에는 timeout evidence를 completion으로 승격하지 않는다.
- mutation, Rule Pack, repository posture와 semantic provider는 production provider가 등록되지 않으면 의도대로 `unavailable|unverified`다. fixture adapter 통과를 provider 설치 완료로 해석하지 않는다.
- installed Runtime은 이 source revision으로 재설치하지 않았다. 설치·restart·system integration 변경은 이번 push 승인에 포함되지 않는다.
- signed Stable, timestamp, installer lifecycle과 public publish는 기존 `blocked_external` Gate다.

이 위험들은 source review 결과를 숨기지 않으며, remote `main` 전달 뒤에도 release/product completion과 분리해 유지한다.
