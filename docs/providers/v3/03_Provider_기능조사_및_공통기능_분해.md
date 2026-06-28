> 흡수 출처: `star-control_design_v3/docs/03_Provider_기능조사_및_공통기능_분해.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 03. Provider 기능조사 및 공통 기능 분해

이 문서는 주요 AI 코딩 도구의 기능을 조사해 Star-Control의 공통 capability로 분해한 결과다.

## 1. 조사 기준

- 공식 문서 우선
- 공개 changelog/blog는 보조
- 연구 논문은 설계 경향 검토용
- 기능은 provider 고유명으로 저장하지 않고 Star-Control 공통 capability로 환원

## 2. OpenAI Codex

### 공식적으로 확인한 기능군

- Config / Profiles: `--profile`로 named config layer를 적용한다. 프로필 파일은 `~/.codex/profile-name.config.toml` 형식이고 top-level key를 사용한다. 출처: https://developers.openai.com/codex/config-advanced
- CLI automation: `codex exec`, `resume`, `fork`, `--json`, `--output-last-message`, `--output-schema` 등 자동화 친화 기능. 출처: https://developers.openai.com/codex/cli/reference
- Skills: `SKILL.md` 기반 reusable workflow. 출처: https://developers.openai.com/codex/skills
- Hooks: lifecycle 중 deterministic scripts 삽입. 출처: https://developers.openai.com/codex/hooks
- Rules: Starlark 기반 `.rules`, `allow/prompt/forbidden`, `codex execpolicy check`. 출처: https://developers.openai.com/codex/rules
- Goals: `/goal`, pause/resume/clear, durable objective. 출처: https://developers.openai.com/codex/use-cases/follow-goals
- Subagents: 전문 agent를 spawn해 병렬 작업 후 결과 수집. 출처: https://developers.openai.com/codex/subagents
- MCP: third-party tools/context 연결. 출처: https://developers.openai.com/codex/mcp
- Plugins: skills/app integrations/MCP servers 묶음. 출처: https://developers.openai.com/codex/plugins
- Sandbox/network approval: workspace-write 기본 네트워크 차단, network rules. 출처: https://developers.openai.com/codex/agent-approvals-security

### Star-Control 반영

| Codex 기능 | Star-Control 공통 기능 |
|---|---|
| Profiles | Provider Role Config |
| Skills | Skill Engine + Codex Skill Renderer |
| Hooks | Hook Engine + Codex Hook Renderer |
| Rules | Command Policy Engine + Codex Rules Renderer |
| Goals | Goal Engine native adapter |
| Subagents | Worker Engine native adapter |
| exec/resume/fork | Thread / Run Engine adapter |
| MCP | Tool / MCP Engine |
| Plugins | Extension Package Engine |
| Sandbox/approval | Permission / Sandbox Policy |

## 3. Claude Code

### 공식적으로 확인한 기능군

- Claude Code는 codebase를 읽고, 파일을 편집하고, 명령을 실행하고, development tools와 통합되는 agentic coding tool이다. 출처: https://code.claude.com/docs/en/how-claude-code-works
- Commands는 모델 전환, permissions, context clear, workflow 실행, `/plan`, `/model`, `/effort` 같은 세션 제어를 제공한다. 출처: https://code.claude.com/docs/en/commands
- Skills는 `SKILL.md`로 추가하며 관련 있을 때 자동 사용되거나 `/skill-name`으로 호출된다. Bundled skills로 `/code-review`, `/batch`, `/debug`, `/loop`, `/claude-api`가 있다. 출처: https://code.claude.com/docs/en/skills
- Hooks는 shell/HTTP/LLM prompt handler를 lifecycle event에 붙인다. 출처: https://code.claude.com/docs/en/hooks
- Subagents는 Markdown + YAML frontmatter로 정의하고 tool restriction, permission mode, hooks, skills를 가질 수 있다. 출처: https://code.claude.com/docs/en/sub-agents
- Agent Teams는 여러 독립 Claude Code 세션을 team lead가 조율하고, shared task list와 peer messaging을 사용한다. 출처: https://code.claude.com/docs/en/agent-teams
- Plugins는 skills, agents, hooks, MCP servers, LSP servers, monitors를 묶는 self-contained directory다. 출처: https://code.claude.com/docs/en/plugins-reference
- `.claude`는 instructions/settings/hooks/skills/subagents/memory를 user/project scope에서 읽는다. 출처: https://code.claude.com/docs/en/settings
- MCP는 external tools/databases/APIs 연결 표준으로 사용된다. 출처: https://code.claude.com/docs/en/mcp

### Star-Control 반영

| Claude 기능 | Star-Control 공통 기능 |
|---|---|
| CLAUDE.md | Context Engine |
| Commands | Command Engine |
| Plan Mode | Plan Engine native adapter |
| Skills | Skill Engine + Claude Renderer |
| Hooks | Hook Engine native adapter |
| Subagents | Worker Engine native adapter |
| Agent Teams | Team / Multi-thread Engine reference |
| Plugins | Extension Package Engine |
| Permissions | Permission Policy Renderer |
| Checkpoints | Checkpoint Engine |
| Sessions/resume | Session Store / Resume Engine |

## 4. Gemini CLI

### 공식적으로 확인한 기능군

- Plan Mode는 skills, safety policies, plan storage, hooks로 커스터마이징 가능하다. 출처: https://geminicli.com/docs/cli/plan-mode/
- Extensions는 prompts, MCP servers, custom commands, themes, hooks, sub-agents, agent skills를 하나의 설치 가능한 패키지로 묶는다. 출처: https://github.com/google-gemini/gemini-cli/blob/main/docs/extensions/index.md
- Subagents는 별도 context window, custom instructions, curated toolset을 가진 전문 agent로 main session을 가볍게 유지한다. 출처: https://developers.googleblog.com/subagents-have-arrived-in-gemini-cli/
- Agent Skills는 specialized expertise/procedural workflows/task-specific resources를 제공하는 open standard 기반 디렉터리다. 출처: https://geminicli.com/docs/cli/skills/
- MCP servers는 settings.json의 `mcpServers` 설정으로 연결된다. 출처: https://geminicli.com/docs/tools/mcp-server/
- Gemini CLI는 slash/@/! command 체계를 가진다. 출처: https://geminicli.com/docs/reference/commands/
- 설정은 environment variables, command-line arguments, settings files를 통해 제공된다. 출처: https://geminicli.com/docs/reference/configuration/

### Star-Control 반영

| Gemini 기능 | Star-Control 공통 기능 |
|---|---|
| GEMINI.md | Context Engine |
| Plan Mode | Plan Engine native adapter |
| Extensions | Extension Package Engine reference |
| MCP | Tool / MCP Engine |
| Commands | Command Engine |
| Hooks | Hook Engine |
| Subagents | Worker Engine |
| Skills | Skill Engine |
| Model routing/fallback | Provider Selection / Fallback Policy |

## 5. Cursor

### 공식적으로 확인한 기능군

- Cursor Plan Mode는 codebase를 조사하고 clarifying questions를 묻고 reviewable plan을 만든 뒤 code writing 전에 계획을 제공한다. 출처: https://cursor.com/docs/agent/plan-mode
- Cursor Agent는 autonomous coding tasks, terminal commands, code editing을 수행한다. 출처: https://cursor.com/docs/agent/overview
- Cursor Headless CLI는 automation/CI/CD를 위한 non-interactive 실행이다. 출처: https://cursor.com/docs/cli/headless
- Cursor Search는 Instant Grep, semantic search, Explore subagent를 제공한다. 출처: https://cursor.com/docs/agent/tools/search
- Cursor Terminal은 sandboxing, preserved history, native terminal integration을 제공한다. 출처: https://cursor.com/docs/agent/tools/terminal

### Star-Control 반영

| Cursor 기능 | Star-Control 공통 기능 |
|---|---|
| Plan Mode | Plan Engine |
| Agent Mode | Worker Engine |
| Background/Cloud Agents | Background Run Engine |
| Headless CLI | Provider Adapter |
| Rules | Context / Rule Engine |
| Memories | Memory Engine |
| Semantic Search | Retrieval Engine |
| Explore Subagent | Explore Worker |
| Terminal sandbox | Shell Tool Adapter + Sandbox Policy |

## 6. GitHub Copilot / VS Code

### 공식적으로 확인한 기능군

- Copilot cloud agent는 repo를 research하고 implementation plan을 만들고 branch에 code changes를 만든 뒤 diff review, iteration, PR 생성 흐름을 제공한다. 출처: https://docs.github.com/copilot/concepts/agents/cloud-agent/about-cloud-agent
- Copilot Agent Skills는 instructions/scripts/resources 폴더로, relevant task에서 Copilot이 로드한다. 출처: https://docs.github.com/en/copilot/how-tos/copilot-on-github/customize-copilot/customize-cloud-agent/add-skills
- Custom instructions는 repository-wide와 path-specific으로 제공된다. 출처: https://docs.github.com/copilot/customizing-copilot/adding-custom-instructions-for-github-copilot
- Copilot code review는 repository custom instructions로 review를 커스터마이징할 수 있다. 출처: https://docs.github.com/copilot/using-github-copilot/code-review/using-copilot-code-review
- MCP는 Copilot과 외부 시스템을 연결한다. 출처: https://docs.github.com/en/copilot/concepts/context/mcp
- VS Code Custom Agents는 planning agent에서 implementation/reviewer agent로 handoff하는 guided workflow를 지원한다. 출처: https://code.visualstudio.com/docs/agent-customization/custom-agents
- VS Code Agent Skills는 GitHub Copilot in VS Code, Copilot CLI, Copilot cloud agent에서 동작하는 open standard라고 설명된다. 출처: https://code.visualstudio.com/docs/agent-customization/agent-skills

### Star-Control 반영

| Copilot/VS Code 기능 | Star-Control 공통 기능 |
|---|---|
| copilot-instructions.md | Context Engine |
| Path-specific instructions | Scoped Context Rules |
| Prompt files | Prompt Template Engine |
| Custom Agents | Worker Role Registry |
| Handoffs | Routing / Stage Transition Engine |
| Agent Skills | Skill Package Engine |
| Cloud Agent | Background Run / PR Engine |
| Code Review | Review Engine |
| MCP | Tool Adapter |
| Budgets | Budget / Cost Engine |

## 7. Jules

Jules는 Google의 autonomous coding agent로 GitHub integration, diff review, PR creation, test suite 실행/생성 흐름을 제공한다. 출처: https://jules.google/

### Star-Control 반영

| Jules 기능 | Star-Control 공통 기능 |
|---|---|
| Async task | Background Run Engine |
| GitHub repo import | Git Platform Adapter |
| Branch changes | Branch / Worktree Engine |
| Diff review | Diff Review Engine |
| PR creation | PR Engine |
| Test suite | Validation Engine |

## 8. Devin

Devin은 AI software engineer를 표방하며, release note에는 workspace가 매 세션 시작 때 saved machine state로 reset된다고 설명된다. 출처: https://docs.devin.ai/get-started/devin-intro, https://docs.devin.ai/release-notes/2024

### Star-Control 반영

| Devin 기능 | Star-Control 공통 기능 |
|---|---|
| Autonomous software engineer | Background Worker Adapter |
| Cloud workspace | Workspace Engine |
| Saved machine state | Workspace Snapshot Engine |
| Long-running sessions | Async Run Engine |
| Team workflow integration | Control Plane / Task Queue |

## 9. Local Models

로컬 모델은 provider native 기능이 적으므로 Star-Control이 대부분 emulation한다.

| Local 기능 | Star-Control 공통 기능 |
|---|---|
| Ollama/LM Studio/vLLM 호출 | LocalModelAdapter |
| 초안 생성 | Draft Worker |
| 로그 요약 | Summary Worker |
| 테스트 후보 | Test Candidate Worker |
| 1차 리뷰 | Reviewer-lite |
| 비공개 정보 처리 | Privacy Preprocessor |

## 10. 공통 기능군 전체 목록

아래 기능은 Star-Control에서 core capability로 구현한다.

1. Context Engine
2. Scoped Context Rules
3. Memory Engine
4. Context Compaction Engine
5. Plan Engine
6. Goal Engine
7. Skill / Procedure Engine
8. Command Engine
9. Hook Engine
10. Rule / Permission Engine
11. Worker Engine
12. Team Engine
13. Thread / Run Engine
14. Background Run Engine
15. Tool / MCP Engine
16. Search / Retrieval Engine
17. VCS / Worktree / PR Engine
18. Validation Engine
19. Review / Critic Engine
20. Evidence Engine
21. Extension Package Engine
22. UI / Control Plane Engine
23. Budget / Cost Engine
24. Provider Adapter Engine
25. Renderer Engine
