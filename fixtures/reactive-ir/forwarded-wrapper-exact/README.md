# forwarded-wrapper-exact

**Claim (ADR 1179).** ADR 0200's forwarding (`callee_roles::forwarded_compute_slot`) proves that a literal handed to a wrapper runs in the
callback the wrapper gives a computation only when the call runs that wrapper exactly. The wrapper is resolved as ADR 0278 resolves a call
(`project_callee_at_parameter`, exact): a function declaration, a `const` or a `let` nothing writes bound to the function itself, or an
object's member the census (ADR 0299) resolves. That last includes a slot holding a function declaration by name (`{ hand: handDecl }`).
A `let` written later, a call's result, and a member an assignment replaces name no wrapper: no forwarding.

Each component reads a pending async memo in the literal (ADR 1155 r3: SC5001 where the leaf runs it).

| component | outcome | Chrome |
|---|---|---|
| `DeclApp`, `ConstApp`, `LetApp`, `MemberApp` (a declared method), `MemberDataApp` (a slot naming the declaration) | SC5001 a violation | PENDING_ASYNC_FORBIDDEN_SCOPE |
| `DropApp` (a `let` rewritten to a function that never calls it), `FactoryDropApp` (`drop(…)`'s result), `MemberDropApp` (a replaced method) | uncertifiable (main: SC5001 a violation, a false positive) | silent |
| `SwapApp`, `FactoryApp`, `MemberSwapApp` (the function the call runs calls the literal bare) | uncertifiable (main: SC5001 a violation, right by coincidence) | PENDING_ASYNC_UNTRACKED_READ |
| `PassApp` (`pass(f)` returns `f`) | uncertifiable (main: a violation by coincidence) | PENDING_ASYNC_FORBIDDEN_SCOPE |
| `SwapInApp` (a `let` rewritten to the leaf wrapper) | uncertifiable, as on main | PENDING_ASYNC_FORBIDDEN_SCOPE |
| `DataSwapApp` (a slot naming a rewritten `let`), `DataReplacedApp` (a replaced slot) | uncertifiable | silent |
| `DataArrowApp` (a slot naming a `const` arrow: the census reads a slot's name as a function declaration only) | uncertifiable | PENDING_ASYNC_FORBIDDEN_SCOPE |

No namespace variant: a namespace import makes every SC5001 here uncertifiable (ADR 0518's unknown load), so it could not tell the shapes
apart. Chrome: `fixtures/runtime-oracle/forwarded-wrapper-exact` (its harness imports the module whole, so the census leaves the member sites
uncertifiable there; the fixture holds the claim). Stubs: `forwarded-async-read`'s.
