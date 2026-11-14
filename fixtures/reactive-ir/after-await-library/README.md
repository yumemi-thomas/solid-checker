# after-await-library

**Claim (batch 11, slice 9B follow-up; ADR 0976).** A call into code outside the project, written after an async function's own suspension, is a site of SC1002's accounting, and it ends in no row only where the typed fact `library-runs-nothing` holds. A site no ending holds gets an uncertifiable row of SC1002, blocker `read-after-await-unaccounted`: never a violation, never a silence. The reviews of #233 found that the arm that skipped every such call was a false clean: Chrome rc.21 reports UNTRACKED_READ_AFTER_AWAIT under a callback the library runs here, a setter's updater, a synchronous dispatch to a listener registered earlier, a stored generator or an iterator literal stepped by a call, a `Map` subclass's method, a constructor (the oracle case `after-await-library`).

The fact, from exact resolution alone (never a name, a type or a package's name):

| clause | what it asks |
|---|---|
| the program leaves every built-in alone | no assignment writes a standard global or a member of a standard prototype or constructor, no reflective definer is spelled, no reflective mutator is called, nothing writes through a `prototype`, no code is evaluated from a string (`after-await-library-patched` is the twin that fails this) |
| the callee resolves exactly | the compiler's selected declaration is the standard library's and no subclass, patch, impostor or assertion census withholds it; `eval` and the `Function` constructor never qualify |
| it delivers to no listener on this stack | `delivers_synchronously` (`dispatchEvent`, `click`, `focus`, `reportError`…), by the declaration's qualified name |
| its receiver is the library's own value | none, a primitive proven from its own expression, or an unshadowed ECMAScript global (`lib.es*.d.ts`) |
| no argument carries project code onto this stack | a nullish literal, a proven primitive, an exact data literal, or a function handed to a slot that runs later (a string there is code, not inert) |

A setter is inert on the same arguments when the signal it sets (the exact `createSignal` call it was destructured from) has no function-valued `equals`: absent or `false`. An action is never inert, and a package's call has no exact fact, so it is a site (the owner ruled FULL, ADR 0976).

`Outside` holds the calls that end in no row (the process test `a_library_call_that_runs_no_project_code_is_no_site`; the mutation test drops `library-runs-nothing` and gets exactly their rows). `Kept` holds the same calls with one thing the proof lacks (a value no fact proves, a function-valued `equals`, options no fact shows, a spread, a receiver that is only a library type, an argumentless call: no resolved-call fact), and each keeps its row. `after-await-library-patched` is `Outside` in a program that writes `String.prototype.concat` (and evaluates the string form of a timer): every call keeps its row. `node_modules/solid-js/package.json` selects the dialect; `solid-js.d.ts` declares what the program calls (its `Setter` is byte-faithful to `@solidjs/signals`).
