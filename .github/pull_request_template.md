## 목표

- 계획 ID:
- 한 문장 요약:

## 변경 범위

- [ ] 계약·Schema
- [ ] Gateway·IPC·Registry
- [ ] Windows process·격리
- [ ] CLI·운영
- [ ] 문서만

## 검증

```text
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo run --locked -p star-schema-gen -- --check
cargo run --locked -p star-matrix-check
git diff --check
```

- [ ] `legacy/` 변경 없음
- [ ] 실제 Codex 증거가 필요한 matrix 변경은 정규화 evidence와 연결됨
- [ ] secret 원문·runtime log·`target/` 생성물을 추적하지 않음
- [ ] dependency 또는 workflow 변경은 diff와 lockfile을 검토함

## 위험과 후속 작업

- 남은 위험:
- 사람이 확인할 사항:
- 후속 계획 ID:
