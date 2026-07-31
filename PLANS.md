# Star-Control 현재 작업 원장

## 목표와 불변식

- 범위는 P-0078~P-0086이다. source·manifest·Catalog가 정본이고 DB/index/cache는 derived state다.
- `partial|stale|unverified|flaky|not_run|outcome_unknown`과 operation `accepted|approval_required`를 PASS로 승격하지 않는다.
- Finding에서 source 변경은 `ChangeRecipeV2 → isolated PatchSetV2 → pre/post Gate → exact approval`을 거친다.
- package/tool 설치, Runtime 교체, 전역 Rules 적용, 실제 retention/compaction은 검증된 plan·receipt·fingerprint를 사용한다. Codex cache/trust DB와 Star-Control DB를 직접 수정하지 않는다.
- `target/`, linked worktree, 사용자 source와 `.ai-runs`를 정리하지 않는다. force push는 금지한다.

## 현재 Gate

| P-ID | 목표 | 현재 상태 | 다음 Gate |
|---|---|---|---|
| P-0078 | Ready 이후 lifecycle-bound retention, lease/cancel/join/checkpoint, hold·migration backup floor, 별도 compaction | 구현·focused PASS. 10,005행 generation을 10,000+5행 transaction으로 분할하고 commit/checkpoint 사이 replay 및 process restart를 검증했다. compaction은 verified `BackupApplyResult`와 실제 backup byte를 다시 결합한다. | 최종 source FULL 후 실제 DB read-only plan. 삭제는 exact plan 승인과 backup 뒤에만 적용한다. |
| P-0079 | 32/256 item, 65,536-byte hard limit, opaque revision cursor의 management status page | 구현·state/Controller/CLI focused PASS. legacy oversize와 stale cursor는 fail closed다. | 설치 Runtime cold status와 `--all` cap 실측. |
| P-0080 | help-first parsing, `command describe --json`, token-aware alias audit, recipe→change→patch lifecycle | 구현·CLI 29/29 PASS. canonical `patch.apply-v2`와 legacy alias를 descriptor에서 분리했다. | installed CLI/Skill introspection과 updater receipt hash 확인. |
| P-0081 | typed 8-Hook set, Rules audit, granular approvals | repository source 구현. `codex execpolicy check` 포함 policy validator PASS(`rules=16`, `hooks=8`). 전역 config/rules는 `C:\Users\thdqu\.codex\backups\star-control-p0081-2aa532a`에 백업한 뒤 granular approval과 exact project rules로 적용했다. | 새 Hook hash는 공식 설치 뒤 `/hooks` 또는 `hooks/list`에서 사용자 trust 검토한다. |
| P-0082 | changed executable-line coverage와 exact-input/environment flaky evidence | 계약·Schema·application normalizer·provider descriptor 구현, contract tests PASS. branch는 shadow이고 retry 성공은 PASS가 아니다. | current source live artifact 생성; protocol/result가 불완전하면 `unverified|partial` 유지. |
| P-0083 | Buf/oasdiff/cargo-semver/libabigail compatibility evidence와 architecture rules | `CompatibilityReportV2`, CLI `contract compare --providers`, layer/edge/cycle exception 계약 구현·focused PASS. STRICT에서 human/raw-only 결과의 classification 승격을 막고 application 수용 시 exception owner/expiry를 검증한다. | available provider별 exact artifact와 current subject binding 검증. |
| P-0084 | SBOM/advisory/license/VEX/provenance와 두-root reproducibility | `SupplyChainSnapshot` provider path와 `ReproducibilityVerificationReportV1` 구현. PASS는 두 root의 동일 artifact digest와 complete provider evidence에 exact 연결된 `slsa_provenance_ref`를 요구하며 human/raw-only protocol은 PASS에 기여할 수 없다. | Syft/cargo-deny/cargo-audit live evidence, isolated two-root build와 ReleaseManifest binding. |
| P-0085 | sanitizer/generator/doctest/Loom 및 bounded near-clone | unavailable/timeout/cancel/raw artifact와 advisory-only 계약·Schema·normalizer를 구현했다. built-in near-clone은 identifier/literal normalized 5-token-shingle SimHash, token-count ratio와 configurable threshold·candidate/pair cap으로 실제 Finding을 생성하고 recipe/PatchSet을 만들지 않는다. adapter 13/13, validation 50/50 PASS다. | 실제 프로젝트 선언/fixture가 있는 runtime provider만 실행한다. |
| P-0086 | final source·Runtime·Registry·Profile·Code Health·Evaluation seal 및 delivery | source FULL 13/13과 STRICT APPROVE 뒤 `2aa532ab3486b541667b0ed19cae73f90318d300`을 commit했다. 첫 offline installer restart는 bundled `resources\codex.exe` 잔존을 설치기가 감지해 파일 변경 전에 `rollback_required`로 중단했다. 종료 증명을 Desktop+bundled CLI로 강화한 회귀 테스트와 updater-core 20/20, Clippy가 PASS했다. | 현재 source evidence/FULL/STRICT 재봉인 → additive commit → verified installer 재빌드·공식 재시도 → installed operations → force-free push/readback. |

## 열린 위험

- 현재 provider discovery는 16개 descriptor 중 9개가 host에서 보이지만 모두 `unverified`다. executable/version/digest discovery는 analysis run·등록·protocol artifact를 증명하지 않는다.
- 전역 Rules/config는 backup 뒤 적용했지만 Runtime 설치는 이전 payload 그대로다. 첫 updater receipt는 `rollback_required`이고 설치기는 파일을 바꾸지 않았다. 보강 payload의 terminal receipt 전에는 설치 완료로 보지 않는다.
- Hook 8/8 trusted 확인과 실제 management DB retention/compaction은 아직 실행하지 않았다. official updater/operation receipt 뒤에 수행한다.
- 실제 DB read-only retention plan에서 보호 대상, 단일-row byte 초과 또는 migration backup floor가 나오면 삭제량이 줄 수 있다. 이를 숨기거나 정책을 약화하지 않는다.
- `slsa_provenance_ref`, reviewed VEX/reachability, project-declared sanitizer/Loom가 없으면 각각 재현성·not-affected·runtime-safety PASS를 만들지 않는다.
- public signed Stable은 certificate/private key/trusted timestamp와 별도 publish lifecycle이 없어 `blocked_external`이다.

## 이어서 실행할 순서

1. bundled Codex CLI 잔존 회귀 수정 뒤 `catalog/product-source-evidence.json`과 ignored live provider inventory를 재생성하고 inventory·Schema·policy/provider validator를 통과시킨다.
2. source FULL 13/13과 exact-tuple STRICT를 현재 diff에서 다시 통과시키고 additive local commit한다.
3. x64 stage/installer를 새 revision으로 검증·재빌드하고 candidate의 공식 updater transaction으로 설치한 뒤 exact receipt/hash, TARGET·Doctor·Registry·16 Profile·Code Health를 검증한다.
4. 실제 DB에는 read-only retention/compaction plan을 먼저 만들고, reclaim 가치·hold·backup을 검토한 뒤 exact fingerprint로만 apply한다.
5. 최종 commit을 force 없이 `origin/main`에 push하고 `git ls-remote origin refs/heads/main`으로 SHA를 readback한다.
