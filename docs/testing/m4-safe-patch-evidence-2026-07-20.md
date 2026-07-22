# P-0045 M4 안전 Patch 첫 수직 Slice 증거

## 구현 범위

- `prepare_trailing_whitespace_patch`는 live source를 쓰지 않고 byte buffer에서 expected-after와 line edit artifact를 만든다.
- artifact hash와 PatchSet fingerprint를 다시 계산해 sealed artifact 하나가 없는 provisional PatchSet은 apply할 수 없다.
- exact approval fingerprint, Project-relative canonical non-symlink path, before hash와 pre-write 재관찰이 모두 일치해야만 같은 디렉터리 atomic replace를 수행한다.
- 여러 파일 중간 실패는 이미 적용한 경로를 reverse order로 복구하고, 복구를 증명하지 못하면 `partially_applied`를 보존한다.
- explicit rollback은 current byte가 expected-after와 정확히 같을 때만 original bytes를 복원한다.

## 실제 fixture

- target source의 trailing whitespace만 제거하고 unrelated dirty file bytes는 그대로 유지한다.
- apply 뒤 rollback이 exact original bytes를 복원한다.
- artifact가 없는 provisional PatchSet을 `PATCH_ARTIFACT_INVALID`로 거부한다.
- prepare 뒤 target이 바뀌면 `PATCH_TARGET_DIRTY_OR_STALE`이고 사용자 변경을 덮어쓰지 않는다.

workspace Gate의 immutable report path는 `PLANS.md`의 P-0045 evidence 항목이 소유한다.
