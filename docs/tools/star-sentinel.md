# Star Sentinel

Star Sentinel은 Star-Control의 기본 검증 도구다. AI가 만든 변경사항을 diff, policy, evidence, validation 결과로 평가하고 review pack과 approval gate를 생성한다.

## 경계

- Star-Control core는 Star Sentinel을 tool adapter 계약으로 호출한다.
- 구현 코드는 `packages/star-sentinel/`에 둔다.
- 등록정보, policy, schema, template, corpus는 `builtin-tools/star-sentinel/`에 둔다.
- Star-Control `WorkSpec`과 Star Sentinel `SentinelTask`는 별도 계약으로 유지한다.

## 입력

- 대상 repository 경로.
- 변경 diff와 changed line map.
- Star-Control route/work metadata.
- policy profile과 required validation 목록.
- provider가 제출한 claim, log, report.

## 출력

Star Sentinel 산출물은 대상 프로젝트 실행 디렉터리 아래에 둔다.

```text
대상 프로젝트/.ai-runs/J-0001/tool-output/star-sentinel/
  repo_map.json
  changed_lines.json
  diagnostics.json
  validation_runs.json
  review_pack.md
  approval.json
  ledger.jsonl
```

## 판정

- `AUTO_PASS`: 정책 위반이 없고 required validation이 통과했다.
- `HUMAN_REVIEW`: 사람이 확인해야 할 diagnostic이나 불완전한 evidence가 있다.
- `BLOCK`: 금지 변경, secret, validator bypass, validation 약화 등 차단 사유가 있다.

## 구현 우선순위

1. repo root detector, file classifier, git diff parser.
2. risk path classifier, policy registry, diagnostic model.
3. scope validator, test weakening detector, secret detector.
4. dependency approval validator, validator selfguard, AI claim verifier.
5. review pack generator, approval gate, run/task ledger.
