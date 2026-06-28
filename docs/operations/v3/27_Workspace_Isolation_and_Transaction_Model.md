> 흡수 출처: `star-control_design_v3/docs/27_Workspace_Isolation_and_Transaction_Model.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 27. Workspace Isolation과 Transaction Model

## 목적

AI worker가 코드를 직접 수정하면 원본 작업트리가 오염될 수 있다. Star-Control은 worker 실행을 가능하면 격리하고, 변경을 patch/diff 단위로 검토한 뒤 반영해야 한다.

## 격리 단계

| 단계 | 방식 | 장점 | 단점 | 사용 시점 |
|---|---|---|---|---|
| Level 0 | 같은 workspace 순차 실행 | 단순 | 오염 위험 | 초기 MVP |
| Level 1 | git status snapshot | 저비용 | 완전 격리 아님 | MVP 기본 |
| Level 2 | git worktree | 충돌 감소 | 관리 필요 | 병렬/중대형 작업 |
| Level 3 | temp copy | Git 미사용 가능 | 비용 큼 | 비 Git 프로젝트 |
| Level 4 | container/VM | 안전 | 비용/복잡도 큼 | 위험 작업 |

## Transaction 단위

```text
begin transaction
  ↓
snapshot baseline
  ↓
worker run
  ↓
collect changed files
  ↓
validation
  ↓
review
  ↓
commit candidate patch
  ↓
사용자 승인 후 apply
```

## 변경 반영 전략

### MVP

동일 workspace에서 순차 실행.

필수 안전장치:

- 실행 전 `git status --short` 저장.
- 실행 후 changed files 수집.
- WorkSpec allowed_scope와 비교.
- 범위 밖 변경이면 BLOCK.

### 안정화 버전

`git worktree` 사용.

```text
project/
  .worktrees/
    J-0001-impl/
    J-0001-review/
```

worker는 worktree에서만 수정한다. 최종 diff를 원본 branch에 apply하기 전 사용자 확인을 받는다.

## Rollback 정책

| 상태 | rollback 방식 |
|---|---|
| 같은 workspace | patch reverse 또는 사용자 확인 |
| worktree | worktree 삭제 |
| temp copy | temp folder 삭제 |
| container | container discard |

## WorkSpec 격리 필드

```yaml
workspace:
  isolation_level: worktree
  base_ref: main
  worktree_path: .worktrees/J-0001-impl
  apply_mode: patch_after_review
```

## 범위 위반 검사

```yaml
allowed_scope:
  - src/**
  - tests/**
forbidden_scope:
  - .env
  - secrets/**
  - .git/**
  - package-lock.json
```

`changed_files`가 allowed scope 밖이면 `WORKSPEC_VIOLATION`으로 처리한다.

## 구현 체크리스트

- [ ] baseline git status 저장.
- [ ] changed files 수집.
- [ ] scope guard 실행.
- [ ] worktree 생성/삭제 API 설계.
- [ ] patch artifact 저장.
- [ ] apply 전 사용자 승인 gate.
