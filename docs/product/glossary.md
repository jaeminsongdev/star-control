# 용어

## 사용자 목표

사용자가 최종적으로 얻고 싶은 개발 결과다.

## 단계

같은 모델, 생각 깊이, 실행 방식, 권한, 검사 방법으로 처리할 수 있는 작업 묶음이다. 파일 하나나 함수 하나를 뜻하지 않는다.

## 배정

한 단계에 모델, 생각 깊이, 실행 방식, 권한, 검사, 비용 한도를 정하는 일이다.

## 모델 역할

Sol, Terra, Luna처럼 작업 난도와 성격에 맞춘 모델의 역할 구분이다.

## 생각 깊이

한 Codex 작업에 사용할 판단 깊이의 원시 설정이다. 실행 계약에는 `minimal`, `low`, `medium`, `high`, `xhigh` 중 실제 지원되는 값을 기록한다.

## Max

하나의 어려운 단계에 더 많은 판단 시간이나 강화된 단일 실행 경로를 요청하는 Star-Control 실행 방식이다. 생각 깊이 값이 아니다.

## Ultra

나눌 수 있는 큰 단계를 여러 Codex가 병렬로 처리하고 결과를 통합하는 Star-Control 실행 방식이다. 생각 깊이 값이 아니다.

## Plugin

Star-Control의 작업 규칙, MCP 연결, 실행 전후 검사, 설치 정보를 묶어 Codex에 설치하는 단위다.

## MCP

Codex가 Star-Control의 목표, 계획, 실행, 상태, 증거 기능을 호출하는 연결 통로다.

## 실행 전후 검사

사용자 입력이나 도구 실행 전후에 규칙을 확인하는 기능이다. 계획 없는 수정 차단과 결과 기록에 사용한다.

## Controller

목표와 단계 상태를 관리하고 Codex 작업을 조정하는 Windows 배경 프로그램이다.

## Codex App Server

Star-Control이 Codex 작업을 만들고 모델, 생각 깊이, 작업 폴더, 권한을 지정하며 진행 상태를 받는 공식 통로다.

## 필요한 자료 묶음

현재 단계에 필요한 파일, 규칙, 이전 결과만 모은 입력이다.

## 검사 계획

현재 변경이 맞는지 확인하기 위해 실제로 실행할 검사 목록이다.

## 증거 묶음

변경, 검사, 실패, 비용, 위험, 완료 결과를 모은 기록이다.

## 이어하기 기록

다음 Codex 작업이나 재개 작업이 반복 조사하지 않도록 남기는 짧은 상태 요약이다.

## 작업 복사본

병렬 작업이 서로 방해하지 않도록 Git worktree로 만든 별도 작업 공간이다.

## 병합 대기열

검사를 통과한 병렬 변경을 정해진 순서로 합치는 목록이다.

## safe_default

공개 사용자용 안전 기본 승인 설정이다.

## personal_auto

유료 동작만 사용자에게 묻고 나머지는 승인된 계획 안에서 자동 진행하는 개인 설정이다.

## 정책 Profile

`safe_default`, `personal_auto`처럼 승인, 비용, 허용 범위와 최소 검사를 정하는 설정 묶음이다.

## 작업 Profile

프로젝트 이해, 변경 계획, migration과 Rust style 자동 교정처럼 개발 작업 성격에 맞춰 단계, Context와 검사를 조합하는 최종 16개 유형이다. 권한을 넓히지는 않는다.

## CLI-only core

Codex나 다른 AI 없이 `star` CLI가 같은 application command, 계약, Gate와 evidence를 사용해 결정적 계획·검사·상태 조회·승인된 동작을 수행하는 기본 제품 경로다. Codex 연동은 이 core의 선택 소비자다.

## 검사 계층

`local_quick`, `target`, `full`, `release` 순으로 범위와 환경 보증을 넓히는 검사 단계다. 모두 같은 Task·source revision·tool/config/Profile identity를 사용하며 계층 사이 identity가 바뀌면 새 candidate다.

## Build once와 승격

final release artifact를 한 번 build·package하고 immutable digest로 봉인한 뒤 같은 byte를 검사하고 승격하는 원칙이다. 검사나 publish를 위해 다시 build하면 같은 candidate가 아니다.

## ready

공개 전 required release Gate와 evidence가 통과해 승인 가능한 상태다. 사용자 승인이나 원격 publish 성공을 뜻하지 않는다.

## approved

exact ReleaseManifest revision, artifact digest, channel, provider, destination과 만료에 대해 사용자가 release action을 허용한 상태다. 원격 결과를 뜻하지 않는다.

## published

provider receipt와 후속 RemoteStateSnapshot이 exact version·source/tag·artifact digest를 확인한 상태다. 요청 접수, timeout 또는 화면 표시만으로 만들지 않는다.

## EvaluationRun

Rule·Check·Profile·Recipe 또는 routing/policy의 baseline과 candidate를 같은 case·protocol에서 비교하는 versioned 증거다. CLI-only와 Codex-integrated context를 분리하고 실제 결함·오탐·flaky·suppression·재작업·실패·검증된 비용을 기록한다.

## Maintenance Radar

dependency·rule·recipe·Profile 등 유지보수 항목의 current risk, 마지막 평가, replacement, owner, deadline과 다음 검토를 정렬하는 derived view다. 평가 결과가 Catalog source를 직접 수정하지 않는다.

## MCP Gateway

Codex와 Star-Control Controller 사이에서 MCP 형식만 바꾸는 얇은 `star-mcp.exe`다. 고정된 검색·설명·위험 lane 도구만 가지며 TOML, 실제 개발 도구 이름과 실행 코드는 가지지 않는다.

## Tool Registry

어떤 EXE를 어떤 이름·입력·권한·출력 방식으로 연결할지 TOML로 모은 Controller 소유 live 목록이다. 검증된 변경은 MCP와 Codex를 재시작하지 않고 다음 호출부터 사용한다.

## Descriptor hash

Codex가 설명을 읽은 tool 계약과 Controller가 실행하려는 계약이 같은지 확인하는 SHA-256이다. 다르면 다시 설명을 조회하고 이전 입력을 추측 실행하지 않는다.

## Risk lane

실제 action을 읽기, 변경, 파괴적 변경과 로컬·외부 접속으로 나눈 고정 MCP 호출 통로다. MCP 표시를 위한 분류이며 실제 승인은 Controller가 다시 판단한다.

## Last-known-good

새 TOML·Schema·EXE 후보가 잘못됐을 때 유지하는 해당 package의 마지막 검증 정상본이다.

## Trusted desktop

현재 사용자 token과 Job Object로 실행하는 일반 외부 CLI profile이다. process 수명과 자원은 관리하지만 filesystem·network sandbox는 아니므로 EXE code trust가 필요하다.

## AppContainer adapter

Star-Control JSON-STDIO protocol을 구현하고 materialized input·brokered output만 사용하는 격리 adapter다. 일반 개발 CLI를 자동으로 AppContainer에 넣는 기능이 아니다.

## MCP contract version

고정 12개 MCP tool, annotation, hash, state machine과 wire 의미를 함께 versioning하는 값이다. 외부 action 추가는 이 version을 바꾸지 않는다.

## Adapter EXE

고유한 입력·출력을 가진 기존 개발 도구를 Star-Control의 공통 JSON 형식으로 바꾸는 작은 연결 프로그램이다.

## 레거시

이전 Star-Control의 로컬 읽기 전용 참고자료다. 새 제품의 실행 또는 문서 기준으로 사용하지 않는다.
