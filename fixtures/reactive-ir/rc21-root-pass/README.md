# SC5003 needs a first run inside `render()` (ADR 0661)

The boundary warning is enabled only during a synchronous `render()` call, so a pending
source is not enough. A read unconditional in a function `render` mounts, through any
synchronous holder, is proven. A read a condition may delay is uncertifiable, blocker
`root-mount-pass-unproven`.
