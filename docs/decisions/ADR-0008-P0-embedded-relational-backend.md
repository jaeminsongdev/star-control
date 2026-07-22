# ADR-0008: P0 embedded relational backend

## 상태

채택 — 2026-07-12, Windows x64·ARM64 release cross-build 통과. `v0.1.0`의 native ARM64 limitation과 Preview 정책은 ADR-0015가 소유

## 맥락

[ADR-0006](ADR-0006-공통-개발-관리와-로컬-관리-DB-경계.md)과 [ADR-0007](ADR-0007-P0-하이브리드-저장소와-운영-정책.md)은 backend-neutral repository와 global/project 하이브리드 store를 고정했다. 사용자는 P0 구현과 embedded relational 방향의 dependency 조사·추가를 승인했다.

backend는 Controller 단일 Writer, store-local transaction, consistent backup, integrity 검사, read-only recovery, side-by-side migration과 Windows x64·ARM64 배포를 지원해야 한다. v1은 DB 전체 암호화를 요구하지 않으며 public contract에 backend 이름을 노출하지 않는다.

## 결정

`star-state` private adapter는 `rusqlite 0.40.1`과 bundled SQLite를 사용한다.

```toml
rusqlite = { version = "0.40.1", default-features = false, features = ["backup", "bundled", "limits"] }
```

- `bundled`로 제품이 SQLite build를 통제하고 Windows 대상 PC의 별도 SQLite 설치·version에 의존하지 않는다.
- `backup`의 online backup API로 일관된 store backup을 만든다. 열린 transaction 중 DB 파일만 복사하지 않는다.
- `limits`로 SQL statement·value·column 등 adapter 입력 상한을 connection에 적용한다.
- default feature는 사용하지 않아 P0에 필요 없는 cache와 다른 FFI backend를 암묵적으로 넣지 않는다.
- loadable extension, user-defined function, virtual table, SQLCipher와 dynamic system SQLite를 활성화하지 않는다.
- connection, SQL, pragma, error code는 `star-state` 밖으로 반환하지 않고 stable repository result·error로 변환한다.

SQLite transaction은 한 SQLite database 안의 atomicity에만 사용한다. global DB와 project DB 사이에는 분산 transaction이 없으며 `CoordinatedOperation`·participant receipt·active-set manifest가 partial commit을 복구한다.

## 근거와 검증 gate

- [rusqlite repository](https://github.com/rusqlite/rusqlite)는 bundled feature를 Windows처럼 linking이 복잡한 환경의 선택지로 설명하고 MIT license, bundled SQLite의 public-domain 조건을 명시한다.
- [rusqlite backup API](https://docs.rs/rusqlite/0.40.1/rusqlite/backup/index.html)는 별도 source·destination connection을 사용하는 online backup을 제공한다.
- [SQLite transaction 문서](https://www.sqlite.org/transactional.html)는 한 transaction의 ACID·crash atomicity 범위를 설명한다.
- [SQLite corruption 문서](https://www.sqlite.org/howtocorrupt.html)는 transaction 중 raw file copy와 journal/WAL 오분리를 피하고 backup API 또는 quiesced copy를 사용해야 함을 설명한다.

채택 뒤 다음 항목을 release gate로 확인한다.

1. lockfile에 실제 선택된 `rusqlite`, `libsqlite3-sys`와 bundled SQLite version을 기록한다.
2. clean x64 build·test와 `aarch64-pc-windows-msvc` cross-build를 통과한다.
3. dependency license·advisory와 build script 입력을 검사한다.
4. transaction crash, online backup restore, `quick_check`·`integrity_check`, future version read-only와 double-writer fixture를 통과한다.
5. DB·WAL·journal·backup·recovery 파일의 current-user DACL을 검사한다.

ARM64 C toolchain이 bundled SQLite를 cross-compile하지 못하면 이 ADR을 완료로 포장하지 않는다. 필요한 toolchain 설치는 별도 승인을 받고, dependency를 public contract로 올리는 fallback은 사용하지 않는다.

### 2026-07-12 검증 상태

- lockfile에 `rusqlite 0.40.1`, `libsqlite3-sys 0.38.1`과 bundled build dependency가 고정됐다.
- `cargo build --workspace --release --locked`와 `cargo build --workspace --target aarch64-pc-windows-msvc --release --locked`가 통과해 bundled SQLite의 x64·ARM64 cross-compile을 확인했다.
- `cargo deny check advisories`와 `cargo audit --deny warnings`가 통과했다. `cargo metadata`에서 이번 DB dependency 계열은 MIT 또는 MIT/Apache-2.0으로 확인했다.
- workspace test에서 exclusive writer, project partition, online backup·read-only restore copy, future/corrupt inspection, retention과 redaction fixture가 통과했다.
- 실제 ARM64 Windows에서의 DB open·DACL·backup/restore 실행과 installer packaging은 P9 native gate다. cross-build를 native 실행 증거로 표시하지 않는다.
- 저장소에는 아직 실제 `deny.toml` allow 정책이 없으므로 전체 workspace license 정책 gate는 별도 작업이다. 기본 설정의 `cargo deny check` license 실패를 dependency license 위반으로 오해하지 않는다.

## 선택하지 않은 대안

### 시스템 SQLite dynamic link

대상 Windows의 설치 여부·version과 packaging 환경에 의존하므로 v1 기본값으로 선택하지 않았다.

### SQLx 또는 Diesel

P0는 한 process의 동기 단일 Writer이며 connection pool, async DB runtime 또는 ORM code generation이 필요하지 않다. dependency·compile surface가 커지므로 선택하지 않았다.

### pure-Rust key-value/embedded store

관계·foreign key·transaction·integrity query 요구와 맞지 않고 repository 설계를 backend 제약에 맞춰 바꾸게 되므로 선택하지 않았다.

### SQLCipher 또는 전체 DB 암호화

사용자 선택 6A는 v1에서 current-user ACL과 persistence 전 redaction을 사용한다. root locator만 Windows current-user protection으로 분리하므로 SQLCipher를 추가하지 않는다.

## 연결 문서

- [공통 개발 관리와 로컬 관리 DB 계약](../contracts/development-management.md)
- [Version과 Migration 계약](../contracts/versioning-and-migrations.md)
- [상태 기록과 이어하기](../architecture/state-and-artifacts.md)
- [Repository·Package 구조](../architecture/repository-layout.md)
- [최종 구현 로드맵](../roadmap/final-implementation.md)
