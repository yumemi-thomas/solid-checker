# An owned edge does not hide another entry (ADR 0868)

A function the component body calls has an owned edge, and that edge is only one
way in. Assigned to `onclick`, pushed to an array, written to a member, kept by a
value-only argument (`keep.call(n)`), or held by an object slot, it is also run by a
click with no owner: rc.21 raises NO_OWNER_CLEANUP there. Those entries have no edge,
so the site is uncertifiable (`owner-callers-unenumerated`), not clean. `Row` returns
JSX but no tag renders it, so it is a layer, not a component. `Removed` is
uncertifiable too: nothing proves the receiver's `removeEventListener` drops the value.
A function every reference of which is a direct call or an inline run in the body
(`Twice`, `Inline`) stays proven owned.

R29: a whole argument of a primitive is placed only where the dialect gives that
argument an owner. `action(q)` keeps `q`, and the click steps it with no owner.

A closed layer is entered as its callers are: `viaHelper`, called once from a function
that escapes, is uncertifiable. `inRoot`, called under a root that function creates, is silent.
