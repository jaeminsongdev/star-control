# P-0077 목표추진 Codex App thread 관제·Controller bounded status 설치 폐쇄 — 2026-07-30

## 판정

- `orchestrate-parallel-implementation`은 collaboration subagent가 아니라 명시 승인된 Codex App `create_thread` project worktree를 사용한다. Sol Max는 설계·DAG·worker 전체 diff·combined 전체 diff를 소유하고, Terra High는 activation 직후 token budget 없는 Goal을 만든 뒤 하나의 Bundle 구현·검증·교정만 소유한다.
- bootstrap은 `BOOTSTRAP_ONLY`와 identity 확인을 거쳐 같은 `threadId`에 `ACTIVATE_BUNDLE`을 보내는 2단계다. `clientThreadId` only는 bounded `list_threads` unique match 전까지 fail-closed하며, correction은 새 task/Goal이 아니라 같은 task·같은 Goal의 `EXISTING_GOAL_RESUMED`다.
- exact 12개 forward scenario는 production contract validator와 inventory checker가 순서·문구·각 scenario의 negative semantic mutation까지 검사한다. source FULL의 `validation-contract`와 `product-inventory`가 모두 통과했다.
- 7.12GB management DB 때문에 Ready 뒤 `management.status`가 10초 IPC budget을 넘던 원인은 deep `verify_all` 호출이었다. status는 bounded read-only `status_all`로 분리하고 explicit deep verification은 보존했다. 설치본 cold 요청은 40~449ms에 typed `MANAGEMENT_STORE_BUSY/retryable=true`, Ready 뒤 성공 응답은 74ms였다.
- source FULL 11/11, Sol Max combined STRICT P0~P3 0건, verified x64 stage·공식 offline updater·Runtime reconcile, installed TARGET 8/8, Doctor 4/4, Profile 16/16과 rendered/cache Skill identity를 확인했다. 설치 source는 `567ca04432639d76456610676764ec46a47657c3`이며 이 문서를 포함하는 뒤따른 final HEAD는 docs/source-evidence seal이다.

## 역할·작업·Goal 증거

| 역할 | Codex App task | exact 결과 |
|---|---|---|
| Sol Max 설계 | `019fae06-eb58-7391-8896-a82d0d6dcd3d` | Skill thread lifecycle과 bounded status Bundle 경계·소유권·검증을 설계했다. |
| Terra High A | `019fae13-a038-7453-9d8b-e3e3c0ca228e` | baseline `8f14ebb474083ba5566b91d03c7254bf43efe7cd`, HEAD `ab2a30922e3f738e6838c6e4954488a076104580`, 15 files, diff fingerprint `cd7ac5d40974a11c5317a9062c065bec43fc9aea6e830239759566f2ca9fc2fd`; 같은 Goal에서 교정 후 `complete`. |
| Terra High B | `019fae13-a045-79b2-95c9-7a0c8913dd28` | baseline `8f14ebb474083ba5566b91d03c7254bf43efe7cd`, HEAD `4eecf23bc66426089a7fac9c708cf04c339d8359`, 6 files, diff fingerprint `281df9e08a8bd91868aa1684d7ea1563f8e849ca`; blocked 상태를 같은 task/Goal에서 재개해 `complete`. |
| Sol Max 전체 검토 | `019fae2e-7375-7422-b449-66cff5c40c66` | A/B exact 전체 diff 각각 승인, 중앙 interaction 검토, final HEAD `567ca044...`에서 P0 0·P1 0·P2 0·P3 0 `APPROVE`. |

두 worker는 `WORKER_COMPLETE` 뒤 Sol review를 polling하지 않았다. Controller가 승인과 correction을 같은 task에 전달했고, exact baseline·HEAD·fingerprint 승인 뒤에만 각 기존 Goal을 `complete`로 닫았다. 중앙 관제 task를 Skill이 별도 Goal로 만드는 계약은 구현하지 않았다.

## Skill·12개 forward scenario

- canonical source는 `integrations/codex-plugin-template/marketplace-root/plugins/star-control/skills/orchestrate-parallel-implementation/`의 9개 구성 파일이다. `SKILL.md`, 4개 reference, 3개 report/context asset, agent metadata가 같은 계약을 공유한다.
- `scripts/validation/contract-tests.ps1`는 numbered 1..12 mapping과 각 scenario를 하나씩 변조한 negative candidate 12개를 모두 거절한다.
- `scripts/validation/check_product_inventory.py`는 canonical Skill, architecture/installation contract, renderer/CLI surface가 같은 12개 의미와 2단계 lifecycle을 유지하는지 검사한다.
- Rust renderer 검증은 9개 구성 모두에서 actual non-schema `create_thread` alias와 collaboration API 이름을 거절한다. 설치 cache의 `SKILL.md`는 `create_thread`, `gpt-5.6-terra/high`, `gpt-5.6-sol/max`, `clientThreadId` fail-closed, activation 뒤 `create_goal`, same-thread correction을 포함하고 `spawn_agent|collaboration.spawn` match는 0건이다.
- source, rendered Marketplace, Codex Plugin cache의 installed `SKILL.md` SHA-256은 모두 `8b5cf45056b57c7591839630c0983f90f994b80442aece7d07546551a848a995`다. integration record는 plugin `0.1.0+codex.2c7a7693b178`, render `sha256:f14cd894c6c3f1bc171d32cdb59abbd70b7a9e25f782300792dae84eb642b0b9`, `registered`다.

## Controller IPC unavailable 교정

