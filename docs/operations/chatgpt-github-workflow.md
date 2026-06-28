# ChatGPT GitHub 작업 운영 지침

이 문서는 ChatGPT가 GitHub 연결을 통해 Star-Control repository를 수정할 때 확인할 운영 지침이다. Codex worker용 지침이 아니라, 대화형 ChatGPT가 GitHub connector로 브랜치, 파일, PR, CI, merge를 다룰 때의 기준이다.

## 기본 원칙

- `main`은 직접 수정하지 않는다.
- 작업마다 새 브랜치를 만들고 PR을 연다.
- PR 범위는 한 가지 목적에 맞게 작게 유지한다.
- 문서와 기본 응답은 한국어를 우선한다.
- 설정 키, 명령어, 파일명, 로그 원문은 원문 표기를 유지한다.
- 의존성 추가, 패키지 매니저 도입, 배포, 릴리즈, 외부 계정 수정은 명시 승인 전까지 하지 않는다.
- 실패한 검사를 삭제하거나 약화해서 통과시키지 않는다.

## 작업 전 확인

작업을 시작할 때는 필요한 파일만 최소 범위로 확인한다.

필수 기본 확인:

```text
README.md
AGENTS.md
.github/workflows/ci.yml
docs/operations/ci-roadmap.md
builtin-tools/star-sentinel/tool.yaml
```

작업 성격에 따라 관련 파일을 추가로 확인한다. 예를 들어 CI를 수정하면 `scripts/ci/`, 정책을 수정하면 `configs/policies/`, manifest를 수정하면 `builtin-tools/` 또는 `builtin-providers/`를 함께 본다.

## PR 작성 방식

- 가능한 한 파일 변경 수를 줄인다.
- workflow 변경은 한 번에 정확히 반영한다.
- PR 본문은 처음에는 요약 중심으로 작성하고, 최종 검증 결과는 merge 직전 한 번만 갱신한다.
- 중간 디버깅용 변경이 생기면 최종 merge 전 정리한다.
- 중간 커밋이 많아졌다면 squash merge를 우선 고려한다.

## CI 확인 방식

- PR 생성 후 GitHub Actions 결과를 확인한다.
- 실패하면 먼저 실패 job과 실패 step을 확인한다.
- 로그가 길면 핵심 실패 메시지, 파일 경로, 줄 번호를 우선 찾는다.
- CI 실패 원인이 실제 문법, 계약, 정책 위반이면 해당 원인을 고친다.
- CI 실패 원인이 오탐이면 검사 범위를 문서화된 의도에 맞게 좁힌다.
- 검사를 없애거나 성공으로 위장하지 않는다.

## merge 기준

다음 조건을 만족할 때만 merge한다.

- 변경 범위가 요청한 PR 목적에 맞다.
- 관련 CI job이 성공했다.
- 실패 원인이 있었으면 원인과 수정 내용을 PR 본문 또는 완료 보고에 남겼다.
- 의존성, 패키지 매니저, 배포, 외부 계정, 대량 이동 같은 승인 필요 작업을 몰래 포함하지 않았다.

## 완료 보고

완료 보고에는 다음을 요약한다.

- PR 번호와 제목
- merge 여부와 merge commit
- 변경 파일
- 검증 결과
- 남은 수동 작업이 있으면 명확히 표시

GitHub connector는 브랜치 삭제 기능을 노출하지 않을 수 있다. merge 후 작업 브랜치 삭제가 필요하면 사용자가 GitHub 웹 UI에서 처리한다.
