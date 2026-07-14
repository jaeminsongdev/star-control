# 구현 대상 기능

## 문서의 역할

이 문서는 새 Star-Control에 실제로 구현할 개념 기능을 정한다. 다음 세 자료군의 기능을 합쳐 중복을 제거하고, Star-Control의 핵심 목적과 1인 개발자 순효용 기준을 모두 통과한 요소만 남겼다.

- `D:/개발/관제/star_control_developer_tools_01_15/`의 15개 개발 도구 설계 문서
- `D:/개발/관제/custom_dev_verification_platform_design_v4_curated/`의 선별형 검증 플랫폼 설계 자료
- [레거시 기능 카탈로그](../history/legacy-feature-catalogue.md)의 역사적 기능 목록

이 문서에 적힌 기능은 최종 제품 범위다. 자료에 등장했더라도 이 문서에 포함되지 않은 기능은 이후 별도 재판정을 받기 전까지 구현 범위가 아니다.

이 문서는 기능의 목적과 책임만 소유한다. 외부 도구, 세부 알고리즘, Schema 필드, 명령 이름과 임계값은 각 기능의 하위 정본 계약에서 확정한다. 현재 MCP 범위의 exact 값과 구현 상태는 [MCP 구현 동결 계약](../contracts/mcp-implementation-contract.md)과 [MCP 완료 감사](../testing/mcp-completion-audit.md)를 따른다.

## 선정 기준

구현 대상으로 남긴 기능은 다음 조건을 만족한다.

1. 목표를 단계로 나누고 Codex 실행·검사·병합을 제어하는 핵심 흐름에 직접 필요하다.
2. 혼자 개발할 때 반복 시간, 실수 위험, 기억 부담 또는 복구 비용을 실제로 줄인다.
3. Codex나 기존 전문 도구가 수행할 기술을 다시 만들지 않고도 고유한 관제 가치를 제공한다.
4. Star-Control이 여러 작업에 걸친 상태, 정책, 판단, 증거 또는 순서를 소유해야 한다.
5. 일반 작업에 불필요한 질문과 전체 검사를 강요하지 않고 위험에 따라 작동 범위를 조절할 수 있다.
6. Windows 개인 개발 환경과 공개 배포용 안전 기본값을 모두 설정으로 지원할 수 있다.

## 구현 형태의 경계

| 구현 형태 | Star-Control이 담당하는 것 |
|---|---|
| 직접 구현 | 목표·단계·상태·정책·배정·증거·승인·병합처럼 여러 작업을 잇는 관제 책임 |
| Codex 연결 | 검색, 코드 수정, 인터넷 조사, 작업 생성·재개·분기 같은 Codex 제공 능력의 호출과 결과 기록 |
| 프로젝트 도구 연결 | 컴파일러, LSP, 테스트기, 패키지 관리자, codemod, 스캐너, 디버거, profiler, CI·배포 도구의 실행과 결과 정규화 |
| 설정·템플릿 | 프로젝트별 위험 경로, 검증 명령, 계약, 예산, 승인 정책, 개발 작업 프로필 |

전문 분석기나 실행기를 Star-Control 안에 다시 만드는 것은 구현 대상으로 보지 않는다. Star-Control은 필요한 도구를 발견하고, 안전한 입력으로 실행하고, 결과를 공통 증거로 연결하는 계층을 구현한다.

## 전체 기능 흐름

```text
사용자 목표
  -> 질문으로 모호함 제거
  -> 목표·범위·완료 조건 고정
  -> 프로젝트와 변경 영향 파악
  -> 성격이 다른 단계와 의존관계 계획
  -> 단계별 Codex 모델·생각 깊이·실행 방식 배정
  -> 필요한 자료와 권한만 전달
  -> Codex 작업 실행·병렬 작업·중단·재개
  -> 변경 범위와 필요한 검사 선택
  -> 프로젝트 도구로 검증
  -> 주장·변경·검사 증거 대조
  -> 자동 통과·본인 확인·차단 판정
  -> worktree 결과 병합과 통합 검사
  -> clean CI·package·install lifecycle과 artifact digest 검증
  -> release ready 판정
  -> 명시적 승인 뒤 publish·deploy, 원격 after-state 확인
  -> 최종 보고·이어하기·비용·평가 자료 기록
  -> Rule·Check·Profile·Recipe baseline/candidate 평가와 review된 개선
```

