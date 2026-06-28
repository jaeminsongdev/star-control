# 실행 결과 저장 규칙

Star-Control은 실행 결과를 repo 루트에 저장하지 않는다. 대상 프로젝트에 `.star-control/` 설정과 `.ai-runs/` 실행 결과를 둔다.

```text
대상 프로젝트/
  .star-control/
    project.yaml
    context.yaml
    approvals/
    rendered/
    cache/

  .ai-runs/
    J-0001/
      job.json
      effective-config.yaml
      route.json
      workspecs/
      provider-output/
      tool-output/
        star-sentinel/
      events.jsonl
      final-report.md
```

`tool-output/star-sentinel/`은 Star Sentinel의 공식 산출 경로다.
