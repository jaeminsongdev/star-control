# E47 Stacked Merge Procedure

## 목표

M9v slice는 M0~M9 stacked PR을 실제로 ready/merge하기 전 사람이 승인할 수 있는 절차를 문서화한다. 이 slice는 PR ready 전환, PR merge, main update, release/deploy/publish, repository settings 변경을 실행하지 않는다.

## 선행 문서

```text
docs/implementation/audit/stacked-pr-readiness.md
docs/implementation/audit/final-completion-audit.md
docs/implementation/briefs/E46-final-evidence-refresh.md
```

## 허용 파일

```text
docs/implementation/**
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
docs/implementation/audit/stacked-pr-readiness.md
gh pr list --state open --limit 100 --json number,title,baseRefName,headRefName,isDraft,mergeStateStatus,url
gh run view <latest-top-branch-ci-run>
```

## 출력

```text
docs/implementation/audit/stacked-pr-merge-procedure.md
review order
merge execution order
pre-merge validation gates
stop conditions
explicit approval phrase
```

## 핵심 TASK

```text
bottom-up human review order 문서화
top-down stacked branch merge order 문서화
pre-merge verification command 문서화
merge 중 stop condition 문서화
explicit approval phrase 문서화
no-action/no-main-update boundary 문서화
```

## 완료 기준

- procedure가 review order와 merge execution order를 분리해서 설명해야 한다.
- procedure가 branch-to-branch stacked PR의 실제 merge 순서를 top-down으로 고정해야 한다.
- procedure가 `mergeStateStatus=CLEAN`, draft state, latest CI success, local validation command를 precondition으로 둬야 한다.
- procedure가 conflict, failed CI, unexpected non-draft, base/head discontinuity 발견 시 즉시 중단하도록 해야 한다.
- 이 slice는 PR ready/merge, main update, release/deploy/publish, destructive recovery action, repository settings 변경을 수행하지 않는다.

## 검증

```text
python scripts/ci/run_all.py
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
git diff --check
```

## 다음 handoff

이후에는 사용자가 explicit approval phrase로 승인한 경우에만 stacked PR ready/merge coordination을 수행한다. 승인 전까지 main update, PR ready/merge, release/deploy/publish, destructive recovery action은 RESERVED다.
