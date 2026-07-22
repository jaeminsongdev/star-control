# ADR-0010: Build Once 승격과 Release·평가 Gate 경계

## 상태

- 상태: 채택
- 결정일: 2026-07-14
- 적용 단계: 10단계 CI·Release·배포 준비, 규칙 평가와 최종 제품 완성

## 맥락

로컬·CI에서 검사한 source와 실제 배포 byte가 다르면 검사 통과가 release 안전성을 증명하지 못한다. 또한 release 준비, 외부 publish 승인과 실제 원격 공개 결과를 하나의 `released` boolean으로 합치면 승인 전 외부 effect나 확인되지 않은 성공을 숨길 수 있다.

Rule·Check·Profile·Recipe 개선도 통과율이나 실행 시간만 보면 validator 약화, false positive 증가와 기존 evidence 재해석을 효율 개선으로 오판할 수 있다. CLI-only core와 Codex 선택 연동의 효용도 서로 다른 비용·재작업 구조를 가지므로 같은 cohort로 합치면 안 된다.

## 결정

1. release candidate는 exact source revision·Task ID·config·Catalog·Tool·Profile fingerprint에서 한 번 build·package하고 final artifact set digest로 봉인한다.
2. verification과 promotion은 같은 artifact byte를 재사용한다. recompile, 재압축, signing 등으로 byte가 바뀌면 새 candidate와 새 release Gate가 필요하다.
3. `ready`, `approved`, `published`를 분리한다. `ready`는 current release Gate, `approved`는 exact remote action의 single-use 사용자 승인, `published`는 provider after snapshot의 실제 결과 확인이다.
4. publish·deploy·withdrawal·rollback은 action별 승인과 before/after remote observation을 가진다. adapter success response만으로 `published`를 만들지 않는다.
   ReleaseManifest top-level 상태는 주 publication lifecycle이고, deploy·withdraw·rollback은 target별 remote action 상태를 유지한다. deploy 승인이 기존 `published`를 되감거나 한 target 결과가 다른 target을 채우지 않는다.
5. install·update·rollback·uninstall은 program payload와 user config·management state·project evidence의 ownership을 분리한다. uninstall은 기본적으로 사용자 자료를 보존하고 purge는 별도 destructive action이다.
6. SBOM·provenance·signing은 release policy가 `required|not_required|unavailable|incomplete|complete`로 각각 판정한다. public 배포 구현 직전 현재 채널·Windows·signing 공식 요구를 다시 조사한다.
7. EvaluationRun은 Rule·Check·Profile·Recipe별 actual defect, false positive, flaky, suppression, duration, failure, rework와 검증된 비용을 baseline/candidate로 비교한다. 미측정 값은 0으로 만들지 않는다.
8. validator guard·Corpus·new/worsened ratchet은 평가의 protected metric이다. 이를 약화해 얻은 pass율·시간 개선은 candidate accept 근거가 아니다.
9. 오래된 Catalog item은 `active -> deprecated -> retired`와 replacement·migration·tombstone을 사용한다. trial candidate rejection도 historical EvaluationRun과 ID/version tombstone을 보존한다.
10. CLI-only와 Codex-integrated EvaluationRun을 별도 context로 유지한다. Codex는 core release/evaluation state machine의 필수 dependency나 Writer가 아니다.
11. ReleaseManifest·EvaluationRun·Gate·event·current projection은 Controller가 single writer다. CI, build, installer, signer와 remote adapter는 typed observation·artifact·receipt만 반환한다.

상세 상태기계, 검사 계층, metric, 소유권과 구현 순서는 [10단계 정본](../contracts/ci-release-evaluation-and-product-completion.md)이 소유한다.

## 결과

- 검증한 source와 publish한 artifact byte를 digest로 끝까지 추적할 수 있다.
- release readiness와 외부 권한·실제 원격 결과가 섞이지 않는다.
- architecture별 package·lifecycle·rollback 증거가 같은 release subject에 연결된다. `v0.1.0`의 x64 Stable·ARM64 `native_unverified` Preview evidence floor는 ADR-0015가 소유한다.
- 규칙 개선이 validator 약화나 오탐 은폐가 아니라 실제 결함·재작업·시간 근거로 판단된다.
- 제품 source·Catalog·policy는 review된 정본으로 남고 DB·EvaluationRun이 자동으로 역쓰기하지 않는다.
- CLI-only 제품 효용과 Codex 연동의 추가 효용을 독립적으로 판단할 수 있다.

## 기각한 대안

- **release 단계에서 다시 build**: 검증한 byte와 배포 byte가 달라질 수 있어 기각한다.
- **`released=true` 하나로 상태 표현**: 준비·승인·외부 결과·미확정 상태를 숨기므로 기각한다.
- **signing·SBOM placeholder를 항상 생성**: 실제 적용 필요성·coverage를 거짓으로 보이게 하므로 기각한다.
- **pass율만으로 Rule·Profile 자동 갱신**: validator 약화와 false positive·false negative를 숨길 수 있어 기각한다.
- **EvaluationRun이 Catalog를 자동 수정**: review·migration·source 정본 경계를 우회하므로 기각한다.
- **Codex 결과를 CLI-only core Gate로 사용**: 결정적 도구·사람 검토 경계를 흐리므로 기각한다.
- **uninstall·rollback 시 user state 정리**: 복구 자료와 사용자 설정을 잃을 수 있어 기본 경로에서 기각한다.

## 관련 정본

- [검사·완료·증거](../contracts/validation-and-evidence.md)
- [Version과 Migration](../contracts/versioning-and-migrations.md)
- [9단계 CrossRepo ChangeBundle](../contracts/cross-repo-change-bundle.md)
- [승인·권한·안전](../architecture/security-and-permissions.md)
- [상태 기록과 이어하기](../architecture/state-and-artifacts.md)
- [설치와 공개 배포](../operations/installation.md)
- [x64 Stable과 ARM64 Preview 출시 정책](ADR-0015-x64-Stable과-ARM64-Preview-출시-정책.md)
