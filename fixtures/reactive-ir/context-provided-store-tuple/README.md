# A context's `[store, { actions }]` literal (ADR 1147, arm 3)

Textbook `id-context`'s idiom: a provider component creates a store and hands `[cart, { add }] as const` to a default-less context; readers take it apart
(`const [cart] = useContext(Ctx)`, `const [, { add }] = useContext(Ctx)`). Arm 3 (`context-provided-store-tuple`,
`callback_return::provided_store_tuples`) proves, from ADR 1123's premises (the context an unwritten, unexported `const` made by the dialect's `createContext`
with no argument, one provider, every reference the provider's tag or an exact `useContext` an array pattern takes apart at once, no code built from a string:
ADR 1137's veto), that a reader's slot-0 name is the store itself and that an action's local is the function the literal holds, only called. So:

- the reader's store name is a store source (as a store a hook returns is, ADR 0628): its reads have store rows, and the root's census follows it;
- the census gives a reader only the names of the slot it walks (a hole holds nothing: `pattern_elements`, facts schema 62);
- a write in an action is placed by its readers' calls (the write-role walk's caller fold), so `add("apple")` in a handler is a handler's write.

Positive: `Shop` (the textbook's shape: the SC9012 rows at the value and the readers and SC2001 at `setCart` end).

Negative, each keeps its rows: `WholeApp` (a reader holds the tuple whole), `AliasApp` (an action's local aliased), `HeldApp` (slot 1 bound whole),
`KeptApp` (the value's `const` handed on), `TwiceApp` (a second provider), `SignalApp` (slot 0 a signal's accessor, no store), `QueuedApp` (an action's local aliased and handed to `queueMicrotask`), `HandedApp` and `OutApp` (the review's F1: an action called in a callback handed to a project helper that runs it in `createTrackedEffect`, or to a callee nothing reads; a package read as source is the same position), and `variants/eval` (the
positive's shape in a program with a direct `eval`).

Stub: `context-provider-children-owned`'s, plus the plain `createStore` form from the published rc.21 typings.

`LeakedApp` (the review's b): the store's setter is also handed to code nothing here reads. The arm still applies (the action's entries are its readers'
calls), and the setter's hand-off keeps its own rows; the reader's store name is the same store, so the root's census of that write covers the reader.
