---
name: star-control-workflow
description: Use when a Codex development request should be started, staged, continued, reviewed, or completed through the installed Star-Control MCP tools.
---

# Star-Control workflow

개발 변경 목표는 Star-Control MCP에서 목표를 시작한 뒤 반환된 단계와 상태를 기준으로 진행한다.

1. 새 개발 목표이면 `star_goal_start`를 먼저 사용한다.
2. 질문이 반환되면 사용자의 답을 `star_goal_answer`로 전달한다.
3. 현재 단계와 실행 조건은 `star_plan_get`, `star_status_get`으로 확인한다.
4. 단계 실행과 계속 진행은 Star-Control이 제공한 command와 approval 경계를 따른다.
5. 완료 전에는 검증·증거 상태를 확인하고, 실제로 확인하지 못한 항목을 완료로 표현하지 않는다.

Star-Control MCP가 연결되지 않았거나 시작하지 못하면 우회한 것처럼 행동하지 말고, 연결 실패와 사용자가 실행할 진단 절차를 알린다. 돈이 드는 작업과 외부 상태 변경은 Star-Control이 자동으로 허용한 것처럼 해석하지 않는다.
