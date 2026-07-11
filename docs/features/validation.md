# 검증과 개발 보조 기능

상위 범위와 공통 선정 기준은 [구현 대상 기능](README.md)에서 확인한다.


## B01. 실제 변경·범위·주장·증거 검증

Codex의 설명을 그대로 믿지 않고 저장소의 실제 상태와 실행 증거를 기준으로 완료 여부를 판단한다.

- 작업 계약의 허용 범위와 실제 Git diff 비교
- 보고된 변경 파일과 실제 add, modify, delete, rename 비교
- 요청과 무관한 변경, 빠진 필수 변경, 생성 파일 직접 수정 탐지
- 필수 검사 명령의 실행 여부, revision, 종료 코드와 결과 확인
- "고쳤다", "검사했다", "호환된다" 같은 완료 주장과 근거 연결
- 근거가 없거나 오래됐거나 다른 revision의 결과이면 미확인으로 표시
- 진단을 규칙, 심각도, 확신도, 위치, 근거, fingerprint, 조치로 정규화
- 결과를 `AUTO_PASS`, `HUMAN_REVIEW`, `BLOCK`으로 구분
- 변경 요약, 위험, 계약·의존성·테스트 변화, 미확인 사항, 질문을 Review Pack으로 묶음
- 실패 이유와 필요한 수정만 담은 재작업 지시 생성
- 판단과 근거의 출처를 추가 전용 기록으로 보존

## B02. 테스트 신뢰성 검증

테스트가 실행됐다는 사실뿐 아니라 이번 변경을 실제로 증명하는지 확인한다.

- 프로젝트 테스트 목록과 변경에 관련된 테스트 연결
- 테스트 파일 삭제, 사례 삭제, assertion·expected value 약화 탐지
- skip, ignore, only, timeout·retry 증가와 대규모 snapshot 갱신 표시
- 버그 수정은 수정 전 실패와 수정 후 성공을 재현하는 회귀 증거 요구
- AI가 만든 테스트가 구현을 그대로 복사하거나 같은 잘못된 가정을 공유하는지 검토
- 실패한 seed, 입력, 명령, 환경과 결과를 재실행 가능하게 보존
- 변경 성격과 위험에 맞는 단위·통합·계약·end-to-end 검사 선택
- coverage 수치만으로 통과시키지 않고 중요한 경로와 실패 조건을 함께 확인
- 프로젝트에 필요할 때 property, invariant, differential, metamorphic, fuzz, sanitizer, mutation 검사를 외부 도구로 연결

## B03. 검증기 보호와 회귀 Corpus

검사를 통과시키기 위해 검사 자체를 약하게 바꾸는 일을 별도로 막는다.

- validator, policy, test harness와 CI 검사 경로를 보호 대상으로 등록
- 규칙 삭제, 심각도 하향, allowlist 확대, 필수 명령 제거와 우회 조건 탐지
- 검증기 변경에 별도 승인과 독립 검토 적용
- 새 규칙마다 정상, 실패, 경계 사례 fixture와 기대 진단 요구
- 실제 결함과 공격적 우회 사례를 회귀 Corpus로 축적
- 진단 억제에는 이유, 대상 fingerprint, 만료 시점과 승인자 기록
- 기존 부채는 baseline으로 고정하고 새 악화를 막는 ratchet 적용
- 규칙별 실행 시간, 실제 발견, 거짓 경고, 놓친 결함과 흔들림 측정
- 가능한 판단은 결정적 도구를 우선하고 Codex 평가는 제한된 보조 근거로 사용

## B04. 계약·구조·설정·마이그레이션 검증

프로젝트가 외부나 내부에서 지켜야 할 약속을 등록하고 변경의 파급을 확인한다.

- 공개 API, CLI, 설정, Schema, 직렬화, 파일 형식, 오류 코드와 DB migration 계약 등록
- 기준 계약과 현재 결과 비교, 선언과 실제 구현의 어긋남 표시
- component, layer, 허용·금지 의존, cycle과 공개 경계 규칙 선언
- 구조 위반, 경계 침범, 순환 의존과 의도치 않은 공개 표면 확대 탐지
- generated source와 원본의 drift, 생성 파일 직접 편집 탐지
- 계약·Schema·migration 변경 시 영향 대상, 호환성, 이행 계획과 승인 요구
- 확인할 수 없는 언어 의미나 동적 경로는 확정하지 않고 미확인으로 표시
- compiler, LSP, Schema validator, contract test와 migration tool 결과를 adapter로 수집
- 데이터·설정·DB 이동은 명시적 version, 단계 사슬, 사본 또는 원자적 교체, rehearsal, invariant, 재개와 복구 증거를 요구

Star-Control이 자체 parser, type checker, DB engine이나 범용 정적 분석기를 만드는 것은 이 기능에 포함하지 않는다.

## B05. 보안·의존성·공급망 검증

혼자 개발할 때 놓치기 쉬우면서 사고 비용이 큰 변경을 공통 관문에서 확인한다.

