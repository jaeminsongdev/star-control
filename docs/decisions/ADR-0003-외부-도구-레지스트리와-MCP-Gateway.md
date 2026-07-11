# ADR-0003: 외부 도구 Registry와 고정형 MCP Gateway

## 상태

대체됨 — 2026-07-11, [ADR-0004](ADR-0004-무재시작-고정-MCP와-Live-Tool-Registry.md)

이 문서는 tool별 MCP 이름을 동적으로 공개하고 MCP process 시작 때 snapshot을 고정하던 이전 결정을 보존한다. 현재 구현 기준으로 사용하지 않는다.

## 결정

`star-mcp.exe`를 실제 개발 도구에 독립적인 고정형 Gateway로 만든다.

- MCP tool 이름·설명·입력 Schema·EXE path·argument binding·출력 형식은 외부 ToolPackageManifest TOML이 선언한다.
- Star-Control 기본 MCP tool도 `star-control-core.toml`에 두고 binary에 tool별 handler를 compile하지 않는다.
- Gateway는 manifest에서 MCP tool 목록을 만들고 모든 call을 하나의 `tool.invoke` Local IPC command로 변환한다.
- 실제 외부 EXE 실행, permission, trust, timeout, 취소, redaction과 evidence는 Controller가 담당한다.
- Gateway와 Controller는 같은 ToolRegistrySnapshot hash를 확인해야 한다.
- custom adapter의 표준 경계는 `star_json_stdio_v1`, 단순 기존 CLI는 제한된 `argv_v1`을 사용한다.
- shell command string과 tool별 bespoke parser는 Gateway·Controller에 넣지 않는다.
- v1은 MCP process 시작 때 tool 목록을 고정하고 TOML 변경 뒤 재시작한다.

## 안정성 약속

새 EXE, 새 tool, path, Schema, timeout과 argument mapping 추가는 binary 변경을 요구하지 않는다. 다음 항목은 여전히 binary update가 필요할 수 있다.

- MCP·Windows 보안 결함 수정
- 새 manifest major version
- 기존 두 process protocol로 표현할 수 없는 transport
- 새 permission·sandbox 의미

따라서 약속은 “영원히 같은 binary”가 아니라 “도구 추가를 위한 binary 변경 없음”이다.

## 이유

- tool별 handler를 compile하면 새 개발 도구마다 release와 회귀 검사가 필요하다.
- MCP process가 EXE를 직접 실행하면 Controller의 승인·상태·취소·증거 경계를 우회한다.
- 한 개의 범용 `invoke(name, args)` MCP tool만 노출하면 Codex가 도구별 설명과 typed Schema를 처음부터 볼 수 없고 per-tool approval hint도 약해진다.
- manifest에서 각 action을 독립 MCP tool로 투영하면 binary는 고정하면서 Codex 사용성과 입력 검증을 유지할 수 있다.
- 복잡한 출력은 adapter EXE에서 표준 JSON으로 바꾸면 Gateway의 parser가 계속 커지지 않는다.

## 결과

- `star-config/registry`가 manifest parse, merge, trust와 deterministic snapshot을 소유한다.
- `star-mcp.exe`는 `star-config`, `star-contracts`, `star-ipc`에만 의존하고 tool별 module을 갖지 않는다.
- user·project manifest는 외부 process backend만 등록할 수 있다.
- 내부 Controller operation은 release의 required·trusted core manifest만 노출할 수 있다.
- executable·manifest·Schema hash가 바뀌면 재신뢰와 MCP 재시작이 필요하다.
- P1에서 Registry 계약과 fixture를 먼저 구현하고 P2의 첫 작업으로 generic Gateway를 구현한다.

## 채택하지 않은 방식

### Tool별 Rust handler

새 EXE마다 Gateway를 수정해야 하므로 제외한다.

### Gateway의 직접 EXE 실행

초기 구현은 간단하지만 이후 Controller를 붙일 때 실행·승인 계약이 바뀌므로 제외한다.

### 하나의 범용 MCP invoke tool만 제공

TOML hot reload는 쉬워지지만 Codex의 tool 발견, typed input과 tool별 annotation이 약해져 기본 경로로 사용하지 않는다.

### 실행 중 무조건 hot reload

현재 공식 Codex 문서가 실행 중 MCP tool-list 변경을 지원 기능으로 명시하지 않으므로 v1에서는 제외한다. capability가 확인되면 backward-compatible 선택 기능으로 검토한다.

## 연결 문서

- [현재 결정: 무재시작 고정 MCP와 live Tool Registry](ADR-0004-무재시작-고정-MCP와-Live-Tool-Registry.md)
- [외부 Tool Registry와 고정형 MCP Gateway](../contracts/external-tool-registry.md)
- [Star-Control MCP 도구 계약](../contracts/mcp-tools.md)
- [설정과 Catalog 계약](../contracts/config-and-catalog.md)
- [Windows Local IPC 계약](../contracts/local-ipc.md)
- [Repository 구조](../architecture/repository-layout.md)
- [최종 구현 로드맵](../roadmap/final-implementation.md)
