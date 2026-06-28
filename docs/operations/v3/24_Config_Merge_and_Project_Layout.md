> 흡수 출처: `star-control_design_v3/docs/24_Config_Merge_and_Project_Layout.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 24. Config Merge와 프로젝트 배치 규칙

## 목적

Star-Control은 Codex 전용 설정 모음이 아니라 여러 Provider를 같은 규격으로 제어하는 운영체계다. 그래서 설정 파일의 위치와 병합 순서를 명확히 해야 한다. 이 문서는 전역 설정, 사용자 설정, 프로젝트 설정, 실행별 설정, Provider 산출물의 우선순위를 정의한다.

## 핵심 원칙

1. **원본 설정은 Star-Control에 둔다.**
   - `D:\개발\Star-Control` 또는 사용자가 정한 Star-Control home.
   - Provider별 설정 파일은 원본이 아니라 렌더링 산출물이다.
2. **프로젝트별 상태는 프로젝트 안에 둔다.**
   - `PLANS.md`, `.star-control/`, `.ai-runs/`는 프로젝트별로 독립한다.
3. **Provider별 native 설정은 Adapter가 만든다.**
   - Codex `~/.codex/*.config.toml`, `.agents/skills`, `.codex/rules`는 CodexAdapter 산출물이다.
4. **상위 설정보다 가까운 설정이 우선한다.**
   - 단, 보안 정책은 더 제한적인 쪽이 이긴다.

## 권장 디렉터리

```text
D:\개발\Star-Control\
  router.yaml
  capabilities\
  policies\
  providers\
  roles\
  schemas\
  renderers\
  templates\
  runs\

D:\개발\프로젝트A\
  AGENTS.md
  PLANS.md
  .star-control\
    project.yaml
    context.yaml
    approvals\
  .ai-runs\
```

Windows 전역 Provider 산출물:

```text
%USERPROFILE%\.codex\
  config.toml
  low-router.config.toml
  worker-impl.config.toml
  worker-review.config.toml
  rules\command_block.rules

%USERPROFILE%\.agents\skills\
  code-review\SKILL.md
  validation\SKILL.md
  plan-ledger\SKILL.md
```

## 설정 계층

| 우선순위 | 계층 | 예시 | 설명 |
|---:|---|---|---|
| 1 | Built-in defaults | 코드 내부 기본값 | 없으면 동작 불가한 최소값 |
| 2 | Star-Control global | `Star-Control/router.yaml` | 모든 프로젝트 공통 |
| 3 | User override | `%USERPROFILE%/.star-control/user.yaml` | 사용자 개인 환경 |
| 4 | Project config | `<project>/.star-control/project.yaml` | 프로젝트별 설정 |
| 5 | Run config | `.ai-runs/J-0001/run-state.json` | 특정 작업 실행 상태 |
| 6 | CLI override | `--provider codex` | 이번 실행만 적용 |

보안 관련 계층은 일반 merge와 다르다. `forbidden`, `deny`, `requires_approval`은 하위 계층이 완화할 수 없고, 더 제한적인 정책이 이긴다.

## 병합 규칙

### Scalar

마지막 계층 값이 이긴다.

```yaml
model_tier: low-cost-cloud
```

### Map

키 단위로 deep merge한다.

```yaml
providers:
  codex:
    enabled: true
```

### List

필드 의미에 따라 다르게 병합한다.

| 리스트 종류 | 병합 방식 |
|---|---|
| `forbidden_actions` | 합집합 |
| `allowed_scope` | 교집합 또는 더 좁은 쪽 |
| `provider_candidates` | 프로젝트 설정이 있으면 우선 |
| `hooks.steps` | 순서 유지 append, 같은 `id`는 override |
| `skills` | 합집합, 같은 이름은 가까운 계층 우선 |

## 프로젝트 설정 예시

```yaml
# .star-control/project.yaml
project_id: project-a
project_name: 프로젝트A

source_roots:
  - src
  - tests

plans_file: PLANS.md
run_dir: .ai-runs

validation:
  preferred_commands:
    quick:
      - cargo test --all
    full:
      - cargo fmt --check
      - cargo clippy --all-targets --all-features -- -D warnings
      - cargo test --all

policies:
  allow_dependency_changes: false
  require_user_approval_for_public_api: true
```

## Provider 산출물 생성 원칙

Star-Control 원본에서 Provider 산출물을 렌더링한다.

```text
policies/command-policy.yaml
  → Codex: ~/.codex/rules/command_block.rules
  → Claude: .claude/settings.json permissions/hooks
  → Cursor: .cursor/rules 또는 adapter policy
```

Provider 산출물은 수동 수정하지 않는 것을 원칙으로 한다. 꼭 수동 수정이 필요하면 `rendered_overrides/`에 별도 기록하고, 원본 정책으로 다시 반영한다.

## 구현 체크리스트

- [ ] 설정 로더가 계층별 파일을 읽는다.
- [ ] 일반 merge와 보안 merge를 분리한다.
- [ ] 최종 effective config를 `runs/J-0001/effective-config.yaml`로 저장한다.
- [ ] Provider 산출물 생성 시 원본 hash를 남긴다.
- [ ] 수동 변경 감지를 위해 generated file header를 넣는다.
