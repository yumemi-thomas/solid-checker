# http-response-flush-nested

SC7005's Loading dominance is transitive (ADR 0596): a component rendered by
boundary content is boundary content, at any depth (`Deep` below `Inner`,
`Deepest` below `Mid` below `Top`, a self-rendering `Tree`). Each is an
obligation, as every SC7005 finding. A component rendered only by shell
content (`ShellDeep`) or by the boundary's fallback content (`FallbackDeep`)
is an obligation too, blocker `response-head-position-unproven` (ADR 0703):
no proof yet places it in shell content. The runtime fact is the SSR host's (`scripts/fuzz/ssr.mjs`,
template `m-nested`): the header is dropped on the pinned rc.17 server build.
