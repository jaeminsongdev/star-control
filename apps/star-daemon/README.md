# star-daemon

Local Star-Control daemon app entrypoint.

현재 app은 `packages/star-control-daemon` queue state를 실제 process surface로 여는 productization slice다. `status`, 테스트 가능한 `serve --max-ticks`, loopback-only `api` HTTP server를 제공하고 approve/cancel/resume HTTP control action을 audit log에 연결한다. `serve --max-ticks`는 queued `fake-default` job을 `ExecutionEngine`으로 실행하고 queue에서 제거한다. queue entry에 `provider_instance_paths`가 있으면 allowlisted local-process provider도 실행한다. provider instance path가 없는 non-fake provider와 cloud/live connector 계열은 scheduler result와 함께 `DISABLED`로 남겨 live call을 수행하지 않는다. remote exposure, cloud/live scheduler executor, Local/Cloud AI live connector는 아직 disabled 상태로 둔다.
