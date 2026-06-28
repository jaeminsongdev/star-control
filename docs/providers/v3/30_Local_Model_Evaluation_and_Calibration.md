> 흡수 출처: `star-control_design_v3/docs/30_Local_Model_Evaluation_and_Calibration.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 30. Local Model Evaluation과 Calibration

## 목적

로컬 모델은 비용 절감에 유용하지만 최종 책임자로 두면 위험하다. Star-Control은 로컬 모델이 어떤 작업까지 가능한지 실측하고, 그 결과로 model-routing policy를 보정해야 한다.

## 평가 대상

```text
- 7B급
- 14B급
- 30B급
- 70B급 이상 가능 시
```

## 평가 작업군

| 작업군 | 예시 | 성공 기준 |
|---|---|---|
| 요약 | 파일/로그 요약 | 핵심 누락 없음 |
| 테스트 초안 | 단위 테스트 후보 작성 | 컴파일/실행 가능 후보 |
| 단일 파일 구현 | 작은 함수 추가 | 검증 통과 |
| diff 리뷰 | 변경 위험 찾기 | BLOCKER 감지율 |
| 실패 로그 분석 | 테스트 실패 원인 후보 | 실제 원인 포함 |
| 설계 대안 | 작은 구조 대안 | 무리한 추상화 없음 |

## 금지 평가 영역

초기에는 로컬 모델에 아래를 맡기지 않는다.

```text
- 공개 API 최종 결정
- DB 스키마 변경
- 보안/권한 변경
- 동시성/캐시/파일시스템 핵심 구현
- 대형 리팩토링
- 테스트 실패 우회 판단
```

## Evaluation harness

```text
evals/local-models/
  tasks/
    summarize-file/
    write-test/
    review-diff/
    explain-failure/
  expected/
  results/
```

## 점수화

```yaml
metrics:
  correctness: 0-5
  follows_constraints: 0-5
  hallucination_rate: 0-5
  runnable_output: 0-5
  review_usefulness: 0-5
  cost_latency: 0-5
```

## 라우팅 반영

```yaml
# policies/model-routing.yaml
local_model_thresholds:
  worker-local-draft:
    min_correctness: 3
    max_hallucination: 2
  reviewer-lite:
    min_review_usefulness: 3
```

## 승인 조건

로컬 모델이 특정 역할을 맡으려면 20개 이상 sample task에서 기준을 통과해야 한다.

## 구현 체크리스트

- [ ] eval task fixture 작성.
- [ ] local provider별 결과 저장.
- [ ] 평가 결과를 `provider-features/local-*.features.yaml`에 반영.
- [ ] model-routing policy가 평가 결과를 참조한다.
- [ ] 평가 실패 모델은 초안/요약 외 역할 금지.
