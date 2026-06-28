> 흡수 출처: `star-control_design_v3/docs/26_Context_Pack_and_Memory_Compaction.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 26. Context Pack과 Memory Compaction 설계

## 목적

Star-Control은 새 worker run을 계속 생성한다. 각 worker에게 전체 대화/전체 로그를 주면 토큰이 폭발한다. 따라서 Context Pack은 필수이며, 각 worker가 필요한 정보만 받도록 압축해야 한다.

## Context Pack 원칙

1. 전체 대화 금지.
2. 전체 diff 금지.
3. 전체 로그 금지.
4. 파일 경로, 결정, 제약, 검증 상태를 우선.
5. 다음 worker가 바로 실행할 수 있는 형태.
6. 추측과 사실을 구분.

## Context Pack 표준 구조

```yaml
context_pack:
  goal: "스톱워치 기능 구현"
  user_constraints:
    - "새 의존성 추가 금지"
    - "테스트 약화 금지"
  current_state:
    done:
      - "요구사항 정리 완료"
    remaining:
      - "구현"
      - "검증"
      - "리뷰"
  decisions:
    - id: D-0001
      decision: "외부 라이브러리 없이 구현"
      rationale: "작업 위험도 LOW, 의존성 추가 불필요"
  relevant_files:
    - path: "src/stopwatch.ts"
      reason: "구현 대상"
    - path: "tests/stopwatch.test.ts"
      reason: "검증 대상"
  validation:
    last_run: null
    required:
      - "npm test"
  risks:
    - "기존 UI 구조 확인 필요"
  forbidden:
    - "git commit"
    - "git push"
    - "파일 삭제"
```

## Compaction 계층

| 계층 | 입력 | 출력 |
|---|---|---|
| Raw Artifact | worker output, logs | artifact file |
| Stage Summary | 한 stage 결과 | `stage-summary.md` |
| Job Context Pack | 다음 worker용 압축 | `context-pack.yaml` |
| Project Memory | 프로젝트 장기 결정 | `PLANS.md`, `.star-control/memory.yaml` |

## 기억할 것 / 버릴 것

### 기억

- 사용자 제약
- 공개 API 결정
- 스키마/포맷 결정
- 검증 명령
- 실패 원인과 해결
- 파일/모듈 경계
- 승인 필요 항목

### 버림

- 중간 추론 전문
- 반복 로그
- 성공한 명령의 전체 출력
- 임시 조사 과정
- 같은 설명 반복

## Context Pack Builder 입력

```text
- request.md
- route.json
- run-state.json
- previous report.json
- PLANS.md
- changed_files summary
- validation summary
```

## Output 예시

```md
# Context Pack: J-0001 implement

## Goal
스톱워치 기능 구현.

## Constraints
- 새 의존성 추가 금지.
- 파일 삭제 금지.
- 테스트 약화 금지.

## Done
- 요구사항은 시작/정지/리셋/표시 형식으로 정리됨.

## Remaining
- 구현.
- 테스트.
- 리뷰.

## Relevant Files
- `src/App.tsx`: UI 진입점.
- `tests/App.test.tsx`: 기존 테스트.

## Required Validation
- `npm test`
```

## 구현 체크리스트

- [ ] 각 worker report를 stage summary로 압축한다.
- [ ] 다음 worker WorkSpec에는 Context Pack만 넣는다.
- [ ] PLANS.md는 장기 상태만 유지한다.
- [ ] context size budget을 정책화한다.
- [ ] 요약 실패 시 전체 로그를 넣지 말고 artifact path를 넣는다.
