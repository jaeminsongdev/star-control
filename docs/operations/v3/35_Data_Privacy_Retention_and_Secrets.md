> 흡수 출처: `star-control_design_v3/docs/35_Data_Privacy_Retention_and_Secrets.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 35. Data Privacy, Retention, Secret 관리

## 목적

Star-Control은 여러 클라우드 AI와 로컬 모델을 오가므로 어떤 데이터가 어디로 전달되는지 통제해야 한다.

## 데이터 분류

| 등급 | 예시 | 클라우드 전송 |
|---|---|---|
| PUBLIC | 오픈소스 코드, 공개 문서 | 허용 가능 |
| INTERNAL | 개인 프로젝트 코드 | 사용자 정책에 따름 |
| SENSITIVE | 비공개 키, 토큰, 개인정보 | 금지 |
| SECRET | API key, password, credential | 금지 |
| REGULATED | 의료/법률/금융 민감자료 | 별도 승인 |

## Provider 전송 정책

```yaml
# policies/data-policy.yaml
cloud_allowed:
  - PUBLIC
  - INTERNAL
cloud_forbidden:
  - SENSITIVE
  - SECRET
  - REGULATED
local_allowed:
  - PUBLIC
  - INTERNAL
  - SENSITIVE
local_forbidden:
  - SECRET
```

## Secret Guard

작업 전후 아래를 검사한다.

```text
- .env
- *_KEY
- *_TOKEN
- password
- private key block
- credential file
- cloud config
```

비밀정보가 발견되면:

1. 값을 출력하지 않는다.
2. 위치만 보고한다.
3. 클라우드 provider로 전달하지 않는다.
4. 사용자 승인 없이 파일을 수정하지 않는다.

## Run artifact 보존 정책

```yaml
retention:
  runs: 30d
  raw_logs: 7d
  summaries: 180d
  approvals: 365d
  final_reports: keep
```

## 로그 정책

- 전체 stdout/stderr를 기본 보존하지 않는다.
- 실패 로그는 요약 + artifact path.
- secret pattern 발견 시 redaction.
- provider request/response 원문 저장은 기본 비활성화.

## Redaction 예시

```text
OPENAI_API_KEY=sk-... → OPENAI_API_KEY=[REDACTED]
```

## 구현 체크리스트

- [ ] data classification 필드 추가.
- [ ] ProviderAdapter가 전송 전 secret scan을 호출한다.
- [ ] redaction 후 artifact 저장.
- [ ] raw logs 보존 기간 설정.
- [ ] cloud forbidden 데이터 감지 시 BLOCK.
