# WorkSpec

## Metadata
- job_id: J-0001
- stage: implement
- role: worker-impl
- provider: codex
- project_root: <target-project-root>

## Goal
스톱워치 기능을 구현한다.

## Forbidden Actions
- 새 의존성 추가
- 파일 삭제
- git commit
- git push
- 테스트 약화

## Required Outputs
- report.json
- changed_files
- validation_result
- risks
