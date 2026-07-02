# E46 Final Evidence Refresh

## 목표

M9u slice는 M9s/M9t 이후 stale해진 final completion audit과 stacked PR readiness evidence를 최신 구현 스택으로 갱신한다. 이 slice는 구현 기능을 추가하지 않고, M0~M9 완료 증거와 stacked PR review/merge coordination 증거를 현재 open PR stack에 맞춘다.

## 선행 문서

```text
complete-implementation-roadmap.md
codex-work-queue-current.md
docs/implementation/audit/final-completion-audit.md
docs/implementation/audit/stacked-pr-readiness.md
```

## 허용 파일

```text
docs/implementation/**
examples/release-contracts/**
PLANS.md
README.md
```

## 금지 파일

```text
GitHub workflow
schema field 변경
Rust code
Cargo.toml
Cargo.lock
Cargo 외 package manager
새 external dependency
provider execution
provider live call
release/deploy/publish automation
repository settings 변경
destructive recovery action
main branch update
PR ready/merge action
```

## 입력

```text
gh pr list --state open --limit 100 --json number,title,baseRefName,headRefName,isDraft,mergeStateStatus,url
gh pr view 87 --json number,title,url,isDraft,mergeStateStatus,commits
docs/implementation/audit/final-completion-audit.md
docs/implementation/audit/stacked-pr-readiness.md
examples/release-contracts/complete-implementation-readiness.example.json
examples/release-contracts/stacked-pr-readiness.example.json
```

## 출력

```text
updated final completion audit evidence through M9t
updated stacked PR readiness evidence through PR #87
schema-valid release readiness examples
M9u handoff record
```

## 핵심 TASK

```text
final completion audit snapshot을 M9t/#87/CI run 기준으로 갱신
stacked PR readiness table을 #33~#87로 갱신
machine-readable ReleaseReadiness examples 갱신
brief/work queue/roadmap/PLANS 참조 갱신
approval-gated actions reserved 유지
```

## 완료 기준

- `docs/implementation/audit/final-completion-audit.md`가 M9t CLI sentinel command group과 PR #87 evidence를 포함해야 한다.
- `docs/implementation/audit/stacked-pr-readiness.md`가 #33~#87 contiguous clean draft stack을 설명해야 한다.
- `examples/release-contracts/complete-implementation-readiness.example.json`과 `stacked-pr-readiness.example.json`은 `release-readiness.schema.json`을 만족해야 한다.
- `ready` status, PR merge, main update, release/deploy/publish, destructive recovery action, repository settings 변경은 수행하지 않는다.

## 검증

```text
python scripts/ci/run_all.py
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
git diff --check
```

## 다음 handoff

M9v는 explicit approval을 받은 뒤 stacked PR ready/merge coordination을 수행하거나, 별도 승인된 destructive recovery/release action surface를 작은 slice로 다룬다. 승인 전까지 main update, PR ready/merge, release/deploy/publish, destructive recovery action은 RESERVED다.
