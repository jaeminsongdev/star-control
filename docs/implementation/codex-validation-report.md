# Codex Validation Report

## 목적

이 문서는 Codex가 TASK, PR, EPIC 완료 후 검증 결과를 보고할 때 사용할 표준 형식이다. 보고는 짧고 재현 가능해야 하며, 실패를 숨기지 않아야 한다.

## TASK 완료 보고

```markdown
## TASK 완료 보고

TASK: `E00-T00`
Branch:
PR:

### 완료한 내용

- 
- 

### 변경 파일

- 

### 검증 명령

```text

```

### 검증 결과

```text

```

### 남은 위험

- 

### 다음 작업

- 
```

## PR 완료 보고

```markdown
## PR 완료 보고

PR:
Title:
Branch:
Merge commit:

### 변경 파일

- 

### GitHub Actions 결과

- repository-policy-check:
- data-format-check:
- manifest-contract-check:
- naming-policy-check:
- schema-example-check:
- implementation-documentation-check:
- package-specific tests:

### 직접 실행한 검증

```text

```

### 계약 준수 확인

- [ ] runtime dependency 변경 없음 또는 승인됨
- [ ] package manager 변경 없음 또는 승인됨
- [ ] workflow 변경 없음 또는 승인됨
- [ ] tests/CI/policy 약화 없음
- [ ] artifact layout 준수
- [ ] provider-neutral naming 준수
- [ ] secret raw value 출력/저장 없음

### 남은 TODO

- 
```

## EPIC 완료 보고

```markdown
## EPIC 완료 보고

EPIC:
완료 기간:
완료 PR:

### 완료 TASK

- [x] E00-T00
- [x] E00-T01

### 최종 검증

```text

```

### 산출물

- 

### 남은 TODO

- 

### 다음 EPIC 진입 가능 여부

- 가능/불가능:
- 이유:
```

## CI 실패 보고

```markdown
## CI 실패 보고

Branch:
PR:
Run:

### 실패 job

- 

### 실패 step

- 

### 핵심 오류

```text

```

### 원인 판단

- 실제 코드 오류:
- 문서/계약 위반:
- CI 오탐 가능성:

### 수정 계획

- 

### 금지 사항 확인

- [ ] 실패 검사를 삭제하지 않음
- [ ] test/assertion을 약화하지 않음
- [ ] schema-example-check case를 삭제하지 않음
- [ ] implementation-documentation-check required path를 이유 없이 제거하지 않음
- [ ] policy를 우회하지 않음
```

## Approval 필요 보고

```markdown
## Approval 필요 보고

작업:
Branch:
PR:

### 승인 필요 사유

- 

### 관련 파일

- 

### 위험

- 

### 사람이 답해야 할 질문

1. 
2. 

### 승인 전까지 멈출 작업

- 
```

## Release readiness 보고

```markdown
## Release readiness 보고

Release:
Target:
Version:
Status:

### Checks

- required-ci-passed:
- release-profile-passed:
- changelog-updated:
- version-consistent:

### Blockers

- 

### Approvals

- 

### 금지 사항 확인

- [ ] release/deploy/publish 자동 실행 없음
- [ ] repository settings 자동 변경 없음
- [ ] approval 없이 외부 계정 변경 없음
```

## 보고 원칙

- 성공했다고만 쓰지 말고 어떤 명령이 성공했는지 적는다.
- 실패 로그는 핵심 줄만 포함한다.
- 수동 승인 필요 여부를 숨기지 않는다.
- 다음 작업을 하나 이상 명확히 적는다.
- 불확실하면 불확실하다고 적는다.

## 검증 결과 표현

권장:

```text
passed
failed
skipped
blocked
not_run
```

`skipped`는 이유를 반드시 적는다.

## 사람이 확인해야 하는 항목

다음이 있으면 보고에 반드시 표시한다.

- dependency 또는 package manager 변경
- workflow 변경
- public API 변경
- schema breaking change
- Star Sentinel policy 변경
- release/deploy 관련 변경
- secret/credential 관련 변경
- test deletion 또는 weakening 의심
- implementation-documentation-check required path 변경
