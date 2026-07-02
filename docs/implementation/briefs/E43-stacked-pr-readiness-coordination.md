# E43 Stacked PR Readiness Coordination

## 목표

M9r slice는 M0~M9 stacked PR chain이 review/merge coordination에 들어갈 수 있는지 검토할 수 있도록 machine-readable readiness example과 사람이 읽는 stack evidence 문서를 추가한다. 이 slice는 PR merge, main update, release/deploy/publish, repository settings 변경을 실행하지 않는다.

## 선행 문서

```text
complete-implementation-roadmap.md
release-readiness.md
testing-ci-release.md
docs/implementation/audit/final-completion-audit.md
docs/implementation/audit/stacked-pr-readiness.md
docs/decisions/0005-full-implementation-defaults.md
```

## 허용 파일

```text
examples/release-contracts/**
docs/implementation/**
docs/operations/**
scripts/ci/check_schema_examples.py
PLANS.md
README.md
```

## 금지 파일

```text
GitHub workflow
schema field 변경
Cargo 외 package manager
새 external dependency
PR merge
main branch update
ready status 생성
release/deploy/publish automation
signing automation
package registry 설정
external account/repository settings 변경
destructive recovery action
tmp file 삭제
event log trim
artifact 교체
provider live call
HTTP server 구현
browser UI app 구현
```

## 입력

```text
gh pr list --state open --limit 100 --json number,title,baseRefName,headRefName,isDraft,mergeStateStatus,url
docs/implementation/audit/stacked-pr-readiness.md
examples/release-contracts/stacked-pr-readiness.example.json
```

## 출력

```text
schema-valid ReleaseReadiness example
human-readable stacked PR readiness evidence document
schema example validation case
status = reserved
```

## 핵심 TASK

```text
stacked PR readiness example 추가
stacked PR readiness evidence 문서 추가
schema example check에 새 ReleaseReadiness example 연결
required stacked PR readiness checks sanity check 추가
reserved status/no-main-merge regression 문서화
```

## 완료 기준

- `examples/release-contracts/stacked-pr-readiness.example.json`이 `release-readiness.schema.json`을 만족해야 한다.
- example은 contiguous stack, clean merge state, draft review gate, main merge not performed, final audit evidence link check를 포함해야 한다.
- example status는 `reserved`여야 하고, review/merge coordination reserved blocker를 포함해야 한다.
- `docs/implementation/audit/stacked-pr-readiness.md`는 checked PR range, stack table, clean/draft state, main merge not performed, reserved blockers를 설명해야 한다.
- schema field, workflow, dependency, CLI/API/UI surface, PR merge, main update, signing, publish, deploy, destructive recovery action은 변경하지 않는다.

## 검증

```text
python scripts/ci/run_all.py
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 다음 handoff

M9s는 explicit approval을 받은 뒤 stacked PR ready/merge coordination으로 이어가거나, 별도 승인된 recovery/release action surface로 이어간다. destructive recovery, signing, publish, deploy automation은 별도 승인 전까지 RESERVED다.
