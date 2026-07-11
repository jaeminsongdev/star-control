# ADR-0005: MCP 구현 계약 동결

## 상태

채택 — 2026-07-11

## 결정

ADR-0004의 무재시작 구조를 실제 구현 가능한 contract v1로 동결한다.

1. `star-mcp.exe`는 공식 Rust SDK `rmcp 2.2.0` 기반 local STDIO server다.
2. MCP 기준 규격은 `2025-11-25`, 최소 호환은 `2025-06-18`이다.
3. tools/list는 search·describe·status, 여섯 risk lane, Operation 조회·취소와 approval 해소의 고정 12개다.
4. `tools.listChanged`, resources, prompts, logging, completions와 MCP Tasks를 광고하지 않는다.
5. 장기 실행의 유일한 정본은 Controller Operation이며 MCP progress·cancellation은 현재 request 보강에만 사용한다.
6. ToolPackageManifest v1 문법, source·trust·update policy·binding·output·concurrency·cancel 규칙을 exact enum과 기본값으로 고정한다.
7. manifest·package·descriptor·arguments·snapshot·approval scope hash는 RFC 8785 JCS + SHA-256이다.
8. Registry는 direct `ReadDirectoryChangesW` watcher와 request 전 demand scan을 함께 사용한다.
9. executable은 final handle identity를 검증하고 suspended process 생성·Job Object 할당까지 lease한다.
10. isolation은 의미가 분명한 `trusted_desktop`과 `appcontainer_adapter`만 지원한다. `restricted_token` profile은 제외한다.
11. Windows 지원 기준은 Windows 11 24H2 build 26100 이상, x64·ARM64다.
12. 완료는 실제 Codex same-session 무재시작 E2E를 포함한 MCP 검증 행렬 전체 통과로 판정한다.
13. 외부 process는 최소 environment·allowlist만 받고, project·Goal 설정은 user location·trust·IPC 안전값을 완화할 수 없다.
14. `retryable`은 오류 표시일 뿐이며 v1 Controller는 외부 EXE를 자동 재실행하지 않는다.

## 이유

- 개념 문서만으로 Terra가 Schema·hash·race·Windows API를 임의 선택하면 마지막 검토가 재설계가 된다.
- Codex 공개 문서는 STDIO와 server instructions를 지원하지만 MCP Tasks 지원을 명시하지 않으므로 필수 경로로 둘 수 없다.
- 고정 MCP 목록은 client의 실행 중 tools/list 갱신 동작과 무관하게 새 EXE를 사용할 수 있다.
- file watcher는 overflow·event 유실이 가능하므로 demand scan 없는 hot reload는 완전하지 않다.
- Job Object와 restricted token을 보안 sandbox라고 부르면 실제 filesystem·network 보장을 과장한다.
- hash 정본이 없으면 describe와 invoke 사이 계약 변경을 일관되게 검출할 수 없다.

## 결과

- Terra는 문서에 정의된 type·state machine·Win32 call order와 fixture를 그대로 구현한다.
- 새 EXE·path·CLI contract 추가에는 MCP binary 변경이 없다.
- fixed MCP surface, process protocol, manifest major, isolation enforcement와 hash 알고리즘 변경은 새 ADR이 필요하다.
- dependency minor update도 conformance·golden·same-session E2E를 통과한 뒤 적용한다.
- 현재 P1은 설계 단계가 아니라 contract type·Schema·fixture와 수직 runtime 구현부터 시작할 수 있다.

## 대체·보완 관계

- ADR-0003은 ADR-0004가 이미 대체했다.
- ADR-0004의 구조 선택은 유지한다.
- ADR-0005는 ADR-0004에서 열어 둔 구현 세부를 동결한다.

## 정본

- [MCP 구현 동결 계약](../contracts/mcp-implementation-contract.md)
- [ToolPackageManifest Reference](../contracts/tool-package-manifest-reference.md)
- [Windows Tool Runtime](../architecture/windows-tool-runtime.md)
- [MCP 검증 행렬](../testing/mcp-verification-matrix.md)

## 공식 근거

- [Codex MCP](https://learn.chatgpt.com/docs/extend/mcp)
- [MCP 2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25)
- [공식 Rust SDK](https://github.com/modelcontextprotocol/rust-sdk)
- [Microsoft Win32 API](https://learn.microsoft.com/windows/win32/api/)
- [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785.html)
