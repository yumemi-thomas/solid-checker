# construct-only-class

ADR 1022: a construct-only local class (one reference, a single `new` written after it in the same non-async function body,
outside JSX; no `this`, `super`, `extends`, decorator or computed key) places its constructor, its instance field
initializers and the method called directly on that `new` at that call. Code written directly there takes the role of the
function the class is written in: `MethodRead` (SC1001), `CtorWrite` (SC2001), `FieldAction` (SC2002) and `ConstBound`
(SC2001, a cast receiver) are violations; `InHandler` is clean. Members nothing places stay obligations
(`function-run-unplaced`): a method never called, two constructions, `this`, a run in a handler, a computed member, a
shadowing parameter, and a closure inside the method.
