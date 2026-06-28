> 흡수 출처: `star-control_design_v3/docs/21_Provider_전체기능_매트릭스_확장.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 21. Provider 전체 기능 매트릭스 확장

## 1. 목적

이 문서는 provider별 기능을 빠짐없이 비교하기 위한 확장 매트릭스다.

주의:

- 기능 지원 여부는 시간이 지나며 바뀐다.
- `provider-features/*.features.yaml`이 기계가 읽는 원본이고, 이 문서는 사람이 보는 해설판이다.
- 모든 기능은 `native / partial / emulated / unsupported / unknown` 중 하나로 관리한다.

---

## 2. 기능군별 비교표

| 기능군 | Codex | Claude Code | Gemini CLI | Cursor | GitHub Copilot | Jules | Devin | Local |
|---|---|---|---|---|---|---|---|---|
| Context file | native | native | native | native | native | partial | partial | emulated |
| Plan mode | emulated/native-like | native | native | native | partial | native-like | partial | emulated |
| Goal mode | native | emulated | emulated | emulated | partial | async task | async task | emulated |
| Skills | native | native | native | native/partial | native | unknown | unknown | emulated |
| Hooks | native | native | native | partial | partial | unknown | unknown | emulated |
| Rules/permissions | native | native | partial | native | partial | platform | platform | Star-Control only |
| Subagents/workers | native | native | native | native | custom agents | platform | platform | emulated |
| Thread resume | native | native | partial | partial | platform | async | session | emulated |
| Background agent | cloud/app | partial | partial | native | native | native | native | emulated |
| MCP | native | native | native | native | native | unknown | unknown | possible |
| Checkpoints | partial | native | partial | partial | git-based | PR-based | workspace | Star-Control |
| Worktree/branch | native/app | possible | possible | native/cloud | native/cloud | native | workspace | Star-Control |
| Code review | native/app | skill/subagent | skill | bugbot/review | native | review diff | platform | emulated |
| Extension/plugin | plugins/skills | plugins | extensions | marketplace/skills | skills | unknown | unknown | Star-Control |
| Headless CLI | native | native | native | native | CLI | tools | API/terminal | native script |
| Structured output | native | partial/API | partial | partial | partial | unknown | unknown | Star-Control parser |

---

## 3. Codex 상세

강한 기능:

```text
profiles
skills
hooks
rules
exec/resume/fork
goals
subagents
MCP
output-last-message
output-schema
JSON events
sandbox/approval
cloud/app threads
worktrees
```

Star-Control 반영:

```text
CodexAdapter
CodexRenderer
CodexPolicyRenderer
CodexSkillRenderer
```

---

## 4. Claude Code 상세

강한 기능:

```text
commands
plan mode
permissions
checkpoints
sessions/resume
skills
hooks
subagents
plugins
MCP
memory
agent teams
```

Star-Control 반영:

```text
ClaudeAdapter
PermissionRenderer
HookRenderer
CheckpointEngine 참고
AgentTeamEngine 참고
```

---

## 5. Gemini CLI 상세

강한 기능:

```text
plan mode
extensions
MCP
commands
hooks
subagents
skills
settings
```

Star-Control 반영:

```text
ExtensionPackageEngine 참고
GeminiExtensionRenderer
MCPAdapter
```

---

## 6. Cursor 상세

강한 기능:

```text
agent mode
plan mode
background agents
rules
memories
MCP
skills
headless CLI
semantic search
instant grep
explore agent
bugbot
```

Star-Control 반영:

```text
PlanEngine
BackgroundRunEngine
RetrievalEngine
Bug/CriticEngine
```

---

## 7. GitHub Copilot 상세

강한 기능:

```text
custom instructions
path-specific instructions
prompt files
agent skills
custom agents
handoffs
MCP
coding agent
code review
issue/PR integration
budgets
```

Star-Control 반영:

```text
ContextRenderer
SkillPackageEngine
GitHubAdapter
PRAdapter
ReviewEngine
BudgetEngine
```

---

## 8. Jules 상세

강한 기능:

```text
async coding task
GitHub integration
branch/PR flow
test running
diff review
parallel task model
setup reuse
critic/review direction
```

Star-Control 반영:

```text
AsyncRunEngine
GitHubPRAdapter
CriticEngine
```

---

## 9. Devin 상세

강한 기능:

```text
autonomous engineer workspace
long-running task
saved machine state
session/workspace model
multi-repo work
team workflows
```

Star-Control 반영:

```text
WorkspaceSnapshotEngine
AsyncRunScheduler
StatePersistence
```

---

## 10. Local 모델 상세

강한 기능:

```text
privacy
low cost
fast draft
summary
simple review
offline mode
```

약한 기능:

```text
large architecture
complex repo edits
tool calling stability
long context
final judgment
```

Star-Control 반영:

```text
LocalDraftWorker
LocalSummaryWorker
LocalReviewLiteWorker
```
