# effect-creation-for-await

**Claim (ADR 1215).** rc.21 throws PRIMITIVE_IN_EFFECT_CALLBACK only while an effect's apply runs synchronously (`@solidjs/signals@2.0.0-rc.21`
`dist/dev.js:2772-2801`). A creation that every run reaches only past a suspension of its own async function runs on that continuation, a
microtask on an empty stack: SC6002's site ends in `effect-creation-after-await` (`execution_role::own_body_await_before`, beside ADR 1159's
unconditional `await`). The suspensions: the `for await` whose body holds the creation (each pass awaits the iterator's `next()` first), and an
`await` at statement level of a block whose rest holds it (`AstFacts::dominating_awaits`, never a `catch` or `finally` the operand's synchronous
throw reaches unsuspended).

| component | outcome | Chrome |
|---|---|---|
| `ForAwaitBody`, `BranchBlockAwait`, `FinallyOwnAwait` | silent (main: uncertifiable `effect-creation-unaccounted`) | quiet |
| `ForOfSync` (a synchronous `for…of`), `ForAwaitOperand` (the creation in the operand, before the first suspension) | SC6002 violation | PRIMITIVE_IN_EFFECT_CALLBACK |
| `FinallyAfterTry` (`try { await Promise.resolve(); } finally { … }`: nothing proves the operand cannot throw synchronously), `AsyncGeneratorBody` | uncertifiable, as before | quiet |

Chrome: `fixtures/runtime-oracle/effect-creation-for-await` (the silent and uncertifiable components) and `effect-creation-for-await-{sync,operand}`
(one each: the first throw halts the reactive system). The fixture mounts its components with `render` (an exported component an unseen importer may run is ADR 0967's `unread-package-may-run`); the oracle copies export them for the harness to mount. Stubs: `owner-call-effect-creation/variants/inline-handler`'s.
