---
status: accepted
---

# Colocate Type Facts with the checker

The Type Facts Go producer and Rust process/session client will return to the `solid-checker` repository while retaining their versioned process seam. This lets one reviewed change add a semantic fact, update its producer and client, exercise checker consumers, and prove corpus impact without coordinating an external pull request and pin move; the rejected alternative is to keep a separately versioned repository whose only production consumer is this checker. The migration must import the exact pinned history, prove protocol and finding parity before adding facts, and preserve Type Facts as a deep module rather than exposing TypeScript-Go internals to Reactive IR.