기존 installed Runtime `rt_a2b3b27d764e6eba`는 management 초기화를 background로 옮겨 cold bootstrap을 non-blocking으로 만들었지만, Ready 뒤 `management.status → verify_stores → verify_all`이 7,123,798,516-byte SQLite store의 `quick_check`와 event-chain deep verification을 수행해 약 10.3초 뒤 IPC frame 실패를 냈다.

product-code commit `286b9021b190674e2be3f7dd89f893e3df6074d0`은 다음 경계를 적용한다.

- `management.status`는 global/project store metadata를 bounded read-only snapshot으로 반환한다.
- 호출 전후 `last_verified_at`, active-set bytes와 revision은 바꾸지 않는다.
- explicit verification/recovery 경로는 기존 deep integrity 검사를 계속 소유한다.
- Controller `Initializing`은 `MANAGEMENT_STORE_BUSY`, `retryable=true`를 반환하고 production IPC budget 10초는 늘리지 않는다.

새 Runtime `rt_801e0b34cbe7f7f6`의 cold recovery 동안 6회 BUSY 응답은 40~449ms였고 malformed/unavailable로 승격되지 않았다. Ready 뒤 `management.status`는 74ms, `open_mode=normal`, `recovery_required=false`, global/project 두 store 모두 `integrity_state=healthy`, `open_mode=read_write`였다.

## 검증·패키징·설치 증거

| 항목 | 결과 |
|---|---|
| product inventory | feature 23/23, Profile 16/16, Runtime executable 4/4, Schema 217, MCP 170/170, stable error 533 |
| source FULL | `target/validation/20260729T161602226Z-42912/report.json`, revision `567ca044...`, 11/11 complete·stable·pass, 192,324ms, report `sha256:cf5fde682da4b9bc082a32e73e2ad846e363da4b9025dd31a76fbe284875772a` |
| Sol Max STRICT | combined review `APPROVE`, P0/P1/P2/P3 모두 0건 |
| x64 stage | 570 files, set `sha256:519985f7fb5695e6ab247cf4e2aed4bfa8b1f10848e6226fd4c1dc75bac8727b`, source `567ca044...`, verifier `verified=true`, `unsigned_local` |
| release manifest | stage/root byte-identical `sha256:468070b28e4bbb6129e526c1ec66123d8fa0d65186513b0b69fd446616580293` |
| installer | `star-control-windows-x64-0.1.0-setup.exe`, 27,826,757 bytes, `sha256:e9f4f820254e8c2ffa7f820bf8d3ec82ab7158508245e34ac93f16e28b4da0d0`, Authenticode `NotSigned` |
| offline restart | operation `upd_1qajhFbPixjmfCD8QgJI814PXNGtK4PYiZ2-TSanSj4`, fixed payload 적용 뒤 receipt `partially_applied`; 성공으로 재작성하지 않음 |
| candidate reinspection | `candidate_class=no_change`, changed files 0, restart false, candidate manifest `sha256:468070...` |
| Runtime reconcile | operation `upd_BN4TIPovl09Ly2F9CBy6p-NYzX5f0fUxFocpfNAh4f4`, `rt_a2b3... → rt_801e0b34cbe7f7f6`, activation revision 29, integration verified, fallback terminated PID 0 |
| active Runtime manifest | `sha256:fb62f9188afd32c208175016265c00ab83c374c7d10c073ed978ca46019a7ce9`; running Controller path가 active generation과 일치 |
| installed Profile | 16/16, Registry revision 14, catalog fingerprint `sha256:240fc06b8d4843db32c66460030f92ff78bc7b83f8e90f39185727eb0268987e` |
| installed Doctor | live search→describe→read_closed, 4/4 pass, implemented Controller command 23 |
| first installed TARGET | `target/validation/20260730T014639351Z-44868/report.json`, 7/8 fail; 동시 Cargo target access-denied와 함께 GitHub adapter test 2개가 실패, report `sha256:428002...` 보존 |
| 격리 재현 | GitHub adapter single-thread 5/5, 기본 병렬 suite 30회 연속 5/5 pass; 외부 Cargo 0개 상태에서 전체 TARGET 재실행 |
| final installed TARGET | operation `opn_01KYRBCQMHPQ7YC6D2BJCGZY19` terminal `succeeded`; `target/validation/20260730T014931696Z-26092/report.json`, 8/8 complete·stable·pass, 125,377ms, evidence `sha256:ba53379e3dc63b924acb678fb53d06885e930fddf52dfa3f52f8fcb77d073e87` |

첫 TARGET의 fixed MCP 선택 인자 오류와 첫 Doctor의 process-only timeout 인자 오류는 action 실행 전 `-32602`/`TOOL_ARGUMENT_INVALID`로 거절됐다. 최소 exact Schema 인자로 다시 호출한 결과만 final evidence이며, 앞선 오류를 PASS로 계산하지 않는다.

## 남은 경계

- 설치·검증은 Windows x64와 local unsigned package만 소유한다. Authenticode certificate/private key/trusted timestamp, signed publication, ARM64 증거는 없다.
- integration은 `verified/registered`지만 `requires_new_task=true`, `hook_trust_required=true`, `hook_review_surface=codex_cli`, `hook_review_command=/hooks`를 보존한다. 제품과 AI는 Codex trust DB/cache를 직접 수정하지 않는다.
- source/template, verified installer와 official updater/reconcile만 사용했다. 기존 linked worktree, `target/`, management DB, Runtime selector, Codex Plugin cache를 직접 정리·수정하지 않았다.
- 설치 Runtime은 package source `567ca044...`에 묶인다. 이 문서와 최종 source-evidence commit은 제품 실행 바이트를 바꾸지 않는 별도 closure seal이며, remote `origin/main` readback은 Git remote 상태와 최종 handoff가 소유한다.
