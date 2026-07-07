# star-control-ui

Static browser UI app for the Star-Control control plane.

Open `index.html` in a browser and point API base at a running loopback `star-daemon api` instance. The app consumes daemon/API endpoints for daemon state, project jobs, job detail, timeline, release readiness, and approve/cancel/resume actions. It does not execute providers, implement Star Sentinel rules, mutate StateStore files directly, or connect Local/Cloud AI live connectors.

Example local API command:

```text
cargo run -p star-daemon -- api --config-root <config-dir> --schema-root specs/schemas --bind 127.0.0.1:8787 --max-requests 0 --json
```

Static checks:

```text
node --test apps/star-control-ui/tests/app.test.mjs
```
