# affects-target-untouched (ADR 1150)

`affects(target)` in rc.21 (`@solidjs/signals` 2.0.0-rc.21, `dist/dev.js:10018-10050`, `dist/prod/affects.js:56-69`) reads `target?.[$TARGET]`
(the store half), then `target?.[$REFRESH]`, the node `accessor()` stores on every Solid accessor (`dist/dev.js:3237-3240`), registers a mark
on that node and schedules a flush that re-derives the readers of its verdict (`isPending`). It never calls `target` and never hands it on. So
an accessor whose only use is `affects`'s first argument has no read the value-use walk leaves open (`Dialect::argument_is_untouched_target`,
as for `refresh`, ADR 1089), where nothing can run code on that call. It can: the `$TARGET` read is on a key no accessor owns, so it walks
`Function.prototype` and `Object.prototype`, and a hook there, or a prototype swapped for the accessor's own, runs project code with the
accessor as its receiver, in the scope the call is written in. The proof therefore asks the shared standard-member premise (ADR 1129) for
`Function.$TARGET` and `Object.$TARGET`: nothing in the program may write a prototype by a route, by a key no fact decides, or through a
Proxy, `Reflect`, `Object.assign` or code built from a string, and no loaded package or unread module may.

- `Direct`, `Namespace`: no SC9012 at the accessor (the namespace member resolves to the same primitive).
- `Wrapper`: a project function hands the accessor to `affects`: the walk follows the parameter and ends there, proved.
- `Shadowed`: a local named `affects` is no Solid primitive: still open.
- `AlsoHanded`: the accessor also goes to `keep`, which nothing here reads: still open.
- `InBody`: `affects` in a component body is proved too: it has no owned-scope guard, and Chrome is quiet (`refresh` is the one that writes there).
- `variants/prototype-hook`: `Object.defineProperty(Function.prototype, $TARGET, …)`: the row stays (SC9012 `value-in-carrier`).
- `variants/set-prototype`: `Object.setPrototypeOf(m, hooks)` with a getter keyed `$TARGET`: the row stays.
- `variants/key-write`: `(Function.prototype as any)[key] = …` under a key nothing decides: the row stays.

`refresh` reads `$REFRESH` only, an own property of the accessor, so it asks no premise; a hook installed on that own property by code that
obtained the symbol (`Object.getOwnPropertySymbols`, an import of `$REFRESH`) is not asked either (open-work).
