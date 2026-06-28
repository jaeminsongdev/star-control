# Provider Model

Star-Control provider는 제품명이 아니라 실행 능력과 연결 방식으로 모델링한다.

## 계층

- Provider Manifest: 종류와 기본 실행 형태.
- Provider Instance: 사용자 또는 프로젝트별 구체 설정.
- Transport: CLI, HTTP, process, manual.
- Adapter: WorkSpec과 provider 입출력 변환.
- Capability Profile: router가 provider를 선택할 때 쓰는 능력 선언.
