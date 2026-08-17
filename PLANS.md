# Star-Control 현재 작업 원장

## 목표와 불변식

- 현재 범위는 P-0078~P-0087이다. source·manifest·Catalog가 정본이고 DB/index/cache는 derived state다.
- `partial|stale|unverified|flaky|not_run|outcome_unknown`과 operation `accepted|approval_required`를 PASS로 승격하지 않는다.
- Finding에서 source 변경은 `ChangeRecipeV2 → isolated PatchSetV2 → pre/post Gate → exact approval`을 거친다.
- Runtime·Rules·retention·compaction은 공식 updater/operation과 exact receipt·fingerprint로만 변경한다. Codex cache/trust DB와 Star-Control DB는 직접 수정하지 않는다.
- 사용자 source, linked worktree와 `target/`은 정리하지 않으며 force push를 금지한다.

## 현재 Gate

| P-ID | 목표 | 현재 상태 | 남은 Gate |
|---|---|---|---|
| P-0078 | Ready 이후 lifecycle-bound retention, lease/cancel/join/checkpoint, hold·migration backup floor, 별도 compaction | source와 설치 Runtime에서 active operation idle grace·crash/restart·hold·bounded batch를 구현했다. 실제 7.12GB store는 official backup set `bks_01KYW284RE97ZEMH5KJ3AZY2HP`를 생성했고 read-only compaction plan은 예상 reclaim 0이라 apply하지 않았다. 설치 Runtime cold open 뒤 retention plan `sha256:1c9152a7ff7f135331b4e40363d5d31feb6b57d45192e92966b243291de6a3c3`은 후보 0행/0바이트다. | risk-lane 보강 source로 재설치한 뒤 동일 zero-delete plan을 readback한다. |
| P-0079 | 32/256 item, 65,536-byte hard limit, opaque revision cursor의 management status page | legacy oversize·stale cursor를 fail closed하고 Controller/CLI `--all` 총량 cap을 구현했다. 설치 Runtime의 2개 store는 모두 healthy이고 `--all` 응답 1,689바이트, 2/2건, 비절단이며 4,096건/8MiB cap을 공개한다. Finding projection은 result 64,910바이트, 60/4,099건이다. | risk-lane 보강 Runtime에서 동일 page/read budget을 재확인한다. |
| P-0080 | help-first parsing, `command describe --json`, token-aware alias audit, recipe→change→patch lifecycle | canonical/legacy route, descriptor hash, Skill introspection과 updater terminal receipt 검증을 구현했다. 최종 dogfood에서 suffix 추정이 MCP `doctor.run`, `project.register`와 CLI-only `maintenance.radar.*`의 lane을 오분류하는 결함을 찾아 exact MCP lane과 effect-aware/fail-closed CLI lane으로 보강했다. `star-cli` 32/32, TARGET 10/10, FULL 13/13이 통과했다. | current source evidence·STRICT를 봉인하고 새 installer의 descriptor를 readback한다. |
| P-0081 | typed 8-Hook set과 Rules audit | typed PreToolUse deny와 PostToolUse terminal-state context를 구현했고 Codex `hooks/list` 실측은 8/8 `enabled=true`, `trusted`, warnings/errors 0이다. Codex 실행 approval 설정은 P-0087의 사용자 고정 정책이 소유한다. | Hook hash가 바뀔 때만 동일 live 검토를 다시 수행한다. |
| P-0082 | changed executable-line coverage와 exact-input/environment flaky evidence | 계약·Schema·normalizer·descriptor와 현재 설치 도구 binding을 구현했다. branch는 shadow이고 retry 성공은 PASS가 아니다. | current source live artifact가 없거나 protocol이 불완전한 provider는 `unverified|partial`로 유지한다. |
| P-0083 | Buf/oasdiff/cargo-semver/libabigail compatibility evidence와 architecture rules | `CompatibilityReportV2`, provider evidence, layer/edge/cycle exception owner·expiry 검증을 구현했다. human/raw-only 결과는 상세 진단이나 PASS로 승격하지 않는다. | available provider별 exact executable/protocol/artifact binding을 검증한다. |
| P-0084 | SBOM/advisory/license/VEX/provenance와 두-root reproducibility | 공급망 normalizer와 `ReproducibilityVerificationReportV1`을 구현했다. reviewed VEX 없이 `not_affected`, exact two-root digest/SLSA binding 없이 reproducible PASS를 만들지 않는다. | live SBOM·advisory·two-root evidence가 없으면 `unverified|unavailable`로 유지한다. |
| P-0085 | sanitizer/generator/doctest/Loom 및 bounded near-clone | timeout/cancel/raw artifact와 advisory-only near-clone을 구현했다. project toolchain/fixture가 없으면 의존성을 추가하지 않고 unavailable을 반환한다. | 실제 프로젝트 선언이 있는 provider만 current fingerprint에서 실행한다. |
| P-0086 | source·Runtime·Registry·Profile·Code Health·Evaluation 최종 봉인과 delivery | 설치 Runtime `6cabb77dbb4e8538aa4879a286f2d907fcfaefe0`에서 Doctor 4/4, Registry revision 16, Profile 16/16 resolve, MCP full scan `scn_01KYWG76HQK1JFGG87A9QHN6WJ`과 직접 CLI incremental scan `scn_01KYWGM59Z3GMZXRQ4C83CAVXW`이 terminal succeeded다. index는 current이고 code-health/git-history Radar는 limitation을 보존해 `partial`이다. tracked `evals/`와 live `evaluation_run_v2`는 0이라 Evaluation shadow는 `not_run`이다. risk-lane 보강 source는 FULL 13/13이 통과했다. | current evidence/STRICT → commit/installer/update → live descriptor/Hook readback → force-free push 순서로 닫는다. |
| P-0087 | Codex 무프롬프트·full-access 고정 정책 | project/global config를 `approval_policy="never"`, `sandbox_mode="danger-full-access"`로 통일하고 inert reviewer·prompt Rules를 제거한다. deny-only Rules, typed Hook, AGENTS·Skill·architecture 문서와 validator를 같은 계약으로 맞춘다. | 새 Codex 작업에서 effective policy를 readback하고 source Skill 변경은 공식 updater receipt로 설치본에 반영한다. |
| P-0088 | 대형 management store cold-start와 Profile closure 차단 제거 | 설치 Runtime에서 12.53GB project store가 `MANAGEMENT_STORE_BUSY`를 15분 이상 유지하고 Profile이 차단되는 상태를 재현했다. clean startup의 중복 deep verification, generation별 retention 반복 scan, DB 비의존 Profile command의 management gate 결합을 source에서 분리한다. | 회귀 테스트 → TARGET/FULL → STRICT → local commit → 공식 Runtime update inspect/apply → live management/Profile/fixed MCP readback 순서로 닫는다. |

## 열린 위험

- public signed Stable은 certificate/private key/trusted timestamp와 publish lifecycle이 없어 `blocked_external`이다.
- provider descriptor 16개 중 host executable이 발견된 항목도 exact protocol·artifact binding 전에는 `unverified`다.
- Code Index의 parser/rust-analyzer limitation과 `partial` completeness는 숨기지 않으며, current scan에서 실제로 해소된 항목만 갱신한다.
- Code Health Radar 1,874건과 Git history Radar 147건은 모두 `partial`이다. tracked evaluation corpus와 human ground truth가 없어 EvaluationRun을 임의 생성하지 않는다.
- 실제 DB retention 삭제·compaction은 보호 대상과 예상 reclaim이 0인 현재 plan에서 실행하지 않는다. 정책을 약화해 효과를 만들지 않는다.
- Windows incremental cache finalize의 access-denied note는 테스트 종료 코드를 실패시키지 않았지만 재사용 불가 환경 신호로 남긴다. `target/`을 정리해 숨기지 않는다.
- P-0088 source가 검증되기 전에는 live management DB·`writer.lock`·Runtime selector를 직접 수정하지 않으며 현재 Profile closure는 `unverified`로 유지한다.
