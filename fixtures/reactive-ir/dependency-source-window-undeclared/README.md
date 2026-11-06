# dependency-source-window-undeclared

**Claim (ADR 0295).** `--runtime-target browser` alone declares no Window realm. This is
`dependency-source-window-realm` without `"realm": "window"` in `.solid-checker/runtime.json`; every other file is
byte-identical (coverage's `IDENTICAL_SOURCES`). No `typeof window`/`typeof document` guard is decided, every trap
keeps its server branch, and no violation is reported: every read is SC9012 uncertifiable, never silent (ADR 0822).
