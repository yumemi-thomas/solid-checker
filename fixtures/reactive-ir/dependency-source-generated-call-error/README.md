# Generated-only call error (ADR 0368)

`dep-api`'s source augments `API` after `App.tsx`, so only the dependency-source program has TS2349 at `run(count)`: SC9012, not silence.
