# PLANS.md

## 완료 작업

| ID | 상태 | 목표 | 검증 상태 |
|---|---|---|---|
| P-0001 | DONE | v3/v4 설계를 Star-Control monorepo 스캐폴드와 정본 문서로 흡수 | 통과 |

## 결정 기록

| ID | 결정 |
|---|---|
| D-0001 | 원본 폴더는 삭제하지 않고, 정규 구조와 `source-absorption-map.md`로 흡수 상태를 기록한다. |
| D-0002 | 라이선스는 MIT를 사용한다. |
| D-0003 | provider core package에는 특정 provider 이름을 넣지 않고 transport/adapter/capability 중심으로 분리한다. |
| D-0004 | Star Sentinel의 과거 이름은 `tool.yaml` legacy alias와 원본 흡수 맵의 출처 표기에만 남긴다. |

## 변경 파일 목록

| 파일 | 상태 | 변경 요약 | 검증 상태 |
|---|---|---|---|
| 루트 스캐폴드 | 추가됨 | README, AGENTS, LICENSE, CHANGELOG, gitignore 생성 | 통과 |
| docs/ | 추가됨 | 정본 문서와 원본 흡수 맵 생성 | 통과 |
| specs/ | 추가됨 | schema와 contract 생성 | 통과 |
| configs/ | 추가됨 | 정책, 역할, hook, template, registry 흡수 | 통과 |
| builtin-providers/ | 추가됨 | provider manifest와 capability 생성 | 통과 |
| builtin-tools/star-sentinel/ | 추가됨 | Star Sentinel manifest, policy, schema, template, docs 생성 | 통과 |
| packages/, apps/, examples/, tests/, scripts/ | 추가됨 | 구현 전 스캐폴드 생성 | 통과 |

## 열린 리스크

| ID | 리스크 | 대응 |
|---|---|---|
| R-0001 | 실제 구현 언어와 패키지 매니저가 아직 확정되지 않음 | 의존성 추가 없이 스캐폴드와 문서만 생성 |
| R-0002 | 원본 폴더 삭제는 되돌리기 어려움 | 이번 작업에서는 삭제하지 않고 삭제 가능 판단 근거만 남김 |

## 검증 결과

| 항목 | 결과 |
|---|---|
| JSON 파싱 | `scripts/test.ps1` 통과 |
| 원본 커버리지 | `rg --files --hidden` 기준 v3 208개 + v4 29개 = 237개, 흡수 맵 237행 |
| Star Sentinel legacy alias | `builtin-tools/star-sentinel/tool.yaml`에만 남음 |
| 금지 provider core package | 검색 결과 없음 |
| old tool-output 경로 | 검색 결과 없음 |
| new tool-output 경로 | README, architecture, operations 문서에 반영 |
| runtime/ 루트 디렉터리 | 없음 |
| integrations/ provider 구현물 | 없음. `.gitkeep`만 존재 |

## 다음 조치

- 원본 폴더 삭제는 사용자가 별도 승인할 때만 진행한다.
- 실제 구현 언어와 패키지 매니저는 다음 구현 단계에서 결정한다.
