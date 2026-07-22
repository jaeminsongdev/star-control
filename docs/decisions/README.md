# 결정 기록

이 폴더는 현재 설계의 중요한 선택과 변경 이유를 짧게 기록한다. 결정 기록은 제품·구조·계약 정본을 대신하지 않으며, 해당 정본 문서로 연결한다.

| ID | 상태 | 결정 | 정본 |
|---|---|---|---|
| ADR-0001 | 채택 | D0 최종 설계 기준과 RouteDecision 분리 | [결정 기록](ADR-0001-최종-설계-기준.md) |
| ADR-0002 | 채택 | 데이터 계약, Event·Snapshot, 설정 병합, 정책·작업 Profile 분리와 기계 정본 | [결정 기록](ADR-0002-데이터-계약과-설정-정본.md) |
| ADR-0003 | 대체됨 | 동적 MCP tool 공개와 process별 Registry snapshot 고정 | [과거 결정](ADR-0003-외부-도구-레지스트리와-MCP-Gateway.md) |
| ADR-0004 | 채택 | 고정 generic MCP surface와 Controller 소유 live Tool Registry | [결정 기록](ADR-0004-무재시작-고정-MCP와-Live-Tool-Registry.md) |
| ADR-0005 | 채택 | MCP v1 fixed surface·manifest·hash·Windows runtime·검증 gate 동결 | [결정 기록](ADR-0005-MCP-구현-계약-동결.md) |
| ADR-0006 | 채택 | Git 정본·local management DB·`.ai-runs` 분리, 단일 Writer, backend-neutral repository와 rebuild 경계 | [결정 기록](ADR-0006-공통-개발-관리와-로컬-관리-DB-경계.md) |
| ADR-0007 | 채택 | global+project 하이브리드 store, local-first ProjectId, 운영·scan·decision·patch·설치 기본 정책 | [결정 기록](ADR-0007-P0-하이브리드-저장소와-운영-정책.md) |
| ADR-0008 | 채택, x64·ARM64 release cross-build 통과 | `star-state` private `rusqlite` bundled backend와 기능 최소화; current ARM64 support tier는 ADR-0015가 소유 | [결정 기록](ADR-0008-P0-embedded-relational-backend.md) |
| ADR-0009 | 채택 | Git manifest 정본 Managed Registry, derived DB Index, M2→M4→M3 변경 경계와 cross-repo 적용 보류 | [결정 기록](ADR-0009-Git-정본-Managed-Registry와-Patch-Gate-경계.md) |
| ADR-0010 | 채택 | build-once artifact 승격, ready·approved·published 분리, release/evaluation 보호 Gate와 user data 보존 | [결정 기록](ADR-0010-Build-Once-승격과-Release-평가-Gate-경계.md) |
| ADR-0011 | 채택 | stable rustfmt, exact allowlisted Clippy·coverage, isolated PatchSet과 `personal_auto` exact policy approval 경계 | [결정 기록](ADR-0011-Stable-rustfmt-Allowlisted-Clippy-Personal-Auto-경계.md) |
| ADR-0012 | 채택, P-0026 수직 Slice 구현 | 선택형 current-user Inno Setup 설치, 설치 기록, 실제 경로 기반 Codex Plugin·MCP·Hook 렌더링과 데이터 보존 | [결정 기록](ADR-0012-선택형-Windows-설치와-Codex-Plugin-연동.md) |
| ADR-0013 | 채택, 구현 진행 중 | 고정 Bootstrap Bridge와 versioned Runtime Generation, candidate review·rollback 경계 | [결정 기록](ADR-0013-고정-Bootstrap-Bridge와-Runtime-Generation.md) |
| ADR-0014 | 채택, P-0039 구현 | 전용 one-shot Updater, 4 Runtime EXE와 Codex 전체 생명주기 | [결정 기록](ADR-0014-전용-Star-Updater와-Codex-생명주기.md) |
| ADR-0015 | 채택, P-0040 정책 | `v0.1.0` GitHub Releases, x64 Stable·ARM64 `native_unverified` Preview와 서명 차단 경계 | [결정 기록](ADR-0015-x64-Stable과-ARM64-Preview-출시-정책.md) |

새 결정은 기존 문서의 책임을 바꾸는 경우에만 추가한다. 단순 구현 세부사항과 조사 과정은 이 폴더에 기록하지 않는다.
