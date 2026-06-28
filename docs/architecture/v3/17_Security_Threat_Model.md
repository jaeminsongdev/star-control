> 흡수 출처: `star-control_design_v3/docs/17_Security_Threat_Model.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 17. 보안 Threat Model

## 1. 목적

Star-Control은 AI가 파일을 읽고, 수정하고, 명령을 실행하게 만든다. 따라서 위협 모델을 먼저 정의해야 한다.

보안 목표:

1. 사용자 승인 전 복구 불가능한 변경 방지
2. 비밀정보 유출 방지
3. 원격 저장소/외부 서비스 영향 통제
4. AI의 범위 이탈 방지
5. 테스트 약화/검증 우회 방지
6. Provider별 권한 차이를 공통 정책으로 흡수

---

## 2. 주요 자산

```text
소스코드
비밀정보
Git history
로컬 개발환경
원격 저장소
패키지/배포 권한
사용자 계정/API token
작업 산출물
```

---

## 3. 위협 목록

| ID | 위협 | 예시 | 대응 |
|---|---|---|---|
| T-001 | 대량 삭제 | `rm -rf`, `git clean` | command-policy forbidden |
| T-002 | 작업트리 초기화 | `git reset --hard` | forbidden |
| T-003 | 원격 반영 | `git push`, release publish | approval required |
| T-004 | 비밀정보 유출 | token 출력, curl 업로드 | secret guard + network approval |
| T-005 | 의존성 오염 | npm/pip/cargo add | approval required |
| T-006 | 테스트 약화 | skip/delete tests | forbidden |
| T-007 | scope creep | 요청 밖 파일 수정 | scope policy |
| T-008 | prompt injection | repo 문서가 AI를 속임 | instruction hierarchy + prompt guard |
| T-009 | provider compromise | 외부 provider 로그/데이터 | provider permission policy |
| T-010 | infinite loop/cost burn | 반복 retry | budget policy |

---

## 4. 권한 레벨

```text
READ_ONLY
DRAFT_ONLY
WORKSPACE_WRITE
VALIDATION_RUNNER
REVIEW_ONLY
APPROVAL_REQUIRED
FORBIDDEN
```

역할별 기본 권한:

| Role | 권한 |
|---|---|
| router-low | READ_ONLY |
| worker-local-draft | DRAFT_ONLY |
| worker-impl | WORKSPACE_WRITE |
| worker-review | REVIEW_ONLY |
| worker-security | REVIEW_ONLY |
| release worker | APPROVAL_REQUIRED |

---

## 5. Secret Guard

검사 대상:

```text
.env
*.pem
*.key
API_KEY
TOKEN
PASSWORD
SECRET
OPENAI_API_KEY
GITHUB_TOKEN
```

정책:

- secret 원문 출력 금지
- report에는 위치와 위험만 요약
- 외부 provider에 secret 포함 파일을 보내기 전 approval 필요
- local-only provider로 처리할 수 있으면 local 우선

---

## 6. Scope Guard

`WorkSpec.allowed_scope` 밖 파일 변경을 막는다.

처리:

```text
변경 파일 수집
→ allowed_scope와 비교
→ 벗어나면 POLICY_VIOLATION
→ report 생성
→ worker 중단 또는 rollback 대기
```

---

## 7. Command Policy

공통 원본:

```text
policies/command-policy.yaml
```

Provider별 산출물:

```text
Codex .rules
Claude permissions/hooks
Gemini safety policy/hooks
Cursor rules
Star-Control internal gate
```

---

## 8. Approval Queue

승인 요청은 표준 파일로 남긴다.

```json
{
  "approval_id": "APP-0001",
  "job_id": "J-0001",
  "reason": "새 의존성 추가 필요",
  "requested_action": "npm install lodash",
  "risk": "MEDIUM",
  "alternatives": ["기존 의존성 사용", "직접 구현"],
  "status": "PENDING"
}
```

---

## 9. Checkpoint / Rollback

초기 MVP:

- 작업 전 `git status` 저장
- 변경 파일 목록 저장
- patch/diff 저장

고급:

- git worktree 격리
- branch별 작업
- checkpoint rollback
- PR 기반 반영

---

## 10. Provider별 보안 차이

Provider마다 sandbox/permission model이 다르므로 Star-Control은 provider native 보안에 의존하지 말고, 자체 gate를 먼저 적용해야 한다.

```text
Star-Control policy gate
  ↓
Provider native permission/sandbox
  ↓
worker execution
```

---

## 11. Acceptance Criteria

- 위험 명령이 policy test로 차단된다.
- secret scanner가 report에서 원문을 제거한다.
- scope 밖 파일 변경이 감지된다.
- approval queue가 생성된다.
- 테스트 약화 시도가 BLOCK된다.
- budget 초과가 자동 중단된다.
