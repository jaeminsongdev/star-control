# Source Absorption Audit

## Summary

- v3 files: 208
- v4 files: 29
- source map rows: 237
- missing mapped targets: 0
- content absorption failures: 0

## Result

PASS

## Failures

- None.

## Coverage Rules

- Every file under both source folders must have one row in `docs/decisions/source-absorption-map.md`.
- Every mapped target must exist, except split schema targets represented by `provider-*.schema.json`.
- Directly absorbed text files must contain the normalized source text.
- Provider manifests and feature matrices must also preserve the original v3 YAML under each builtin provider `docs/` directory.
