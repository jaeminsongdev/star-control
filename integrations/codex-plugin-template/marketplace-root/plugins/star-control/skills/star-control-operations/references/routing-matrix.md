# Star-Control routing matrix

이 파일은 Codex의 작업 분류용 derived reference다. 실제 command, Profile closure, descriptor version과 fingerprint는 설치된 `star` CLI와 live Registry/Catalog가 소유한다. 불일치하면 이 표를 근거로 실행하지 말고 drift를 기록한다.

## 기능 route

`MCP-first`는 search 결과가 ready일 때만 describe/call한다. `CLI-only`만 MCP 부재 시 설치된 `star` command를 사용한다.

| ID | route | primary surface |
|---|---|---|
| A01 | MCP-first | `goal.start`, `goal.status`, `goal.pause`, `goal.resume`, `goal.cancel` |
| A02 | CLI-only | `stage.plan`, `stage.show`, `stage.replan`, `stage.result.record`, `stage.result.show` |
| A03 | MCP-first | `project.list`, `project.status`, `project.register`, `scan.run`, `index.status`, `index.search` |
| A04 | MCP-first | `validation.plan` |
| A05 | CLI-only | `codex.capability.inspect`, `route.decide`, `route.show` |
| A06 | MCP-first | `run.continue` |
| A07 | mixed | `goal.status`, `goal.pause`, `goal.resume`, bounded `management.status.page`는 MCP-first; retention·backup·restore·compaction은 `command describe`로 확인한 CLI-only 승인 경로 |
| A08 | MCP-first | `star_approval_resolve` |
| A09 | MCP-first | `merge.status`, `handoff.get` |
| A10 | MCP-first | `star_tool_search`, `star_tool_describe`, `star_tool_registry_status` |
| B01 | MCP-first | `validation.plan`, `validation.run`, `evidence.get`, `finding.list` |
| B02 | MCP-first | `validation.run`, `diagnostic.list` |
| B03 | MCP-first | `validation.run` |
| B04 | CLI-only | `contract.snapshot`, `contract.compare`, `registry.declaration.plan` |
| B05 | CLI-only | `security.inspect`, `deps.scan`, `deps.prepare`, `maintenance.radar`, `maintenance.radar.code-health`, `maintenance.radar.git-history`, `maintenance.rule-pack` |
| B06 | CLI-only | `failures.inspect`, `failures.reproduce`, `failures.recovery-plan` |
| B07 | MCP-first | `doctor.run` |
| B08 | CLI-only | `migration.execute`, `migration.resume`, `migration.rollback`, `performance.run`, `performance.compare`, `language-migration.cutover` |
| B09 | CLI-only | `release.candidate.create`, `release.verification.record`, `release.promote`, `release.status` |
| C01 | CLI-only | `profile.list`, `profile.show`, `profile.resolve` |
| D01 | MCP-first | `merge.status`, `handoff.get` |
| D02 | CLI-only | `evaluation.run`, `evaluation.compare`, `evaluation.profile.decision`, `cost.record`, `budget.snapshot` |
| D03 | CLI-only | `release.lifecycle.publish`, `release.publish.prepare`, `release.status`, `release.audit` |

## Profile 선택

여러 행이 적용되면 ID를 함께 `profile resolve`에 전달한다. `default stop`은 성공 보장이 아니라 기본 중단점이다.

| Profile | trigger | default stop |
|---|---|---|
| `project_understanding` | full/incremental 프로젝트 이해와 ContextPack | `context_ready` |
| `docs_config_environment` | 문서·설정·환경 drift와 doctor | `documentation_gate_decided` |
| `change_planning` | read-only scope·impact·risk 계획 | `plan_ready` |
| `test_correctness` | test·fixture·snapshot·bug fix 신뢰성 | `correctness_gate_decided` |
| `architecture_quality` | package/layer/boundary/cycle 변화 | `architecture_gate_decided` |
| `ai_development_validation` | Codex가 만든 변경의 실제 diff 검증 | `goal_gate_decided` |
| `refactor_codemod` | Recipe·selector 기반 PatchSet | `awaiting_apply_approval` |
| `api_contract_change` | API·CLI·Schema·format·config 호환성 | `compatibility_decided` |
| `rust_style_auto_fix` | pinned rustfmt·allowlisted Clippy PatchSet | `prepared` |
| `debug_recovery` | 실패 재현·원인 격리·복구 | `recovery_decided` |
| `security_supply_chain` | secret·auth·dependency·license·release material | `offline_snapshot_decided` |
| `dependency_upgrade` | manifest/lockfile update 후보 | `awaiting_apply_approval` |
| `data_config_db_migration` | data/config/DB/state/file-format migration | `awaiting_approval` |
| `performance_build` | opt-in workload 비교 | `comparison_decided` |
| `language_platform_migration` | language/runtime/SDK/OS/architecture cutover | `awaiting_cutover_approval` |
| `ci_release_deploy` | CI·artifact·install·publish readiness | `ready` |

