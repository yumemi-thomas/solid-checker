# compound-components-escapes

Each file removes one condition of ADR 0342's census, so a member stored in the object is not proven a component and its body read stays uncertifiable (every finding here). Deleting the one disqualifier in a file turns its findings into violations.

- `Passed.tsx`: the holder is also handed to a function (`consume(Parts)`).
- `Alias.tsx`: a member is read as a value (`const P = Parts.Panel`, `const { Handle: H } = Parts`) and a member is called plainly (`Parts.Row()`).
- `Called.tsx`: a member invoked as a plain call beside its tag.
- `Dynamic.tsx`: a computed key, a spread and a getter each leave the member set or a value open.
- `Reassigned.tsx`: a `let` holder, and a `const` holder whose member is overwritten.
- `Shadowed.tsx`: the module's holder is rendered nowhere (the tag names a parameter that shadows it), and a local `Object` is not the standard library's `Object.assign`.
- `Target.tsx`: `Object.assign` mutates its target function, so `TabsRoot.List` reaches every member without naming the holder.
- `HelperCalled.tsx`: the helper-read twin of `compound-components/Helper.tsx` with one member also called plainly (`Split.Panel()`); the holder fails the census, so the read in `Handle` is uncertifiable as well.
- `Targets.tsx`, `Imported.tsx`, `Base.ts`: the `Object.assign` target is a class with a static accessor, an object with accessors, a Proxy, or an imported class; `Object.assign` writes with [[Set]], so the target can receive a member and call it plainly or answer another value.
- `Tdz.tsx`: a holder inside a function (`renderEarly`) that runs before its values (`const Sub`) or its `Object.assign` target (`const Root`) are initialized throws, so no member renders; a holder counts only at module top level with its values and target declared earlier there.
- `Intrinsic.tsx`: `Object.assign(Root, { name: … })` and `{ length: … }` throw (a function's `name` and `length` are non-writable); `prototype`, `arguments` and `caller` are refused too.
- `variants/eval`: a direct `eval` anywhere in the project can replace a member, so no holder is a census. A variant, since the refusal is project-wide.
- `variants/patched-assign`: the project replaces `Object.assign`, so an `Object.assign` holder is withheld project-wide (a write to `Object.assign` or a global `Object`, or `Object.defineProperty(Object, …)`). A local `Object` that shadows the global is `Shadowed.tsx`.