## 구현 대상 요약

| ID | 구현 대상 | 1인 개발자 효용 | 구현 형태 |
|---|---|---|---|
| A01 | 목표·작업 계약 | 요청 누락과 작업 범위 확장을 줄임 | 직접 구현 |
| A02 | 단계 계획과 재계획 | 큰 작업을 성격별로 나누고 실패 범위를 줄임 | 직접 구현 |
| A03 | 프로젝트 이해와 Context Pack | 반복 탐색과 불필요한 자료 전달을 줄임 | 직접 구현 + Codex·도구 연결 |
| A04 | 변경 영향·위험 분석 | 관련 파일·테스트·계약 누락을 줄임 | 직접 구현 + 도구 연결 |
| A05 | Codex 능력 확인과 단계별 배정 | 작업 성격에 맞는 모델·생각 깊이·방식을 선택 | 직접 구현 + Codex 연결 |
| A06 | Codex 실행 제어와 터미널 조작 | 여러 Codex 작업을 한 흐름으로 시작·중단·재개 | 직접 구현 + Codex 연결 |
| A07 | 상태·Checkpoint·이어하기·자체 복구 | 앱·대화가 끊겨도 작업을 잃지 않음 | 직접 구현 |
| A08 | 권한·승인·격리·비밀정보 보호 | 유료·외부·파괴 행동과 범위 이탈을 막음 | 직접 구현 + 설정 |
| A09 | Worktree·병렬 작업·병합 | 혼자서 여러 작업을 안전하게 동시에 진행 | 직접 구현 + Git 연결 |
| A10 | 작업·도구·검증·프로필 Registry | 프로젝트별 반복 절차를 재사용하고 하드코딩을 줄임 | 직접 구현 + 설정 |
| B01 | Diff·범위·주장·증거·Review Pack | AI 보고 대신 실제 변경과 검사만 보고 판단 | 직접 구현 |
| B02 | 테스트 신뢰성 검증 | 테스트 삭제·약화와 잘못된 회귀 증거를 막음 | 직접 구현 + 테스트 도구 연결 |
| B03 | 검증기 자기보호와 반례 Corpus | 검증 규칙 자체의 약화와 오탐 부패를 막음 | 직접 구현 |
| B04 | 계약·아키텍처·설정 검증 | API·Schema·모듈 경계의 조용한 파손을 찾음 | 공통 관제 + 프로젝트 도구 연결 |
| B05 | 보안·의존성·공급망 검증 | 혼자 놓치기 쉬운 secret·의존성·배포 위험을 줄임 | 공통 관제 + 전문 도구 연결 |
| B06 | 실패 재현·원인 격리·프로젝트 복구 | 추측 대신 재현 자료로 수정하고 재발을 막음 | 직접 구현 + 진단 도구 연결 |
| B07 | 문서·설정·개발 환경 검증 | 개인 PC와 기억에 숨은 개발 절차를 재현 가능하게 함 | 공통 관제 + 프로젝트 도구 연결 |
| B08 | 성능·자원·빌드 효율 검증 | 중요한 경로와 개발 피드백의 실제 회귀만 찾음 | 공통 관제 + 측정 도구 연결 |
| B09 | CI·릴리스·배포 준비 검증 | 검증한 변경과 배포할 산출물이 같은지 확인 | 공통 관제 + 외부 서비스·CLI 연결 |
| C01 | 개발 작업 프로필 | 전문 워크벤치를 공통 엔진 위의 재사용 절차로 제공 | 직접 구현 + 설정 |
| D01 | 여러 프로젝트·원격 저장소·인터넷 근거 | 프로젝트 간 순서와 원격 결과를 혼자 추적 | 직접 구현 + Codex·Git 연결 |
| D02 | 비용·실효성 평가와 배정 규칙 개선 | 자동화가 실제로 시간을 아끼는지 확인 | 직접 구현 |
| D03 | Windows 공개 배포와 제품 수명주기 | 설치·업데이트·복구를 개인 기억 없이 수행 | 직접 구현 + 배포 도구 연결 |


## 상세 기능 문서

