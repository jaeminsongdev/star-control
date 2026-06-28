> 흡수 출처: `star-control_design_v3/docs/09_로컬모델_Adapter_설계.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 09. 로컬모델 Adapter 설계

## 1. 역할

로컬 모델은 Star-Control에서 보조 worker다.

용도:

- 초안 생성
- 요약
- 테스트 후보
- 로그 분석
- diff 1차 리뷰
- 반복적인 낮은 위험 작업
- 민감 코드의 클라우드 전송 전 사전 처리

금지:

- 최종 설계 판단
- 보안/권한/DB/스키마/동시성 최종 구현
- 공개 API 변경
- 파일 삭제
- 의존성 추가
- 원격 반영
- 테스트 약화

## 2. Provider 종류

```text
providers/local-ollama.yaml
providers/local-lmstudio.yaml
providers/local-vllm.yaml
providers/local-llamacpp.yaml
```

## 3. 처음 권장 방식

초기에는 로컬 모델이 repo를 직접 수정하지 않는다.

```text
WorkSpec → Local Model → draft.md / notes.md / test-candidates.md → Cloud Worker 검토 후 적용
```

## 4. local-ollama provider 예시

```yaml
provider: local-ollama

capabilities:
  edit_files: false
  run_shell: false
  structured_output: weak
  cheap_retry: true

roles:
  worker-local-draft:
    command: ollama
    args:
      - run
      - qwen2.5-coder:14b

outputs:
  - draft.md
  - notes.md
  - report.json
```

## 5. 역할별 권장 모델 크기

| 모델 | 권장 역할 |
|---|---|
| 7B | 요약, 로그 정리, 단순 문서 초안 |
| 14B | 단일 파일 초안, 테스트 후보, diff 1차 리뷰 |
| 30B+ | 작은 기능 초안, 제한된 설계 대안, 실패 원인 후보 |
| 70B+ | 제한된 구현 보조, 더 신뢰 가능한 리뷰 후보 |

## 6. 로컬 모델 품질 관리

로컬 모델 report는 반드시 Cloud A 또는 reviewer가 검토해야 한다.

체크:

- 존재하지 않는 API 사용 여부
- 하드코딩 여부
- 요구사항 외 구현 여부
- 테스트 약화 여부
- 보안/비밀정보 노출 여부

## 7. 장기 구조

장기적으로는 local model MCP bridge 또는 OpenAI-compatible local endpoint를 provider adapter로 붙일 수 있다.

```text
Star-Control
  ↓
LocalModelAdapter
  ↓
Ollama / LM Studio / vLLM / llama.cpp
```

## 8. 로컬 모델을 쓰면 좋은 순간

- 같은 파일 반복 요약
- 긴 로그 1차 분석
- 테스트 이름/케이스 후보 생성
- 코드 변환 초안
- 문서 초안
- 클라우드 토큰을 쓰기 아까운 탐색성 작업

## 9. 로컬 모델을 피해야 하는 순간

- 보안 관련 수정
- 공개 API 변경
- DB migration
- 동시성/비동기 버그
- 파일시스템/캐시/락 관련 버그
- 복잡한 테스트 실패 수정
- 대형 리팩토링
