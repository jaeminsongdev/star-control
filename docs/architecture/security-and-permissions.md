# 승인·권한·안전

## 기본 생각

승인 질문은 많을수록 안전한 것이 아니다. 너무 자주 묻으면 사용자가 내용을 읽지 않게 된다.

Star-Control은 모든 동작의 승인 여부를 설정할 수 있게 하고, 공개 배포용 안전 기본값과 개인용 자동 기본값을 분리한다.

## 기본 프로필

### safe_default

공개 배포의 기본값이다.

- 읽기와 일반 조사는 자동
- 승인된 계획 안의 파일 수정은 자동
- 파일 삭제와 대량 이동은 질문
- 새 도구와 프로그램 추가는 질문
- 시스템 설정 변경은 질문
- 원격 업로드, 검토 요청, 병합은 질문
- 외부 계정 변경은 질문
- 유료 동작은 질문

### personal_auto

사용자가 원하는 개인 기본값이다.

- 유료 동작만 반드시 질문
- 나머지는 승인된 계획과 설정 범위 안에서 자동
- 되돌릴 수 있는 기록과 검사 의무는 유지
- Codex 또는 관리자가 요구하는 승인은 그대로 유지

### custom

행동 종류별로 자동, 질문, 금지를 직접 설정한다.

## 판단할 행동 종류

- 파일 읽기
- 파일 수정
- 파일 삭제
- 파일 대량 이동
- 명령 실행
- 새 프로그램 또는 의존 항목 추가
- 인터넷 접근과 다운로드
- 컴퓨터 설정 변경
- 로컬 변경 기록 생성
- 원격 저장소 업로드
- 검토 요청 생성과 수정
- 병합
- 배포와 공개
- 외부 계정 수정
- 유료 서비스 사용

## 유료 동작

다음 중 하나면 유료 가능성이 있는 것으로 본다.

- 도구나 서비스가 유료임을 명시함
- 사용량에 따라 요금이 발생함
- 유료 계정 자원을 생성하거나 변경함
- 비용 여부를 확실히 알 수 없음

비용을 확실히 알 수 없으면 실행 전에 사용자에게 묻는다.

## 계획과 권한의 관계

승인이 없다는 것은 무엇이든 해도 된다는 뜻이 아니다.

- 활성 목표와 단계가 있어야 한다.
- 단계 목적과 관련된 동작이어야 한다.
- 사용자가 금지한 경로를 지켜야 한다.
- 실행 전후 기록을 남겨야 한다.
- 실패를 숨기거나 검사를 약화하면 안 된다.
- Star-Control은 Codex의 더 강한 제한을 약화할 수 없다.

## 외부 개발 도구 신뢰

ToolPackageManifest 등록과 매 실행 승인은 다른 경계다.

- TOML이 EXE를 가리킨다는 사실만으로 검증·권한·비용 판단을 건너뛰지 않는다.
- manifest, Schema, update policy, 허용 path와 executable identity를 한 묶음으로 trust한다.
- `safe_default`는 새 user package·path를 처음 쓸 때 확인하고, `personal_auto`는 사용자가 관리 root에 직접 저장한 valid user manifest를 등록 의도로 볼 수 있다.
- project manifest는 `personal_auto`에서도 사용자가 한 번 명시적으로 trust해야 하며 `pinned_hash`만 허용한다.
- user manifest의 `version_compatible`은 valid Authenticode subject와 probe·interface·product version 범위를 모두 통과한 EXE만 자동 반영한다.
- user manifest의 `follow_path`는 허용 path의 현재 EXE를 쓸 수 있지만 매 실행 identity·hash·version을 기록하며 path 범위와 permission은 넓히지 않는다.
- trust한 뒤의 반복 실행은 ToolDescriptor의 Permission ActionId와 현재 정책 Profile을 따른다.
- manifest, Schema, permission 분류, protocol 또는 허용 path가 바뀌면 기존 trust를 재사용하지 않는다. executable byte 변경은 선택한 update policy에 따라 거부하거나 호환 검증한다.
- describe가 정한 risk lane보다 낮은 MCP lane으로 호출하면 side effect 전에 거부한다.
- TOML이 자체적으로 `trusted=true`, `approval=auto`를 주장하는 field는 인정하지 않는다.

상세 형식은 [외부 Tool Registry 계약](../contracts/external-tool-registry.md)이 소유한다.

ToolPackageManifest의 read·write·network 표시는 임의 EXE의 실제 행동을 스스로 제한하지 못한다. Job Object는 timeout·resource·process tree를 관리하지만 보안 sandbox가 아니다. 일반 CLI의 `trusted_desktop`은 현재 사용자 권한으로 실행할 local code를 신뢰하는 결정이다. OS 수준 접근 제한은 materialized artifact만 다루는 호환 `appcontainer_adapter`에서만 주장한다. `restricted_token` profile은 실제 project path·network 제한을 과장할 수 있어 지원하지 않으며, Codex sandbox가 Controller child에도 자동 적용된다고 간주하지 않는다.

## 범위가 늘어났을 때

예상하지 못한 파일을 건드렸다는 이유만으로 즉시 중단하지 않는다.

1. 현재 단계에 꼭 필요한 변경인지 판단한다.
2. 안전하고 같은 성격의 작업이면 범위를 넓히고 이유를 기록한다.
3. 다른 모델, 다른 권한, 유료 동작, 큰 위험이 필요하면 새 단계로 분리한다.
4. 사용자 파일인지 판단할 수 없거나 안전하게 진행할 수 없으면 중단한다.

safe_default는 의심스러운 범위 확대를 더 자주 질문하고, personal_auto는 기록 후 진행을 우선한다.

## 작업 시작 전 기존 변경

- 현재 변경 상태를 기준선으로 저장한다.
- 사용자가 이미 바꾼 파일을 Star-Control 변경으로 오인하지 않는다.
- 같은 파일을 수정해야 하면 기존 내용을 보존한 상태에서 작업한다.
- 자동으로 되돌리거나 덮어쓰지 않는다.
- 충돌 가능성이 크면 새 작업 복사본을 사용하거나 중단한다.

## 비밀정보

- 비밀번호, 인증키, 개인정보 원문을 기록하거나 출력하지 않는다.
- 발견 위치와 위험만 남긴다.
- 외부 전송 전에 검사한다.
- 실행 로그와 증거에도 같은 가림 규칙을 적용한다.

## 금지 경로

프로젝트와 사용자 설정은 읽기 금지, 수정 금지, 외부 전송 금지 경로를 따로 지정할 수 있다.

## 공개 배포 원칙

- 새 사용자는 safe_default로 시작한다.
- personal_auto는 사용자가 의미를 확인한 뒤 직접 선택한다.
- 설치 과정에서 Plugin 검사와 권한 범위를 보여준다.
- 보호 기능이 꺼지면 상태 명령과 실행 보고서에 명확히 표시한다.
