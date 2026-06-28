> 흡수 출처: `star-control_design_v3/docs/19_Migration_and_Provider_Update_Playbook.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 19. Provider 업데이트 / 마이그레이션 플레이북

## 1. 목적

AI provider 기능은 자주 바뀐다. Star-Control은 provider별 기능 변화를 흡수할 수 있어야 한다.

이 문서는 provider 기능 확인, feature matrix 갱신, renderer 수정, regression test 절차를 정의한다.

---

## 2. 업데이트 주기

```text
정기 점검: 월 1회
긴급 점검: provider CLI/API 오류 발생 시
주요 점검: 신규 provider 추가 전
```

---

## 3. 점검 대상

Provider별 확인 항목:

```text
CLI 버전
설정 파일 형식
profile/config 지원 여부
skills/hooks/rules 지원 여부
subagent/worker 지원 여부
thread resume/fork 지원 여부
structured output 지원 여부
sandbox/permission 변경
MCP 변경
비용/쿼터 정책 변경
```

---

## 4. Provider 갱신 절차

1. 공식 문서 확인.
2. provider CLI/API 버전 확인.
3. `provider-features/*.features.yaml` 갱신.
4. renderer 산출물 변경 필요 여부 확인.
5. adapter smoke test 실행.
6. fake provider test 재실행.
7. golden test 재실행.
8. migration note 작성.

---

## 5. 문서 출처 기록

각 provider feature file은 출처를 기록해야 한다.

```yaml
provider: codex
last_verified: "2026-06-28"
sources:
  - title: "Codex CLI reference"
    url: "https://developers.openai.com/codex/cli/reference"
  - title: "Codex Skills"
    url: "https://developers.openai.com/codex/skills"
```

---

## 6. Breaking Change 처리

Breaking change 예:

```text
CLI 옵션 이름 변경
output schema 지원 방식 변경
profile config 위치 변경
skills 폴더 구조 변경
hook event schema 변경
permissions 설정 변경
```

처리:

```text
1. provider-features status를 degraded로 변경
2. 해당 adapter를 disabled로 표시
3. fallback provider 선택
4. renderer 업데이트
5. compatibility test 추가
```

---

## 7. 새 Provider 추가 절차

1. provider 기능 조사.
2. `provider-features/<provider>.features.yaml` 작성.
3. `providers/<provider>.yaml` 작성.
4. adapter contract 구현.
5. 최소 smoke test 작성.
6. fake run 테스트.
7. capability registry와 매핑.
8. docs/99_참고자료.md 갱신.

---

## 8. Codex 중심에서 Star-Control 중심으로 이전

기존 Codex 전용 파일:

```text
low-router.config.toml
worker-impl.config.toml
command_block.rules
.agents/skills/*
```

최신 위치:

```text
roles/*.md
skills/*.md
policies/*.yaml
providers/codex.yaml
renderers/codex/*
```

이전 원칙:

```text
Codex 파일을 원본으로 보지 않는다.
Star-Control 원본에서 Codex 파일을 재생성한다.
```

---

## 9. Provider Feature Matrix 변경 로그

`provider-features/CHANGELOG.md`를 둔다.

예:

```md
## 2026-06-28
- Codex: output_schema 지원 확인
- Gemini CLI: extensions에 hooks/subagents/skills 포함 확인
- Claude Code: hooks event schema 갱신 필요
```

---

## 10. Acceptance Criteria

- provider 기능 변화가 Core Engine을 깨지 않는다.
- adapter 비활성화 시 fallback이 동작한다.
- feature matrix에서 unknown/native/emulated 상태를 추적한다.
- 공식 출처와 검증 날짜가 남는다.
