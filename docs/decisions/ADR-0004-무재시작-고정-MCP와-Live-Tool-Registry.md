# ADR-0004: 무재시작 고정 MCP와 Live Tool Registry

## 상태

채택 — 2026-07-11

구현 세부는 [ADR-0005](ADR-0005-MCP-구현-계약-동결.md)가 동결한다.

ADR-0003의 “manifest action마다 MCP tool을 동적으로 공개하고 process lifetime에 snapshot을 고정”한 부분을 대체한다. EXE별 code가 없는 Gateway와 Controller 실행 경계는 유지한다.

## 문제

사용자는 새 개발 도구 EXE를 추가하거나 TOML의 path·명령 형식을 바꾸거나 같은 path의 EXE를 교체한 뒤 다음을 하지 않고 즉시 쓰기를 원한다.

- `star-mcp.exe` rebuild
- MCP server 재등록
- MCP process 재시작
- Codex 재시작

일반 MCP server는 tool별 이름·설명·input Schema·annotation을 `tools/list`에 공개한다. 이 목록을 EXE마다 동적으로 만들면 이미 연결된 client가 변경 알림을 실제로 다시 조회하고 새 도구를 사용할 수 있는지에 제품 동작이 의존한다. 반대로 범용 invoke 하나만 두면 실제 도구별 Schema와 위험 annotation이 보이지 않는다.

## 결정

### 1. MCP surface와 실제 Tool Registry를 분리한다

`star-mcp.exe`는 다음 고정 surface만 제공한다.

- search, describe, Registry status
- read/write/destructive와 closed/open-world를 조합한 여섯 risk lane call
- Operation 조회·취소
- 사용자 승인 해소

Star-Control core action과 외부 EXE action은 모두 Controller의 Tool Registry 항목이다. 새 항목이 생겨도 MCP tool 이름은 늘어나지 않는다.

### 2. Controller만 live Registry를 소유한다

`star-mcp.exe`는 TOML과 EXE를 읽지 않고 typed Local IPC만 사용한다. Controller가 release·user·trusted project source를 병합하고 immutable RegistrySnapshot을 atomically publish한다.

이 구조는 Gateway와 Controller가 서로 다른 snapshot을 보관하는 문제를 없앤다. handshake에 Registry hash를 고정하지 않는다.

### 3. watcher와 demand scan을 함께 사용한다

Windows file watcher가 manifest·외부 Schema·EXE path 변경을 빠르게 알린다. search·describe·invoke 직전 demand scan이 알림 유실, timestamp 정밀도와 같은 path 교체를 보완한다. 별도 주기 scheduler는 만들지 않는다.

reload는 package 단위로 다음 순서를 지킨다.

1. debounce와 stable-file 확인
2. TOML·Schema·reference·trust·EXE probe 검증
3. candidate package 전체 성공
4. 새 immutable snapshot atomic publish

candidate가 잘못됐으면 해당 package의 last-known-good를 유지하고 진단한다. 다른 정상 package의 update는 계속 반영한다.

### 4. describe hash로 발견과 실행 사이 변경을 막는다

Codex는 search 뒤 describe에서 input·output Schema, side effect, 비용·권한, 정확한 risk lane과 `descriptor_hash`를 받는다. invoke는 이 hash와 typed arguments를 반드시 보낸다.

실행 직전 descriptor가 바뀌었으면 `TOOL_DESCRIPTOR_STALE`, 다른 lane으로 호출했으면 `TOOL_LANE_MISMATCH`로 side effect 전에 거부한다. Codex는 다시 describe한다.

실행이 시작되면 그 call은 lease한 descriptor와 resolved EXE identity를 끝까지 사용한다. 새 call만 최신 snapshot을 사용한다.

### 5. EXE 교체 정책을 명시한다

- `pinned_hash`: 지정 SHA-256과 다르면 실행하지 않는다. project source의 유일한 정책이다.
- `version_compatible`: path의 새 EXE를 probe하고 선언 interface·version 범위가 맞을 때 자동 반영한다.
- `follow_path`: user source에서만 허용한다. path의 현재 유효 EXE를 즉시 사용하되 매번 identity를 확인하고 hash·version 변경을 기록한다.

