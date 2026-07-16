---
name: star-control-operations
description: Use when a Codex development task may use installed Star-Control operations; discover and invoke only ready actions, otherwise continue with native project tools and record the fallback reason.
---

# Star-Control operations

1. MCP 연결이 가능하면 `star_tool_search`로 현재 작업에 맞는 action을 찾는다.
2. 검색 결과의 action readiness가 `ready`인지 확인한다. package나 manifest의 ready 상태만으로 action을 실행 가능하다고 판단하지 않는다.
3. ready action만 `star_tool_describe`로 다시 조회해 현재 Schema, risk lane, `descriptor_hash`와 `required_call_tool`을 확인한다.
4. 반환된 `required_call_tool`에 `tool_id`, `descriptor_hash`, `arguments`를 전달해 실행한다.
5. `TOOL_DESCRIPTOR_STALE`이 반환되면 다시 describe하고 최신 hash와 Schema로 재시도한다.
6. `approval_required`, `question_required` 또는 Operation ID 반환을 작업 완료로 간주하지 않는다.
7. registry 진단이 필요할 때만 `star_tool_registry_status`를 사용하며 이를 action readiness 대신 사용하지 않는다.
8. ready action이 없거나 MCP 연결이 실패하면 일반 Codex 개발 작업을 막지 말고 프로젝트 native 도구를 사용한다. 결과에는 native fallback 사실과 이유를 기록한다.
9. 유료 작업과 외부 상태 변경은 Star-Control이 자동으로 승인한 것으로 해석하지 않는다.
