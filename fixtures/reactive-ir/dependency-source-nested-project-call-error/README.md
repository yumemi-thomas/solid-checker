# Nested project call error (ADR 0368)

`tsc` reports TS2349 at `api.get()(count)`; the outer call's own fact says so. Silent.