- source, config, 문서, log와 결과물의 secret·token·개인정보 후보 탐지 및 가림
- manifest·lockfile diff와 새 의존성의 목적, 출처, version, license와 위험 확인
- 취약점, license, SAST 결과를 프로젝트 도구에서 수집하고 중복 진단 통합
- auth, session, token, permission, crypto와 위험 API 변경 표시
- GitHub workflow 권한, 외부 action 고정 여부와 실행 조건 검토
- 배포 대상의 file list, digest, manifest와 package dry-run 확인
- 공개 배포가 있는 프로젝트만 SBOM, provenance, 서명과 검증 절차 연결
- 진단 출처와 갱신 시점 기록, 예외는 이유·범위·만료 시점과 함께 관리

Star-Control은 자체 취약점 DB, 보안 scanner, package registry 또는 공개키 기반 시설을 운영하지 않는다.

## B06. 실패 분석·재현·대상 프로젝트 복구

실패를 다시 만들 수 있는 자료와 수정 후 재발하지 않았다는 증거를 남긴다.

- compile, test, runtime와 운영 실패를 공통 형식으로 정리
- 연쇄 오류 중 첫 원인 후보와 동일 실패 fingerprint 식별
- revision, 환경, 명령, 입력, seed, stdout·stderr와 관련 artifact 묶음
- 최소 재현 절차와 재현 가능 여부 검사
- rerun, 입력 축소, Git bisect와 기존 debugger·trace 도구를 adapter로 연결
- 알려진 실패와 임시 회피책에 근거와 만료 조건 기록
- 수정 전 실패와 수정 후 성공, 관련 회귀 검사 연결
- rollback, roll-forward, restore 순서와 사전 rehearsal 증거
- 민감한 dump·log의 가림, 접근 범위와 보존 기간 관리

여기서는 작업 대상 프로젝트의 실패를 다룬다. Star-Control 자신의 중단 복구는 A07이 담당한다. 자체 debugger, dump analyzer나 tracing backend는 만들지 않는다.

## B07. 문서·설정·개발 환경 일치 검증

새 컴퓨터나 깨끗한 환경에서도 같은 작업을 재현할 수 있도록 코드 밖의 개발 계약을 확인한다.

- README, 운영 문서, 설정 예시와 정본 문서를 Documentation Registry에 등록
- 문서의 명령, code snippet, 링크, anchor와 config example 실행·존재 검사
- CLI·Schema·생성 문서와 실제 동작의 drift 탐지
- config key, 기본값, 필수 환경 변수, secret, local override 경계 확인
- toolchain, package manager, lockfile와 프로젝트 task 명령 발견
- 처음 설치 절차, project doctor와 누락 도구 진단
- line ending, encoding, 대소문자, 경로 길이와 Windows·CI 차이 표시
- clean-room 설치와 환경 fingerprint를 통한 재현성 확인
- 필요한 프로젝트만 reproducible build 여부를 별도 검사

Star-Control이 package manager, container runtime 또는 언어 version manager를 대신 만들지는 않는다.

## B08. 성능·자원·빌드 효율 검증

프로젝트가 중요하다고 선언한 경로만 안정된 조건에서 비교한다.

- 사용자 체감 경로와 개발자 build 경로의 시간·크기·memory 예산 등록
- workload, benchmark 명령, 입력 자료와 환경 fingerprint 보존
- 기준값과 반복 측정으로 noise와 의미 있는 악화 구분
- clean, incremental, cache hit·miss build 결과 분리
- 기존 profiler와 build 분석 도구 결과를 변경 파일·단계와 연결
- 성능 회귀의 후보 변경을 찾는 bisect 절차 연결
- 최적화 뒤 correctness, 유지보수 비용과 자원 trade-off 재검사

모든 작업에 강제하지 않는다. 프로젝트가 중요 경로 또는 반복 병목을 선언했을 때만 활성화하며 자체 profiler, benchmark engine이나 build cache를 만들지 않는다.

## B09. CI·Release·배포 준비 검증

로컬에서 검증한 대상을 같은 식별자로 CI와 배포 단계까지 추적한다.

- 같은 Task ID, revision, 도구 version과 검사 Profile을 로컬·CI에서 공유
- PR, main, release 상황별 검증 강도와 승인 정책
- 깨끗한 환경에서 build·test·package 증거 생성
- version, changelog, package metadata, 포함 파일과 dry-run 확인
- source revision과 불변 artifact digest 연결, 한 번 build한 artifact 승격
- 배포 전 검사, 동시 배포 제한, smoke, 관찰 시간과 rollback 조건
- 데이터·API 호환성과 migration 순서 관문
- publish·deploy·원격 변경·유료 행동의 명시적 승인
- GitHub Actions, package registry와 cloud CLI 결과를 adapter로 수집

Star-Control은 자체 CI/CD 실행 플랫폼이나 배포 서비스를 만들지 않는다.
