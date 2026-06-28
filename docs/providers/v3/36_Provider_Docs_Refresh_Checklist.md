> 흡수 출처: `star-control_design_v3/docs/36_Provider_Docs_Refresh_Checklist.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 36. Provider Docs Refresh Checklist

## 목적

클라우드 AI 도구는 기능 변화가 빠르다. Provider Feature Matrix는 한 번 작성하고 끝내면 안 된다. 이 문서는 정기적으로 공식 문서를 확인해 Star-Control의 provider adapter를 갱신하는 절차다.

## 갱신 주기

| 항목 | 주기 |
|---|---:|
| Codex CLI/profile/skills/rules/hooks/subagents/goals | 월 1회 |
| Claude Code commands/hooks/skills/permissions/subagents/plugins | 월 1회 |
| Gemini CLI plan/extensions/subagents/MCP/settings | 월 1회 |
| Cursor CLI/Plan/Background Agents/Rules/MCP/Search | 월 1회 |
| GitHub Copilot Agent Skills/MCP/Coding Agent | 월 1회 |
| Jules/Devin async agent 기능 | 분기 1회 |
| Local provider endpoint 옵션 | 버전 변경 시 |

## 확인할 공식 자료

```text
- OpenAI Codex developers docs
- Claude Code docs
- Gemini CLI docs / GitHub repository docs
- Cursor docs
- GitHub Copilot docs / VS Code docs
- Jules docs
- Devin docs
```

## 갱신 절차

1. provider docs URL 확인.
2. 기능 추가/삭제/이름 변경 확인.
3. `provider-features/*.features.yaml` 수정.
4. `capability-registry.yaml`에 새 기능이 필요한지 판단.
5. provider renderer 영향 확인.
6. adapter conformance test 갱신.
7. CHANGELOG 작성.
8. README의 `last_verified` 날짜 갱신.

## 변경 기록 형식

```yaml
date: 2026-06-28
provider: codex
change_type: feature_update
summary: "exec resume output-schema 지원 여부 확인"
impact:
  - provider adapter
  - report schema
follow_up:
  - conformance test 추가
```

## 기능 판단 기준

| 상황 | 처리 |
|---|---|
| provider가 native 지원 | `native: true` |
| 일부 지원 | `native: partial` + 제한 기록 |
| 공식 문서에 없음 | `native: unknown` |
| deprecated | `deprecated: true` |
| Star-Control에서 대체 가능 | `emulated` 기록 |

## 최종 체크리스트

- [ ] 공식 문서 링크가 최신인가?
- [ ] provider feature file의 `last_verified`가 갱신됐는가?
- [ ] 새 기능이 Capability Registry에 반영됐는가?
- [ ] renderer 산출물에 영향이 있는가?
- [ ] acceptance test가 깨지지 않는가?
