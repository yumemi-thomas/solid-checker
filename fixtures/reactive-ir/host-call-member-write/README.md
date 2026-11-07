# host-call-member-write

ADR 0874: `document.getElementById("x")!.title = v` writes `title`. The callee `document.getElementById` is only read, so it is no patch of
`Document.getElementById`, and the standard call keeps its resolution: the write in the `then` callback is on a fresh stack, and the
effect-free read in `forEach` is a violation as without the line.
