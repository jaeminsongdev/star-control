# Star-Control 작업 지침

## 기본 원칙

- 응답과 새 문서는 한국어를 우선한다.
- 설정 이름, 명령어, 코드 이름, 파일명은 필요한 경우 원문을 유지한다.
- 정본 문서와 실제 구현·실행 증거의 상태를 구분하고, 설계된 것을 구현 완료로 표현하지 않는다.
- 구 Star-Control 구현은 Git history와 `docs/history/`에만 보존하며 로컬 `legacy/` checkout을 다시 만들거나 현재 정본으로 인용하지 않는다.
- local AI, 다른 AI provider, OpenAI API 직접 호출, browser UI, HTTP control UI와 자체 예약 실행을 다시 넣지 않는다.

## 현재 단계와 범위

- 명시된 P-ID, 단계와 수직 Slice 안에서만 설계·구현·검증한다.
- 승인된 Slice 밖의 제품 코드를 무제한으로 작성하거나 인접 과제를 함께 처리하지 않는다.
- 여러 파일 또는 여러 단계 작업은 `PLANS.md`에 현재 목표, 범위, 검증 상태와 열린 위험만 bounded snapshot으로 남긴다.
- 범위 밖 결함은 근거와 후속 묶음만 기록하고 현재 변경에 섞지 않는다.

## 사용자와 AI의 역할

- 사용자는 제품 동작, UX, 우선순위, 수용 기준과 되돌리기 어려운 관제 결정을 소유한다.
- AI는 명시된 Slice의 기술 설계, 구현, 테스트, 리팩터링과 정본 문서 동기화를 수행한다.
- 기계적 주석을 대량 삽입하지 않고, 설계·manifest·readiness를 실제 실행 완료로 표현하지 않는다.

## 도구 선택

- Star-Control action은 `star_tool_search`로 찾고 `star_tool_describe`로 현재 Schema, 위험 lane, `descriptor_hash`와 readiness를 확인한다.
- action이 `ready`이고 현재 작업 범위와 호출 조건이 맞을 때만 반환된 `required_call_tool`로 실행한다.
- 검색 결과가 없거나 action이 `unavailable`, `untrusted`, `incompatible`, `degraded`이면 native Git·프로젝트 도구를 사용한다.
- package 또는 manifest의 `ready` 상태를 내부 action의 실행 가능 상태로 해석하지 않는다.
- `approval_required`, `question_required`와 장기 operation 시작을 작업 완료로 간주하지 않는다.

## 작업트리 보존

- 기존 dirty 파일과 미추적 파일은 사용자 작업으로 취급하며 성격을 확인하기 전 수정·이동하지 않는다.
- `git reset`, `git clean`, 임의 `restore`, 파일 삭제로 작업트리를 정리하지 않는다.
- 현재 Slice와 무관한 변경을 숨기거나 같은 commit에 섞지 않는다.
- `target/`을 정리하기 전에는 반드시 `git worktree list`를 확인하며, 현재 정책에서는 `target/` 정리를 금지한다.
- linked worktree와 로컬 생성물을 별도 프로젝트나 폐기 가능한 cache로 단정하지 않는다.

## 생성 상태 경계

- `$CODEX_HOME/plugins/cache`, Codex App runtime DB·state, `%APPDATA%\Star-Control`, `%LOCALAPPDATA%\Star-Control`을 직접 수정하지 않는다.
- `dist/`와 `target/`의 설치·빌드 산출물을 직접 고쳐 source 변경처럼 사용하지 않는다.
- 생성 상태를 바꿔야 하면 source, template 또는 installer를 수정하고 검증된 생성·repair 절차로 다시 만든다.
- 로컬 `--check/`와 루트 `manifest.json`은 생성 주체와 용도가 확인된 상태로 보존하며 임의 삭제·commit하지 않는다.

## 승인 경계

- package·dependency 설치, system setting·PATH 변경, 파일 삭제·대량 이동은 별도 사용자 승인을 받는다.
- push, PR, publish, deploy, 외부 account 변경, 유료 기능 사용과 그 밖의 외부 효과는 별도 사용자 승인을 받는다.
- 명시된 작업 범위가 검증됐고 사용자 변경을 숨기거나 섞지 않는 경우 local commit은 별도 사람 승인 없이 가능하다.
- 작업 지시가 commit을 금지하면 해당 제한을 우선하며, P-0003 정책 정리에서는 commit하지 않는다.

## 검증과 증거

- 기본 TARGET 진입점은 `pwsh ./scripts/validate.ps1 -Profile target`이며 CI와 Star-Control도 같은 추적 스크립트를 호출한다.
- 코드 작업의 기본 검증은 `TARGET`이며 영향받은 package·unit과 필요한 smoke·lint만 실행한다.
- 순수 정책·문서 변경은 `QUICK`으로 포맷, 링크, 구조, 범위와 diff를 확인한다.
- 공개 API, Schema, lockfile, 공통 코어, release 변경은 `FULL`로 영향 범위를 검증한다.
- 파일을 수정할 때마다 반복하지 않고 논리적 작업 묶음이 끝날 때 해당 Gate를 한 번 실행한다.
- 검증을 통과시키기 위해 테스트·정책·증거를 약화하거나 실패를 숨기지 않는다.
- 완료 보고에는 변경 파일, 실행 명령, 종료 코드, 소요시간, 실패 요약과 남은 위험만 간결하게 남긴다.
- 공식 Codex 기능을 설명할 때는 최신 OpenAI 공식 문서를 근거로 한다.
