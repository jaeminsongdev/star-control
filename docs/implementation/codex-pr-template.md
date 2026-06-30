# Codex PR Template

## 목적

이 문서는 Codex가 구현 PR을 만들 때 사용할 표준 PR 본문 형식이다. 실제 GitHub PR 기본 템플릿은 `.github/pull_request_template.md`에 둔다. 이 문서는 상황별 확장 템플릿과 작성 규칙을 설명한다.

## 실제 PR template

```text
.github/pull_request_template.md
```

실제 template에는 다음 section이 있어야 한다.

```text
목표
변경 범위
변경 파일
검증
계약 준수
위험 / 승인 필요 여부
다음 작업
```

## 기본 작성 규칙

- 변경 목표는 EPIC/TASK id와 한 문장 요약으로 시작한다.
- 변경 범위를 docs/schema/CI/runtime/approval required 중 하나 이상으로 표시한다.
- 검증은 실행 명령과 GitHub Actions 결과를 분리해서 적는다.
- 실패한 검증을 숨기지 않는다.
- approval required change는 PR 본문에서 반드시 드러낸다.
- 긴 로그 전체를 붙이지 않고 핵심 오류만 요약한다.

## 문서-only PR 템플릿

```markdown
## 목표

문서 계약을 보강합니다.

## 변경 범위

- [x] docs only
- [ ] schema/example contract
- [ ] CI validator
- [ ] runtime code
- [ ] approval required change

## 변경 파일

- `docs/implementation/...`

## 변경 내용

- 구현 경계 명시
- 금지 사항 명시
- 테스트/검증 기준 명시

## 검증

- GitHub Actions 결과:

## runtime 영향

- runtime code 변경 없음
- dependency 변경 없음
```

## schema/example PR 템플릿

```markdown
## 목표

새 data contract와 canonical example을 추가합니다.

## 변경 파일

- `specs/schemas/...`
- `examples/...`
- `scripts/ci/check_schema_examples.py`
- `docs/implementation/...`

## 계약 추가 절차

- [ ] schema 추가
- [ ] canonical example 추가
- [ ] schema-example-check case 추가
- [ ] 구현 문서에 machine-readable contracts section 추가
- [ ] 필요 시 implementation-documentation-check required path 추가

## 검증

```text
python3 scripts/ci/check_schema_examples.py
```
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

- 실제 코드 오류:
- 문서/계약 위반:
- CI 오탐 가능성:

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
- [ ] implementation-documentation-check required path를 이유 없이 제거하지 않음
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

## 승인 전까지 멈출 작업

- 

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
security:
release:
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
- PR 본문이 실제 변경 범위와 맞지 않으면 후속 정리 PR에서 고친다.