- [핵심 관제 기능](core-control.md) — A01~A10
- [검증과 개발 보조 기능](validation.md) — B01~B09
- [3단계 공통 검증·품질 Gate 상세 설계](common-validation-gate.md) — B01~B07 공통 실행·Diagnostic·ratchet·Patch Gate
- [4단계 안전한 Patch·Refactor·codemod 엔진 계약](../contracts/safe-patch-and-codemod.md) — Recipe·selector·rewrite assurance·dry-run·single-project apply·복구
- [5단계 관리형 Symbol·상수·에러 코드 Registry 계약](../contracts/managed-symbol-registry.md) — 관리 분류·Git 정본·lifecycle·binding·consumer compatibility·M2/M4/M3 경계
- [6단계 계약 호환성·문서·설정·개발 환경 관리](../contracts/contract-compatibility-and-environment.md) — B04/B07 baseline·drift·doctor와 dependency/security input
- [7단계 실패 재현·보안·의존성 유지보수](../contracts/failure-security-and-dependency-maintenance.md) — B05/B06 failure identity·ReproductionPack·freshness·dependency PatchSet·Radar
- [8단계 Migration·성능·언어·플랫폼](../contracts/migration-performance-and-platform.md) — B04/B06/B08 version chain·restore·comparable measurement·equivalence
- [9단계 CrossRepo ChangeBundle](../contracts/cross-repo-change-bundle.md) — A09/D01 project별 worktree·merge·partial recovery·remote state·release handoff
- [10단계 CI·Release·평가·최종 제품 완성](../contracts/ci-release-evaluation-and-product-completion.md) — B09/D02/D03 검사 계층·artifact 승격·설치 수명주기·평가·최종 소유권 감사
- [개발 작업 Profile](profiles.md) — C01
- [11단계 Rust 코드 스타일 자동 교정 Profile](rust-code-style-auto-fix.md) — C01의 16번째 `rust_style_auto_fix`, stable rustfmt·allowlisted Clippy·isolated PatchSet·`personal_auto`
- [확장 운영 기능](operations.md) — D01~D03
- [구현 대상 선정 근거](../history/source-selection-record.md) — 외부 자료·레거시 대응

## 개념 설계 완료 조건

다음 조건을 모두 만족하면 이 문서의 기능이 제품 전체 흐름으로 연결됐다고 본다.

1. 자연어 목표가 A01 작업 계약과 A02 단계 계획으로 변환된다.
2. 각 단계가 A03~A05의 근거를 가지고 적절한 Codex 실행 방식에 배정된다.
3. 실행, 질문, 승인, 중단과 재개가 A06~A09에서 상태 손실 없이 이어진다.
4. 모든 결과가 B01 공통 관문과 필요한 B02~B09 검사를 통과하거나 미확인 이유를 남긴다.
5. 최종 16개 작업 유형이 별도 engine 복제가 아니라 C01 Profile로 같은 기반을 재사용한다. 최초 입력 자료가 15개였다는 역사적 사실은 제품 Profile 수와 구분한다.
6. 유료·외부·파괴적 행동은 정책에 따라 승인되고, 보통의 로컬 저위험 작업은 불필요한 질문 없이 진행된다.
7. 여러 프로젝트, 원격 Git, 평가와 Windows 배포가 D01~D03에서 같은 Task ID와 증거 사슬을 유지한다.
8. 전문 도구를 새로 흉내 내지 않고 기존 프로젝트 도구와 Codex 기능을 adapter로 연결한다.
9. release `ready`, 외부 effect `approved`, 실제 원격 `published`가 분리되고 final artifact digest가 source revision에 결합한다.
10. Rule·Check·Profile·Recipe 개선은 검증기 보호·실결함·오탐·재작업·시간 근거를 거쳐 review된 source change로만 반영한다.

이 문서는 구현 대상을 확정하는 개념 목록이다. 각 기능의 물리 Package와 문서 정본은 [최종 Repository·Package·문서 구조](../architecture/repository-layout.md)에서 확인한다. 세부 기술, 공개 계약, 규칙 임계값과 외부 도구 선택은 각 구현 단계 전에 최신 공식 자료와 실제 대상 프로젝트를 다시 조사해 결정한다.
