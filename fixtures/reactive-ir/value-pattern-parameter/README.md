# value-pattern-parameter

ADR 1104 part 2: a reactive value handed to a project function whose parameter is a destructuring pattern. The walk past the
argument's row (ADR 0860) stopped there, while Chrome rc.21 warns STRICT_READ_UNTRACKED at the read inside the callee: `get()` of
`({ get }) => get()` handed `{ get: a }`, and the pattern `{ a }` of a helper handed a props object or a store. Each read is now
uncertifiable (SC9012) at the call or the pattern; the row at the argument stays.
Twins: a plain function, a fresh literal or a plain object handed to the same shape stays silent; the accessor, props object or
store read through a plain parameter keeps its rows.