## Code Health route

| 작업 | route | 필수 경계 |
|---|---|---|
| SARIF·clone·complexity·unused surface | `project.register` → `scan.run` → `index.status|search` → `finding.list|diagnostic.list`를 MCP-first로 검색 | current Project·checkout·index와 source-bound evidence가 없으면 `unverified` |
| Code Health Radar | CLI-only `maintenance radar code-health` | read-only projection이며 source 수정 완료가 아님 |
| Git history·ownership·debt | CLI-only `maintenance radar git-history` | bounded revision range와 redacted author identity 필요 |
| Rule Pack·mutation evidence | CLI-only `maintenance rule-pack`; mutation provider는 registered adapter only | exact pack/provider/tool/config/artifact digest가 없으면 `unavailable|unverified` |
| changed-line coverage·flaky | Catalog descriptor → source-bound provider artifact → application normalizer → 기존 Gate/Radar | line coverage만 Gate; branch는 shadow, retry 성공은 PASS가 아니며 exact input/environment 혼재만 flaky |
| compatibility·architecture | CLI-only `contract compare`, `maintenance rule-pack` + provider evidence | structured machine detail만 정규화; exit-only/human log에서 진단을 합성하지 않음 |
| SBOM·advisory·VEX·provenance | CLI-only `security inspect` + `SupplyChainSnapshot`/ReleaseManifest binding | reviewed VEX·reachability 없이 `not_affected` 금지, freshness·limitation 보존 |
| reproducibility·runtime safety | project-declared validation/evaluation provider | 서로 다른 두 root digest와 complete provider evidence에 exact 연결된 `slsa_provenance_ref`가 PASS 조건; toolchain/test가 없으면 설치하지 않고 `unavailable` |
| near-clone | bounded built-in scan → `star.rule.near-clone-candidate` advisory Finding | identifier/literal normalized 5-token-shingle SimHash, token-count ratio, configured threshold·candidate/pair cap을 사용하고 자동 PatchSet 생성 금지 |
| semantic refactor | `recipe list|describe|validate` → `change prepare` → `patch show|apply|status|recover` | provider 부재 시 결과를 합성하지 않고, 구형 project-scoped patch alias는 descriptor의 deprecated 경고를 보존 |
| Profile evaluation | CLI-only `evaluation run`, `evaluation compare`, `evaluation profile-decision` | 기존 16 Profile 유지; EvaluationRun과 별도 제품 결정 전에는 새 built-in을 추가하지 않음 |

## Runtime update route

설치·integration·update command는 product action이 아니라 installed Bootstrap의 local lifecycle surface다. `star command describe <dotted-command> --json`에 현재 선언된 command만 사용하고 `star --help`는 사람용 usage 보조로 제한한다.

| candidate | apply route | Codex restart |
|---|---|---|
| Runtime-only generation | `update stage` → `update inspect` → generation `update apply` | inspect가 `false`이면 금지 |
| Codex integration/Bridge | complete release stage `update inspect` → absolute-stage `update apply` | inspect가 `true`이고 exact approval이 있을 때만 전용 Updater가 수행 |
| Updater-only·mixed·offline install | online integration apply로 우회하지 않음 | declared offline Updater transaction과 별도 승인 필요 |

- `restart_armed`, `approval_required`, process spawn과 candidate copy는 terminal success가 아니다.
- source revision, active Runtime, fixed Bridge, rendered marketplace, Codex cache와 management state를 각각 검증한다.
- Runtime selector, Codex cache·Plugin cache와 Runtime/management DB를 직접 수정하지 않는다.

## 공통 중단 규칙

- `approval_required`, `question_required`, Operation ID, `ready` manifest는 완료가 아니다.
- `partial`, `stale`, `unverified`, `not_run`, `flaky`, `outcome_unknown`을 pass로 승격하지 않는다.
- remote effect와 여러 repository 결과는 participant별 receipt·partial·recovery 상태로 유지한다.
