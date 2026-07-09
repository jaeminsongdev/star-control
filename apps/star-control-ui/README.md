# star-control-ui

Static browser UI app for the Star-Control control plane.

Open `index.html` in a browser, or serve this directory from `127.0.0.1` when the browser blocks `file://` module loading, and point API base at a running loopback `star-daemon api` instance. The app consumes daemon/API endpoints for daemon state, project jobs, job detail, timeline, release readiness, and approve/cancel/resume actions. It does not execute providers, implement Star Sentinel rules, or mutate StateStore files directly. Local OpenAI-compatible provider execution is available through explicit CLI provider-instance runs; daemon scheduler Local/Cloud AI live connectors remain disabled in this app surface.

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
