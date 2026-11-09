# SC2003: where a delete through a wrapped operand is reported

ADR 0922. `delete (id(store.r) as T).b` runs as `delete id(store.r).b`, and
Chrome's frame is the call `id` (a link, or the statement, elsewhere). Each row
(uncertifiable; `store-delete` holds a violation) starts at the chain's root
through `()`, `as`, `satisfies` and `!`: a cast call, link and root, their
plain twins, `satisfies` and `!`, in a handler and a body. Silent: a plain
object, the store's own setter. A cast assignment keeps the member's start.
