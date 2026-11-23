# forwarded-wrapper-exact

**Claim (ADR 1179, 1182).** ADR 0200's forwarding (`callee_roles::forwarded_compute_slot`) proves that a literal handed to a wrapper runs in the
callback the wrapper gives a computation only when the call runs that wrapper exactly. The wrapper is resolved as ADR 0278 resolves a call
(`project_callee_at_parameter`, exact): a function declaration, a `const` or a `let` nothing writes bound to the function itself, or an
object's member the census (ADR 0299) resolves. That last includes a slot holding a function declaration by name (`{ hand: handDecl }`).
A `let` written later, a call's result, and a member an assignment replaces name the wrapper only where ADR 1182 proves the one value
the name holds at the call (`forwarded-wrapper-values` holds the refused shapes); otherwise no forwarding. A slot naming a `const` arrow
is followed when the arrow is declared at module level before the literal and no module-level call runs between them (ADR 0299's census).

Each component reads a pending async memo in the literal (ADR 1155 r3: SC5001 where the leaf runs it).

| component | outcome | Chrome |
|---|---|---|
| `DeclApp`, `ConstApp`, `LetApp`, `MemberApp` (a declared method), `MemberDataApp` (a slot naming the declaration), `DataArrowApp` (a slot naming a module-level `const` arrow declared before the literal, ADR 1182) | SC5001 a violation | PENDING_ASYNC_FORBIDDEN_SCOPE |
| `SwapApp` (the `let`'s last write runs the literal bare), `FactoryApp` (`ignore(…)` returns a function that runs it bare), `MemberSwapApp` (the replaced method runs it bare) | SC5001 a violation (ADR 1182; 1179: uncertifiable) | PENDING_ASYNC_UNTRACKED_READ |
| `PassApp` (`pass(f)` returns `f`), `SwapInApp` (the `let`'s last write is the leaf wrapper) | SC5001 a violation (ADR 1182) | PENDING_ASYNC_FORBIDDEN_SCOPE |
| `DropApp`, `FactoryDropApp`, `MemberDropApp` (the function the call runs never calls the literal; main a63e1302e: SC5001 a violation, a false positive) | uncertifiable | silent |
| `DataSwapApp` (a slot naming a rewritten `let`), `DataReplacedApp` (a replaced slot) | uncertifiable | silent |

No namespace variant: a namespace import makes every SC5001 here uncertifiable (ADR 0518's unknown load), so it could not tell the shapes
apart. Chrome: `fixtures/runtime-oracle/forwarded-wrapper-exact` (its harness imports the module whole, so the census leaves the member sites
uncertifiable there; the fixture holds the claim). Stubs: `forwarded-async-read`'s.
