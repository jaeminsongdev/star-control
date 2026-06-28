# Star Sentinel

Star Sentinel은 Star-Control에 기본 탑재되는 AI 코드 변경 검증 도구다.

## 책임

- AI가 만든 변경사항을 diff, policy, evidence, validation 기준으로 검증한다.
- 테스트 삭제/약화, secret, dependency 변경, scope 위반, validator self-bypass를 탐지한다.
- review pack과 approval gate를 생성한다.

## 경계

- 구현 코드는 `packages/star-sentinel/`에 둔다.
- manifest, policy, schema, template, corpus는 `builtin-tools/star-sentinel/`에 둔다.