PATH의 첫 executable을 암묵적으로 선택하지 않는다. path canonicalization, file identity, architecture, protocol, license/readiness와 trust를 실행 전에 검증한다.

### 6. CLI 차이는 data로 표현한다

단순 CLI는 `argv_v1` manifest에 subcommand, positional·flag binding, stdin, cwd, env, timeout, exit code와 stdout/stderr parser를 선언한다. 구조화 결과·progress·취소가 복잡하거나 interactive한 도구는 별도 adapter EXE가 `star_json_stdio_v1`을 구현한다.

Gateway·Controller에 EXE별 parser, raw shell string과 bespoke handler를 추가하지 않는다.

### 7. annotation과 authorization을 분리한다

실제 action의 annotation을 동적 MCP tool로 보여줄 수 없으므로 여섯 고정 risk lane이 보수적인 MCP annotation을 제공한다. 실제 permission, trust, 비용, scope와 승인은 Controller가 descriptor 기준으로 다시 판단한다. annotation은 authorization 근거가 아니다.

### 8. tool 목록 변경 알림에 의존하지 않는다

MCP 규격은 `tools.listChanged` capability와 `notifications/tools/list_changed`를 정의한다. 그러나 고정 surface에서는 목록 자체가 바뀌지 않으므로 이를 광고하거나 Codex의 live refresh 동작에 의존하지 않는다.

## 결과

- TOML 저장, path 수정과 유효한 EXE 교체는 같은 Codex 작업의 다음 search·describe·invoke부터 반영된다.
- 새 EXE 때문에 Gateway를 rebuild·재등록·재시작하지 않는다.
- generic call의 약한 typed discovery는 describe의 실제 JSON Schema와 descriptor hash로 보완한다.
- generic call의 약한 per-tool annotation은 여섯 risk lane으로 보완한다.
- Registry load·trust·watch·search index는 Controller와 `star-config/registry`가 담당하고 Gateway는 `star-ipc`와 계약에만 의존한다.
- 실행 증거에는 Registry revision, descriptor hash, resolved executable identity와 arguments hash를 남긴다.
- 새 process protocol, 새 risk lane, 새로운 permission 의미나 보안 수정은 binary·계약 변경이 필요할 수 있다.

## 채택하지 않은 방식

### action마다 동적 MCP tool 공개

Codex가 연결 중 변경된 목록을 항상 재탐색한다는 전제가 무재시작 요구를 약하게 만들므로 제외한다.

### Gateway가 TOML을 직접 읽음

Controller와 snapshot이 어긋나고 두 곳에서 trust·reload logic을 유지해야 하므로 제외한다.

### raw `invoke(exe, argv)` 하나만 제공

입력 검증, permission 분류, 안정적인 discovery와 shell injection 방지가 사라지므로 제외한다.

### 잘못된 candidate에서 전체 Registry 중단

한 사용자의 편집 중 TOML이 모든 개발 도구를 막게 되므로 package last-known-good를 사용한다. required core package가 유효하지 않은 경우에만 core workflow를 닫힌 상태로 중단한다.

## 연결 문서

- [MCP 구현 계약 동결 결정](ADR-0005-MCP-구현-계약-동결.md)
- [무재시작 외부 Tool Registry](../contracts/external-tool-registry.md)
- [고정 MCP 도구 계약](../contracts/mcp-tools.md)
- [Windows Local IPC 계약](../contracts/local-ipc.md)
- [설정과 Catalog 계약](../contracts/config-and-catalog.md)
- [Repository 구조](../architecture/repository-layout.md)
- [최종 구현 로드맵](../roadmap/final-implementation.md)

## 공식 근거

- [MCP Tools 규격](https://modelcontextprotocol.io/specification/2025-11-25/server/tools)
- [MCP Schema Reference](https://modelcontextprotocol.io/specification/2025-11-25/schema)
- [Codex MCP 문서](https://learn.chatgpt.com/docs/extend/mcp)
