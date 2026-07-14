# 확장 운영 기능

상위 범위와 공통 선정 기준은 [구현 대상 기능](README.md)에서 확인한다.


## D01. 여러 프로젝트·원격 Git·자료조사

하나의 목표가 여러 저장소와 최신 외부 사실에 걸릴 때도 같은 관제 원칙을 유지한다. 실행 계약은 [9단계 CrossRepo ChangeBundle](../contracts/cross-repo-change-bundle.md)이 소유한다.

- `MultiProjectGoal`의 stable ProjectId, provider·consumer·data owner·tooling relation과 dependency DAG
- provider 호환 경계 개방, consumer 전환, compatibility window 종료와 provider old path 제거 순서
- project별 `ChangeBundleParticipant`, exact base·dirty state·PatchSet·worktree·Gate·EvidenceBundle
- 여러 repository를 하나의 transaction이라고 부르지 않고 effect·receipt·compensation을 project별로 기록
- `prepared|awaiting_apply|partially_applied|awaiting_validation|rollback_required|held|completed|outcome_unknown` 구분
- repository별 merge queue, conflict 양쪽 intent·contract와 stale base 재계획
- project-local 검사와 전체 Goal Gate, 일부 성공 뒤 resume·roll-forward·compensate·hold
- local validated/commit/branch 상태와 remote push·PR·check·merge 상태의 독립 축
- 원격 branch·PR·check·release는 adapter `RemoteStateSnapshot`으로만 관찰하고 추측하지 않음
- upload, PR 생성·수정, remote merge와 publish는 action별 명시적 승인 없이는 실행하지 않음
- 10단계 release가 project별 immutable source revision·artifact hash·Gate를 소비하는 handoff
- Codex 병렬 작업은 선택 소비자이고 CLI-only local ChangeBundle이 core 경로
- Codex를 통한 자료조사 단계와 출처 URL, 확인 날짜, 적용 판단 기록
- 최신성이 필요한 주장은 근거 없이 확정하지 않고 재확인 대상으로 표시

remote adapter가 capability나 성공 response를 반환해도 permission·approval 또는 actual merged/published state가 되지 않는다. after snapshot이 exact commit·PR·check·release를 확인하지 못하면 `unverified|outcome_unknown`이다.

## D02. 비용·평가·규칙 개선

자동 배정과 검증 규칙이 실제로 1인 개발자의 시간을 줄이는지 [10단계 EvaluationRun 계약](../contracts/ci-release-evaluation-and-product-completion.md#evaluationrun-v2-평가-단위)으로 측정하고 근거 있게 조정한다.

- Rule·Check·Profile·Recipe별 실행 시간, finding 수, actual defect, false positive, flaky와 suppression 상태 기록
- 기존 부채 `existing_unchanged`와 새 code의 `new|worsened`를 분리하고 새 악화 방지를 우선
- 같은 case·source·tool·config·environment의 baseline/candidate 비교
- 변경 전후 재계획·재실행·수동 수정·검토 시간, 실패·rollback·revert와 사용자 수락 비교
- provider가 검증 가능한 usage·금액·price source를 제공할 때만 비용 기록; 없으면 `measurement_unavailable`
- 실제 작업을 바꾸지 않는 offline·replay·shadow 비교로 새 규칙 시험
- 효용 부족, 높은 오탐·flaky·suppression 확대, 비교 불가이면 `trial|reject|needs_review`
- validator·severity·required Check·Corpus·freshness를 약화해 통과율을 높이는 candidate 거부
- 실제 성공·실패 사례를 출처·동의·redaction·version이 있는 평가 Corpus로 축적
- Maintenance Radar에 오래된 item의 last evaluation·replacement·deprecation deadline 연결
- Rule·Check·Profile·Recipe의 `active -> deprecated -> retired`와 migration·tombstone 보존
- CLI-only와 Codex-integrated context의 시간·재작업·usage·효용을 별도 cohort로 측정

스스로 학습해 통제되지 않게 바뀌는 router는 만들지 않는다. `EvaluationRun` recommendation은 Catalog나 설정을 자동 수정하지 않으며, 규칙 개선은 review된 source change·migration과 M3 Gate로만 반영한다.

## D03. Windows 배포와 제품 수명주기

개인 사용뿐 아니라 공개 배포를 전제로 설치부터 제거까지 관리한다. 검사 계층·artifact 승격·release 상태는 [10단계 정본](../contracts/ci-release-evaluation-and-product-completion.md), 실제 설치 layout은 [설치와 공개 배포](../operations/installation.md)가 소유한다.

- Windows x64·ARM64 설치, 초기화, 진단, update·rollback과 uninstall
- Codex Plugin·MCP·Hook package와 신뢰·활성 상태 검사
- Controller 시작 방식과 종료·재시작·crash 복구
- 설정·상태·artifact 형식 version과 안전한 migration
- 공개용 `safe_default`와 개인용 `personal_auto` 예시
- 상태 export, 진단 Pack, 복구와 보존 자료 정리
- clean Windows build·test·package와 배포 artifact file list, checksum, 출처·license·release readiness
- 한 번 build한 immutable artifact를 검증·승격하고 source revision·digest 연결
- SBOM·provenance·signing의 공개 배포 필요성별 조건부 Gate
- `ready`, action별 `approved`, 실제 remote after-state의 `published` 분리
- update·deploy 실패 시 검증된 이전 version으로 되돌리고 user config·state·evidence 보존
- 기본 uninstall은 program payload만 제거하고 user data purge는 별도 destructive action

자체 browser UI와 반복 예약 기능은 포함하지 않는다. 사용자 입력은 Codex 앱 또는 CLI-only TaskSpec, 제품 조작과 상태 확인은 터미널을 사용한다. Star-Control은 installer·CI·signing·artifact registry·deploy service를 재구현하지 않는다.
