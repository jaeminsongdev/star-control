# P-0061 PR0~31 CrossRepoChangeBundle 최종 잠금

## 판정

PR0~31을 [source descriptor](../../catalog/cross-repo-change-bundles/pr0-pr31-final-lock.toml) 하나로 묶고 [Project Catalog](../../catalog/projects.toml)에 Server와 Deployment를 추가했다. wire materialization 대상은 `star.cross-repo-change-bundle@1`과 `star.change-bundle-participant@2`다.

aggregate는 `held`다. 일부 저장소가 `partial|unverified`이고 Danpung/Deployment production signing은 `not_provided`, publication은 `not_run`이다. Star-Control external signing/publication도 이 source-only bundle에서 실행하지 않는다. provider 또는 부분 성공을 전체 PASS로 승격하지 않는다.

## 상태 보존

| project | PR31 cutoff SHA | validation | evidence | approval | rollback | signing / publication |
|---|---|---|---|---|---|---|
| star-control | `36f8dc1e...` | pass | current | approved | partial | not_run / not_run |
| devtools | `f9b25576...` | pass | current | approved | not_run | N/A |
| content | `1c6187a0...` | unverified | missing | pending | not_run | N/A |
| server | `aa94f037...` | pass | current | approved | not_run | N/A |
| deployment | `a15d8426...` | pass | current | approved | partial | not_provided / not_run |
| danpung | `eb01ea33...` | partial | partial | approved | partial | not_provided / not_run |
| emulink | `f4324596...` | unverified | missing | pending | not_run | N/A |
| format | `b2a5e130...` | pass | current | approved | not_run | N/A |
| adapter | `415877cc...` | partial | partial | approved | not_run | N/A |
| mod-foundry | `5729d70b...` | unverified | missing | pending | not_run | N/A |
| language | `09aaf6d2...` | unverified | missing | pending | not_run | N/A |
| storage | `4d394774...` | partial | partial | approved | partial | N/A |
| ecosystem-canonical | `a5bc4ec0...` | pass | current | approved | N/A | N/A |
| knowledge | `2075c5e8...` | pass | current | approved | not_run | N/A |
| core | `95573fd5...` | pass | current | approved | not_run | N/A |

`approval=approved`는 PR31까지의 해당 source/evidence 수용 상태이며 signing, publication 또는 새 remote effect 승인이 아니다. Graphics `8fab0c98...`의 PR22~26 evidence는 explicit external evidence root로 보존했고, 이번 지시 범위 밖 catalog 등록으로 확장하지 않았다.

## 경계

- 활성 관제 정본은 Star-Control 하나다.
- `Star-Workflow`는 forbidden target이며 bundle participant, evidence sink 또는 handoff target이 아니다.
- source manifest가 정본이고 management DB/index는 derived state다.
- source catalog 등록은 installed Runtime refresh와 다르다. 이번 변경은 installer 재생성·설치 또는 외부 signing/publication을 수행하지 않는다.
- PR32 세 저장소 seal SHA는 tracked source의 자기참조를 피하도록 push handoff가 소유한다. descriptor cutoff는 PR31 remote-main이다.

## 검증 계약

`check_product_inventory.py`는 다음을 fail-closed로 확인한다.

1. catalog project 15개와 bundle participant 15개가 정확히 일치한다.
2. PR 번호가 0~31을 중복 없이 모두 포함한다.
3. 각 participant exact SHA와 validation/evidence/approval/rollback/signing/publication enum을 개별 검사한다.
4. partial/unverified/missing signing 상태에서 aggregate가 `completed` 또는 release-ready가 되는 것을 거부한다.
5. Server/Deployment origin과 path, Star-Control 단일 active control, Star-Workflow 금지와 Graphics external evidence 경계를 확인한다.
