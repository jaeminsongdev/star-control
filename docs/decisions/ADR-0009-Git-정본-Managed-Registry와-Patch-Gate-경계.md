# ADR-0009: Git 정본 Managed Registry와 Patch·Gate 경계

## 상태

- 상태: 채택
- 결정일: 2026-07-14
- 적용 단계: 5단계 관리형 Symbol·상수·에러 코드 Registry

## 맥락

여러 Project·언어·Schema·문서가 같은 계약 값을 소비하면 선언 위치, stable ID, 언어별 symbol, 호환 기간과 제거 시점을 함께 추적해야 한다. 반면 scanner가 발견한 literal, 한 구현 안에서만 쓰는 상수와 사용자가 승인한 공유 계약을 같은 중앙 Registry에 넣으면 잘못된 결합과 대량 치환 위험이 생긴다. 0단계 관리 DB는 derived state이며 1단계 Index, 2단계 영향 분석, 3단계 Gate와 4단계 Patch 경계를 우회할 수 없다.

## 결정

1. `managed declaration`, `candidate`, `local implementation constant`를 서로 다른 분류로 유지한다. scanner 결과는 승인된 Git 선언 없이는 `managed declaration`이 되지 않는다.
2. Managed Registry의 정본은 Project의 Git manifest다. 관리 DB와 `ManagedRegistrySnapshot`은 검색·영향 분석을 위한 derived Index이며 source와 다르면 stale이다.
3. stable ID·공개 ID·namespace tombstone은 재사용하지 않는다. 같은 raw 값도 의미·owner·lifecycle이 다르면 합치지 않는다.
4. DB 관리 surface는 source를 직접 쓰지 않는다. typed change intent에서 M2 `ChangePlan`, M4 dry-run과 immutable single-Project `PatchSet`, 승인과 M3 pre/post Gate를 거쳐 source를 변경한다.
5. generated output은 generator가 소유하고 직접 편집하지 않는다. 구조적 binding 생성은 codegen, 기존 source와 consumer 전환은 검증 가능한 codemod 또는 수동 검토를 사용한다.
6. 첫 수직 Slice는 error code와 Diagnostic ID다. display message 변경은 stable code 변경과 분리하고, deprecation·bounded alias·consumer 전환·removal·영구 tombstone 순서를 사용한다.
7. 여러 Project의 영향과 compatibility는 read-only로 계산한다. 실제 cross-repo 적용은 9단계 전에는 지원하지 않는다.

상세 field, lifecycle, conflict, evidence와 6단계 drift 입력은 [관리형 Symbol·상수·에러 코드 Registry 계약](../contracts/managed-symbol-registry.md)이 소유한다.

## 결과

- DB 손실·stale 상태에서도 Git source에서 Registry Index를 재구축할 수 있다.
- Registry 변경은 기존 M2·M4·M3 안전 경계를 재사용하고 별도 mutation 경로를 만들지 않는다.
- candidate와 local constant 검색은 유지하되 중앙 소유권을 강요하지 않는다.
- alias 기간과 consumer 최소 지원 version을 근거로 호환 가능·전환 필요·제거 차단을 구분할 수 있다.
- 6단계는 source manifest, binding, generated output, docs·Schema와 consumer 관계의 drift를 결정적으로 비교할 수 있다.

## 기각한 대안

- **DB row를 정본으로 source 문자열을 동기화**: source review·revision·Patch Gate를 우회하므로 기각한다.
- **모든 literal과 local constant를 중앙 관리**: 의미가 다른 값을 결합하고 구현 세부를 공개 계약으로 승격하므로 기각한다.
- **generated source 직접 수정**: generator provenance와 재생성 결정성을 깨므로 기각한다.
- **ID 제거 후 재사용**: 과거 evidence와 consumer가 새 의미로 오인할 수 있어 기각한다.
- **5단계에서 cross-repo 원자 적용**: worktree·승인·복구 소유권이 9단계 범위이므로 보류한다.

## 관련 정본

- [공통 개발 관리와 로컬 관리 DB 계약](../contracts/development-management.md)
- [Project Catalog와 Code Index 계약](../contracts/project-catalog-and-code-index.md)
- [변경 계획·영향 분석 계약](../contracts/change-planning-and-impact.md)
- [안전한 Patch·Refactor·codemod 엔진 계약](../contracts/safe-patch-and-codemod.md)
- [공통 검증·품질 Gate](../features/common-validation-gate.md)
