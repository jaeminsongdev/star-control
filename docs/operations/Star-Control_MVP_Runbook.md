# Star-Control MVP Runbook

## 1. 목표

이 Runbook은 처음 구현되는 Star-Control v0 fake flow를 실행하고 점검하는 기준이다. v0 fake flow는 완전 구현의 첫 검증 마일스톤이며, local/cloud provider나 daemon/API/UI를 포함하지 않는다.

정본 구현 순서는 `docs/implementation/codex-work-queue-current.md`와 `docs/implementation/complete-implementation-roadmap.md`를 따른다.

## 2. 준비

필수 확인:

```powershell
git --version
python --version
```

Cargo workspace가 추가된 뒤 필수 확인:

```powershell
cargo --version
rustc --version
```

## 3. 계약 검증

현재 repository 단계에서는 CLI runtime이 아직 없으므로 로컬 계약 검사를 먼저 실행한다.

```powershell
python scripts\ci\run_all.py
```

Cargo workspace가 생긴 뒤에는 구현 PR에서 아래 검증을 추가한다.

```powershell
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

## 4. v0 fake flow command shape

E08 이후 CLI가 준비되면 v0 fake flow는 다음 command shape를 기준으로 한다.

```powershell
$projectRoot = Join-Path $env:USERPROFILE 'star-control-demo\project-a'
star-control run --project $projectRoot --request "스톱워치 만들어줘" --provider fake-default --json
star-control status --project $projectRoot --job J-0001 --json
star-control report --project $projectRoot --job J-0001 --json
```

Approval flow가 준비되면 다음 command shape를 사용한다.

```powershell
star-control approve --project $projectRoot --job J-0001 --response approved --reason "reviewed" --json
star-control resume --project $projectRoot --job J-0001 --json
star-control cancel --project $projectRoot --job J-0001 --json
```

## 5. v0 완료 기준

- 대상 프로젝트 `.ai-runs/J-0001/` 아래 artifact가 생성된다.
- `route.json`, `workspecs/*.json`, `provider-output/fake-default/response.json`이 존재한다.
- Star Sentinel P0 validation output이 schema를 만족한다.
- final report 또는 stage report가 생성된다.
- RunState가 `DONE`, `WAITING_APPROVAL`, `BLOCKED`, `FAILED` 중 명확한 상태를 가진다.
- 위험 명령, dependency install, release/deploy, 외부 계정 변경이 자동 실행되지 않는다.

## 6. 실패 시 확인 순서

```powershell
star-control status --project $projectRoot --job J-0001 --json
Get-Content (Join-Path $projectRoot '.ai-runs\J-0001\run-state.json')
Get-Content (Join-Path $projectRoot '.ai-runs\J-0001\events.jsonl')
Get-Content (Join-Path $projectRoot '.ai-runs\J-0001\reports\final-report.json')
```

긴 provider log는 user-facing report에 붙이지 않고 `provider-output/` 또는 `tool-output/` artifact path로 추적한다.

## 7. Future / reserved commands

아래 command들은 완전 구현 후반의 후보이며, v0 fake flow에서 지원된다고 가정하지 않는다.

```powershell
$configRoot = Join-Path $env:USERPROFILE '.star-control'
star-control init --global $configRoot
star-control init --project $projectRoot
star-control validate schemas
star-control validate policies
star-control provider check codex
star-control render codex --dry-run
star-control render codex --apply
star-control run --project $projectRoot --request "스톱워치 만들어줘" --provider codex
```

Codex CLI, Claude Code, Gemini CLI 같은 cloud CLI provider는 v0 fake flow와 local provider가 안정화된 뒤 provider별 공식 문서 refresh, credential policy, budget guard, approval gate를 확인하고 별도 PR로 구현한다.
