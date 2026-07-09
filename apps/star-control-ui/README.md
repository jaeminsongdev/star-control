# star-control-ui

Static browser UI app for the Star-Control control plane.

Open `index.html` in a browser, or serve this directory from `127.0.0.1` when the browser blocks `file://` module loading, and point API base at a running loopback `star-daemon api` instance. The app consumes daemon/API endpoints for daemon state, project jobs, job detail, timeline, release readiness, approve/cancel/resume actions, and provider connection management. It does not execute providers, implement Star Sentinel rules, or mutate StateStore files directly.

Provider connections are saved through `POST /provider-connections/instances` under the daemon config root as provider-instance JSON. The returned path can be reused by the existing CLI explicit path:

```text
star-control run --provider <instance-id> --provider-instance <saved-provider-instance-path>
```

Healthcheck and run-request buttons use policy-only API checks unless an existing daemon-supported fake/local-process queue request is accepted. Local OpenAI-compatible provider execution remains explicit CLI provider-instance execution in this slice. Cloud live execution and paid external calls remain blocked without explicit approval.

Example local API command:

```text
cargo run -p star-daemon -- api --config-root <config-dir> --schema-root specs/schemas --bind 127.0.0.1:8787 --max-requests 0 --json
```

Optional static server for browser verification:

```text
python -m http.server 18788 --bind 127.0.0.1
```

Static checks:

```text
node --test apps/star-control-ui/tests/app.test.mjs
```
