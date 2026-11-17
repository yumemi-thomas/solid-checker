# closed-literal-dispatch

**Claim (ADR 1023).** A member call on a closed `const` object literal runs the method written there, whatever type an assertion gives the receiver: `const t = { then(f) { f(); } }; (t as unknown as Promise<void>).then(cb)` runs `cb` in place. The object-member index (ADR 0299) resolves the member, and ADR 1014's and ADR 1009's literal checks hold, so the call is no standard call the impostor census withholds.

`App.tsx`: six proved violations (SC2001 five times, SC1001 once). Each is uncertifiable on main (`function-run-unplaced`). They are a method, a method under `as any`, an arrow member, a read through `forEach`, a method forwarding its argument to a helper with a named callback (the wrapper variant), and a method that also keeps its argument (the run in place is one run). `Legal.tsx`: two clean twins, which hand the callback to `queueMicrotask` (ADR 0781's `cast-late-foreach`). Chrome rc.21: oracle case `closed-literal-dispatch`.

Variants, each its own project, so one refused holder does not withdraw another's proof:
- `shadowed`: a component's `t` shadows a module-level one. The inner call is proved; the module-level receiver is not followed (uncertifiable). A `t` shadowed in a nested block stays uncertifiable at both calls (not followed yet).
- `late-read`: a read handed to a microtask is no violation and keeps its own obligation (`callee-callback-timing`).
- Refused, uncertifiable as on main: `assigned` (`Object.assign(t, …)`), `escaped` (the literal handed to a function), `getter`, `spread-after`, `unnamed-after`, `written-after` (`t.then = …`), `other-member-written` (`t.init = …`), `this-handed-on`, `not-const` (a `let`), `namespace` (the literal read as `L.t` through a module namespace), `alias` (`const q = t as unknown as Promise<void>`, not followed yet).
