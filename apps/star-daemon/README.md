# star-daemon

Local Star-Control daemon app entrypoint.

현재 app은 `packages/star-control-daemon` queue state를 실제 process surface로 여는 productization slice다. `status`, 테스트 가능한 `serve --max-ticks`, loopback-only `api` HTTP server를 제공하고 approve/cancel/resume HTTP control action을 audit log에 연결한다. `api` server는 static browser UI가 loopback origin에서 접근할 수 있도록 loopback CORS/preflight만 허용한다.

`api`는 provider connection 관리 endpoint도 제공한다. `GET /provider-connections`는 builtin provider manifest와 daemon config root에 저장된 provider instance를 조회하고, `POST /provider-connections/instances`는 schema/policy 검증을 통과한 instance를 `<config-root>/provider-instances/<id>.json`에 저장한다. `validate`, `select`, `healthcheck`, `run-request`는 credential raw value 출력/저장과 mock success 없이 policy 결과를 반환한다.

`serve --max-ticks`는 queued `fake-default` job을 `ExecutionEngine`으로 실행하고 queue에서 제거한다. queue entry에 `provider_instance_paths`가 있으면 allowlisted local-process provider도 실행한다. provider instance path가 없는 non-fake provider와 cloud/live connector 계열은 scheduler result와 함께 `DISABLED`로 남겨 live call을 수행하지 않는다. Local OpenAI-compatible 실행은 저장된 provider-instance path를 CLI explicit run에서 재사용한다. remote exposure, cloud/live scheduler executor, daemon scheduler Local/Cloud AI live connector는 아직 disabled 상태로 둔다.
