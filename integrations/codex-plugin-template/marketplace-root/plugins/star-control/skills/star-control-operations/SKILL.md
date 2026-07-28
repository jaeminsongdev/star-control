---
name: star-control-operations
description: Use when Codex should plan, execute, validate, inspect code health, recover, update, install, or release development work through installed Star-Control. Route tasks through declarative Profiles, ready fixed-MCP actions, Catalog-declared CLI-only and installed local lifecycle commands, approvals, durable operations, and evidence; use native project tools only as an explicit fallback.
---

# Star-Control operations

## 1. Preflight

1. 현재 작업 범위와 금지 경로, 요구 산출물, 완료 조건, 승인 조건을 먼저 고정한다.
2. CLI 또는 Profile이 필요하면 `star installation status --json`, `star integration status --json`, `star management status --json`으로 source·설치·management identity를 분리한다.
3. management가 `recovery_only`이거나 결과가 `partial`, `stale`, `unverified`, `not_run`, `flaky`, `outcome_unknown`이면 그 상태를 보존한다. 설정이나 native 도구로 성공을 합성하지 않는다.

## 2. Profile resolution

1. 작업 종류를 분류해야 하거나 MCP가 없는 기능이 필요하면 [routing-matrix.md](references/routing-matrix.md)를 읽는다.
2. 적용 후보를 `star profile show <profile-id> --json`으로 확인하고, 여러 후보는 `star profile resolve <profile-id>... --json`으로 closure와 fingerprint를 고정한다.
3. reference와 live Catalog가 다르면 live `profile show|resolve` 결과를 우선하고 drift를 기록한다.
4. Profile resolution이 차단되면 closure를 추측하지 않는다. 안전한 native 작업은 계속할 수 있지만 Star-Control Profile evidence는 `unverified`로 남긴다.

## 3. Action routing

1. MCP-first 기능은 `star_tool_search`로 현재 작업에 맞는 action을 찾는다.
2. action readiness가 `ready`인 결과만 `star_tool_describe`로 다시 조회해 현재 Schema, risk lane, `descriptor_hash`와 `required_call_tool`을 확인한다.
3. 반환된 `required_call_tool`에 `tool_id`, `descriptor_hash`, `arguments`를 전달한다. `TOOL_DESCRIPTOR_STALE`이면 다시 describe한다.
4. MCP action이 `unavailable`, `untrusted`, `incompatible`, `degraded`이면 같은 기능의 CLI command로 우회하지 않는다.
5. routing matrix가 CLI-only로 표시한 기능만 설치된 `star` CLI의 선언된 command로 실행한다. `--help`와 JSON 결과에서 현재 Schema·승인·중단 상태를 확인한다.
6. ready MCP action도 CLI-only command도 사용할 수 없으면 프로젝트 native 도구로 fallback하고, 이유와 누락된 Star-Control evidence를 결과에 기록한다.
7. `star_tool_registry_status`는 Registry 진단에만 사용하며 action readiness 대신 사용하지 않는다.

## 4. Code Health

1. clone·complexity·unused surface·SARIF·semantic refactor·mutation·Rule Pack·Git history/ownership/debt·Radar 요청이면 [routing-matrix.md](references/routing-matrix.md)의 Code Health route를 읽는다.
2. live Registry에서 `project.register`, `scan.run`, `index.status|search`, `finding.list|diagnostic.list`를 각각 search→describe하고 ready action만 순서대로 호출한다. register·scan의 `write_closed`와 index·finding·diagnostic의 `read_closed`를 바꾸지 않는다.
3. scanner, index, Radar와 provider output은 evidence이며 source 변경 완료가 아니다. current Project·checkout·index·provider identity와 artifact digest가 결합된 결과만 사용한다.
4. executable 존재만으로 provider가 등록됐다고 보지 않는다. exact descriptor·protocol·result artifact가 없으면 `unavailable|unverified`로 유지하고 provider를 설치하지 않는다.
5. semantic refactor는 isolated preview와 typed PatchSet을 만든 뒤 기존 pre/post Gate·exact approval 경로로만 적용한다. mutation result나 Radar가 source를 직접 바꾸지 않게 한다.
6. Code Health 전용 17번째 built-in Profile을 추측해 추가하지 않는다. live 16개 Profile 조합을 resolve하고 별도 EvaluationRun·제품 결정이 있을 때만 profile decision을 반영한다.

## 5. Runtime and Codex integration updates

1. install·repair·update·rollback 요청이면 [routing-matrix.md](references/routing-matrix.md)의 update route를 읽고 source, installed fixed bridge, active Runtime generation, rendered Plugin/cache를 분리한다.
2. `star update inspect` 결과의 candidate class, approval scope와 `requires_codex_restart`를 먼저 확인한다. 일반 `integration status`의 `requires_new_task`를 candidate restart 판정으로 오해하지 않는다.
3. Runtime-only candidate는 installed Updater가 apply하고 Codex·fixed MCP/Hook identity를 유지한다. integration/bridge candidate는 inspect가 restart를 요구하고 사용자가 허용한 경우에만 전용 Updater restart transaction을 사용한다.
4. Codex cache, Plugin cache, Runtime selector, installation/management DB를 직접 수정하지 않는다. source/template 또는 release stage를 고친 뒤 verified updater·repair 경로로 다시 생성한다.
5. apply 시작이나 `restart_armed`를 완료로 보지 않는다. terminal receipt, active source revision, process identity, rendered/cache hash, management와 Doctor postcheck를 확인한다.
6. `hook_trust_required=true`이면 `hook_review_surface=codex_cli`, `hook_review_command=/hooks`를 확인하고 사람에게 Codex CLI Hook browser 검토를 안내한다. Desktop 설정 화면을 Hook browser로 표현하거나 trust state를 직접 쓰지 않는다.

## 6. Approval, operation, and evidence

1. `approval_required`와 `question_required`는 완료가 아니다. exact scope·fingerprint·expiry를 사용자 결정에 묶고 필요한 경우 `star_approval_resolve`로 기록한다.
2. Operation ID가 반환되면 `star_tool_operation_get`으로 terminal result를 확인한다. timeout·취소·outcome unknown을 성공으로 바꾸지 않는다.
3. 적용 뒤 실제 ChangeSet을 다시 수집하고 관련 Check를 실행해 Diagnostic, GateDecision, EvidenceBundle 또는 ReviewPack에 연결한다.
4. 유료 작업, destructive action, remote push·publish·deploy와 system setting 변경은 별도 사용자 승인이 없으면 실행하지 않는다.
5. 최종 보고에는 사용한 Profile resolution fingerprint, MCP/CLI/native route, 승인 상태, 검증 결과와 남은 위험을 구분한다.
