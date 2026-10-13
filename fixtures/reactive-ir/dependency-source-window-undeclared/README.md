# dependency-source-window-undeclared

**Claim (ADR 0295).** `--runtime-target browser` alone declares no Window realm. This is `dependency-source-window-realm` without `"realm": "window"` in `.solid-checker/runtime.json`; every other file is byte-identical (coverage's `IDENTICAL_SOURCES`). No `typeof window` or `typeof document` guard is decided, every trap keeps its server branch, and nothing is reported: `WindowQuery`, `ClientQuery` and `DocumentQuery`, violations under the realm, are silent here.
