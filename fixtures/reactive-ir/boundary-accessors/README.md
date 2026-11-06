# boundary-accessors

**Claim (ADR 0780).** `createErrorBoundary` and `createLoadingBoundary` from `solid-js/internal` return a memo's accessor, a read like a memo's: a violation in a body, silent in a tracked scope.
