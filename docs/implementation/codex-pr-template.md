# Codex PR Template

## 목적

이 문서는 Codex가 구현 PR을 만들 때 사용할 표준 PR 본문 형식이다. 목표는 사람이 빠르게 변경 범위, 검증 결과, 남은 위험을 확인할 수 있게 하는 것이다.

## 기본 템플릿

```markdown
## 목표

- 이 PR이 해결하는 TASK 또는 EPIC:
- 한 문장 요약:

## 변경 파일

- `path/to/file`
- `path/to/test`

## 변경 내용

- 
- 
- 

## 수정 금지 파일 준수

- [ ] workflow 변경 없음
- [ ] 의존성/package manager 변경 없음
- [ ] release/deploy/external account 변경 없음
- [ ] test/CI/policy 약화 없음
- [ ] Star-Control repo 내부 `.ai-runs/` 생성 없음

## 검증

실행한 명령:

```text

```

결과:

```text

```

## 위험 / 승인 필요 여부

- approval required change 여부:
- 남은 risk:
- 사람이 확인해야 하는 사항:

## 다음 작업

- 
```

## 문서-only PR 템플릿

```markdown
## 목표

문서 계약을 보강합니다.

## 변경 파일

- `docs/implementation/...`

## 변경 내용

- 구현 경계 명시
- 금지 사항 명시
- 테스트/검증 기준 명시

## 검증

- GitHub Actions 통과 예정 또는 통과 결과 기록

## runtime 영향

- runtime code 변경 없음
- dependency 변경 없음
```

## 구현 PR 템플릿

```markdown
## 목표

EPIC/TASK: `E00-T00`

## 선행 문서

- `docs/implementation/...`

## 변경 파일

- 

## 구현 내용

- 

## 테스트

- 

## 검증 명령

```text

```

## 계약 준수

- [ ] allowed files만 수정
- [ ] forbidden actions 없음
- [ ] schema/example 계약 준수
- [ ] StateStore artifact layout 준수
- [ ] provider-neutral naming 준수

## 남은 작업

- 
```

## CI 실패 수정 PR 템플릿

```markdown
## 목표

CI 실패 원인을 수정합니다.

## 실패 정보

- 실패 job:
- 실패 step:
- 핵심 오류:

## 원인 판단

- 

## 수정 내용

- 

## 재검증

```text

```

## 금지 사항 확인

- [ ] 실패 검사를 삭제하지 않음
- [ ] assertion을 약화하지 않음
- [ ] schema-example-check case를 삭제하지 않음
- [ ] naming policy를 우회하지 않음
```

## approval required PR 템플릿

```markdown
## 목표

Approval required change를 제안합니다.

## 승인 필요 사유

- [ ] dependency change
- [ ] workflow change
- [ ] public API change
- [ ] schema breaking change
- [ ] validator policy change
- [ ] release/deploy/external account change
- [ ] 기타:

## 변경 내용

- 

## 위험

- 

## 사람이 승인해야 하는 질문

1. 
2. 

## 검증

```text

```
```

## PR 제목 규칙

권장 prefix:

```text
docs:
contracts:
specs:
ci:
state:
schema:
provider:
router:
execution:
validation:
sentinel:
cli:
```

피해야 할 제목:

```text
big update
finish everything
fix all
massive rewrite
```

## 본문 작성 원칙

- 긴 로그 전체를 붙이지 않는다.
- 핵심 오류와 결과만 요약한다.
- 사람이 승인해야 하는 조건은 숨기지 않는다.
- 변경 범위가 커지면 PR을 쪼갠다.
