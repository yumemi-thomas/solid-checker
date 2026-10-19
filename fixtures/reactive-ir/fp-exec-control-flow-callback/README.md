# fp-exec-control-flow-callback

**Claim.** Only a function written in a control-flow component's *children* is its render callback (`control_flow_execution_role`, `named_callback_roles`); a helper merely mentioned in `when`/`each`/`count`/`fallback`/attributes, or a function in an attribute (`each={items.filter(...)}`), belongs to that tracked attribute (no SC1001), and a read in a returned JSX fragment is tracked; body reads and render-callback bodies are violations. Stub copies `createSignal`, `createMemo` and `For`/`Repeat`/`Show`/`Switch`/`Match` from published rc.9.
