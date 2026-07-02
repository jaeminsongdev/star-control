# Stacked PR Merge Procedure

## scope

이 문서는 M0~M9 stacked PR chain을 사람이 검토하고 승인한 뒤 병합할 때 사용할 절차다. 이 문서는 절차만 고정하며 PR ready 전환, PR merge, main update, release/deploy/publish, repository settings 변경, destructive recovery action을 실행하지 않는다.

Authoritative readiness evidence:

```text
docs/implementation/audit/final-completion-audit.md
docs/implementation/audit/stacked-pr-readiness.md
examples/release-contracts/complete-implementation-readiness.example.json
examples/release-contracts/stacked-pr-readiness.example.json
```

## required approval

아래 작업은 모두 명시적 승인 전까지 금지한다.

```text
mark draft PRs ready for review
merge any stacked PR
update main
delete remote/local branches
change repository settings
release/deploy/publish
perform destructive recovery actions
```

승인 문구는 범위와 방식을 포함해야 한다.

```text
승인: Star-Control stacked PR #33부터 최신 top PR까지 ready 전환 후 top-down merge 절차로 main까지 병합해줘. release/deploy/publish와 destructive recovery는 하지 마.
```

다른 방식이 필요하면 승인 문구가 merge method, 대상 PR range, release/deploy/publish 금지 여부를 명시해야 한다.

## pre-merge verification

승인 후 실제 ready/merge 전에 아래를 다시 확인한다.

```text
git status --short --branch
gh pr list --state open --limit 100 --json number,title,baseRefName,headRefName,isDraft,mergeStateStatus,url --jq "sort_by(.number) | map(select(.number >= 33)) | .[] | [.number, .baseRefName, .headRefName, .mergeStateStatus, .isDraft] | @tsv"
python scripts/ci/run_all.py
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

Expected preconditions:

- local worktree is clean
- PR stack is contiguous from `main -> work/e01-schema -> ... -> latest top branch`
- every PR in the approved range reports `mergeStateStatus=CLEAN`
- every PR in the approved range is still draft before the explicit ready step
- latest top branch CI completed with success
- no release/deploy/publish, repository settings change, or destructive recovery approval is implied

## review order

Human review should proceed bottom-up by dependency order:

```text
#33, #34, #35, ... latest top PR
```

Reason: lower-numbered PRs are base layers for later slices. Reviewing bottom-up keeps schema, StateStore, provider, execution, CLI, Sentinel, daemon/API/UI, M9 hardening, and final evidence changes in dependency order.

## merge execution order

Actual branch-to-branch merge should proceed top-down:

```text
latest top PR, ..., #35, #34, #33
```

Reason: each PR head is based on the previous PR head. Merging the top PR into its base first rolls the newest slice into the next lower branch. Repeating downward eventually rolls the whole stack into `work/e01-schema`, and merging #33 then updates `main`.

Recommended merge method:

```text
merge commit
```

Reason: merge commits preserve stacked branch ancestry and reduce duplicate replay risk in branch-to-branch stacked PRs. If repository policy requires squash or rebase merging, stop and confirm the alternate method before merging.

## step sequence

1. Re-run pre-merge verification.
2. Mark the approved PR range ready for review.
3. Confirm GitHub still reports all PRs in the approved range as clean.
4. Review bottom-up from #33 to the latest top PR.
5. Merge top-down from latest top PR down to #33 using the approved merge method.
6. After each merge, re-check the next lower PR's `mergeStateStatus`.
7. Stop before #33 if any lower PR becomes non-clean or CI fails.
8. Merge #33 into `main` only after the full stack has rolled into `work/e01-schema` and final checks remain clean.
9. After `main` updates, run or wait for main CI and record the result.
10. Do not delete branches unless branch cleanup is explicitly approved.

## stop conditions

Stop immediately if any condition appears:

- `mergeStateStatus` is not `CLEAN`
- a PR base/head chain is discontinuous
- a PR unexpectedly stops being draft before the ready step
- CI is failed, cancelled, or still in progress when a merge decision is needed
- local worktree is dirty
- GitHub reports branch protection, required review, or status check requirements that are not satisfied
- merge method differs from the approved method
- requested action includes release/deploy/publish, repository settings mutation, branch deletion, or destructive recovery without explicit approval

## non-goals

This procedure does not authorize:

- release/deploy/publish
- package registry publishing
- external repository settings changes
- branch deletion
- destructive recovery actions
- provider live calls
- credential or external account changes

## next handoff

The next action is an explicit user decision: approve the ready/merge procedure, choose a different merge method, or keep the stack as draft for review.
