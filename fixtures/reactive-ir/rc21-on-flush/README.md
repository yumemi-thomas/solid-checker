# `onFlush`

`onFlush(callback)` ends in `queueMicrotask(callback)` on every path, so the
callback never runs in the call, has no owner, and a read in it has no strict
window. Measured on the rc.21 dev client build (ADR 0622).
