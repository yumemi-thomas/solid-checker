# A value constant re-exported beside a callback-forwarding function stays closed

`index.js` is a barrel — `export * from "./inner.js"` — that republishes two
siblings declared in the same module:

- `subscribe(client, cb)` forwards its caller-supplied callback into an
  unresolved member dispatch (`client.getThing(cb)`). The callee resolves to no
  declaration, so the callback's execution is uncertifiable and the analyzer
  records a contract-generation obligation (`UnknownCallbackExecution`) whose
  subject is `subscribe`. The proposal's unresolved-claim census carries a
  `callbacks` domain entry for `subscribe`, and its summary stays `callable`.
- `PREFIX` is a plain string constant. It must certify as a `value` with no
  function effects: no `callbacks` claim, summary `shape: "plain"`.

The trap this pins: the obligation's owning function and the value constant are
published through the *same* wildcard re-export, so a naive attribution that
reaches from the obligation to every export sharing the barrel's runtime
identity would open `callbacks` on `PREFIX` too. A closed value export with
function effects is then refused outright by the operation-graph invariant
(`package contract value export .:PREFIX cannot have function effects`).

Here `subscribe` is a named export, so its obligation carries an exact compiler
symbol that resolves only to `subscribe`; a value constant never carries a
function's symbol, so `PREFIX` is never marked. This fixture is the
integration-level companion to the unit regression
`contract_generation_callback_attribution_tests::exact_function_symbol_excludes_value_siblings_that_share_runtime_identity`
in `rust/crates/solid-facts-backend/src/main.rs`.

This fixture pins the exact-symbol attribution path only. The superficially
related ecosystem-wrapper failures — `@tanstack/solid-db` (`IR`),
`@tanstack/solid-hotkeys` (`ALL_KEYS`), `@tanstack/solid-query`
(`dataTagErrorSymbol`), `@tanstack/solid-form` (`initialServerFormState`), and
`@tanstack/solid-query-persist-client` (`PERSISTER_KEY_PREFIX`) — do **not** run
through callback-obligation attribution at all. They are a *composition*
mechanism: a non-callable value constant re-exported from a dependency whose
proposal keeps that export's call-path domains open. Composing the proposal
projected those open domains onto the value export and manufactured function
effects. Fixed in `project_accepted_export`
(`rust/crates/solid-reactive-ir/src/contracts.rs`) and pinned by
`contract_document::tests::composing_a_value_export_with_open_call_path_never_manufactures_function_effects`;
see docs/precision-backlog.md. It cannot be a self-contained corpus fixture
because the open call-path on a value export only arises from composing a
cross-package proposal dependency, which single-package `generate` never does.
