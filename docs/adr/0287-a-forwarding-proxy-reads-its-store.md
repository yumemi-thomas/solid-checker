# ADR 0287: A forwarding proxy reads its store

- Status: accepted and implemented (2026-10-13).
- Owners:
  - the handler proof: `solid_facts::ast::proxy_get_forward_target`
    (`solid-facts/src/ast/proxy_get_forward.rs`), with the return walk
    `statements_return_cover` beside the ADR 0222 completion cover;
  - the returned source: `source_discovery::proxied_target` and
    `returned_source_through`, extending ADR 0222's `project_returned_source`;
  - the cache input: `indexes::returns_source_root` (was
    `returns_bare_identifier`).
- Found as a known loss of dependency source mode (ADR 0284): with
  `--dependency-source @tanstack/solid-query`, spotify-desk-thing loses the two
  `prefer-show` findings that contract mode reports.

## Context

`@tanstack/solid-query` 6.0.0-rc.0 returns its query result this way:

```js
function useBaseQuery(options, Observer, queryClient) {
  const [state, setState] = createStore(_stripFnsForSSR(observerResult));
  // …
  return new Proxy(state, {
    get(target, prop, receiver) {
      if (typeof prop === "symbol") return Reflect.get(target, prop, receiver);
      if (isServer2) {
        const resolved = queryResource();
        if (prop in resolved) return Reflect.get(resolved, prop);
      }
      if (errorPassthroughProps.has(prop)) return Reflect.get(target, prop, receiver);
      if (state.isError && !state.isFetching && shouldThrowError(…)) throw state.error;
      return Reflect.get(target, prop, receiver);
    }
  });
}
function useQuery(options, queryClient) {
  return useBaseQuery(createMemo(() => options()), QueryObserver, queryClient);
}
```

ADR 0222 proves that a hook returns the store it created only when every
return is the bare root. Here the root is wrapped in a proxy, and `useQuery`
returns another function's result, so `query.data` had no source and every
caller's read was silent. `interproc::reactive_proxy_return` admits a proxy
only when its handler creates a reactive source, which this one does not.

## Decision

1. **A forwarding proxy over the root is the root.** A return of
   `new Proxy(root, handler)` counts as a return of `root` when:
   - the callee is the standard library's `Proxy` constructor (Type Facts
     `ProxyConstructor.construct`), and `Proxy` is the unbound global;
   - it has exactly those two arguments, no spread, and `root` is an
     identifier;
   - `handler` is an object literal written in place with one property: a
     non-computed `get` (identifier or string key) whose value is a
     non-async, non-generator function or arrow written in place. So there is
     no `set`, `has`, `ownKeys`, `getOwnPropertyDescriptor`, `defineProperty`,
     `deleteProperty`, `apply` or other trap, no spread and no accessor;
   - the trap has two or three plain parameters, with no default and no rest,
     none written or redeclared;
   - every normal completion of the trap returns
     `Reflect.get(target, prop)` or `Reflect.get(target, prop, receiver)`,
     where `Reflect` is the unbound global and the arguments are the trap's
     own parameters. A throw is allowed. Any other returned value, a
     fallthrough (which returns `undefined`), or control flow the ADR 0222
     completion walk does not model (`try`, loops, `switch`, labels) refuses;
   - the trap names `target` only as the first argument of those returns, and
     holds no `this`, `super`, `arguments` or `eval`, so nothing can reach the
     handler or rewrite a parameter;
   - the root's value is a store (`store-path`). A proxy over a signal
     accessor refuses.

   Every other ADR 0222 and ADR 0281 rule is unchanged. The proxy's target
   argument is the one root reference exempt from the escape check, because
   it reaches only the proven trap. Every other reference in the trap, such
   as `state.isError`, is judged as before.

   Why this is the store: a proxy with only a `get` trap forwards every other
   operation to its target. Its `get` returns what the store's own `get` trap
   returns for the same key, after running other code first. A string key
   read through the store's trap is tracked exactly as a direct read
   (`@solidjs/signals` rc.13 `traps.get`); the outer receiver matters only for
   `$PROXY` and accessor or inherited properties.

2. **Host folds decide branches only through the text.** A branch counts as
   dead only when the ADR 0166 fold removed it from the analyzed source. For
   a selected dependency (ADR 0284), that is an `if` on a host constant such
   as `@solidjs/web`'s `isServer`, folded under the declared host. The trap
   proof reads the folded text and gives no condition any meaning of its own.
   solid-query's `var isServer2 = typeof window === "undefined"` is no host
   constant (ADR 0166 lists `typeof window` among the forms it does not fold).
   So its server branch, which returns `Reflect.get(resolved, prop)`, stays
   live and the proof refuses.

3. **A same-file forward returns what its target returns.** A function every
   return of which is `g(…)` returns `g`'s source when:
   - the call is a plain straight-line call;
   - `g` resolves exactly to a function in the same file;
   - `g`'s own proof (this one, at most four forwards deep) succeeds;
   - all such returns reach the same creating call;
   - the completion cover shows every normal completion returns one of them.

   A forward mixed with a root return, a forward to another file, or two
   forwards to different sources refuse. Each call of the forwarding function
   runs that one call, so the caller still gets a fresh source per call.

4. **The cache binds the new syntax.** The cross-file proof digest and
   `interproc::call_role_syntax` bind every file that returns a bare
   identifier (ADR 0222) or a `new` of a callee spelled `Proxy`. A same-file
   forward proves nothing unless its file holds one of these, so it is bound
   too. A cross-file forward is refused rather than widening the digest to
   every file that returns a call.

Every refusal is silent: the value has no source, as before.

## Consequences

- In project code, a hook returning a forwarding proxy over its store is a
  store source at each call, and a same-file wrapper hook forwards it.
- **The solid-query loss is not recovered.** The proof refuses
  `useBaseQuery`, because its server branch is guarded by `typeof window`,
  not by a host constant. Recovering it needs a separate premise: that a
  declared `browser` host defines `window` (a worker is declared `worker`).
  That premise is not taken here. It would change host folding for every
  selected dependency and every contract emission, not only this proof.
- Not covered:
  - a handler held in a variable or built elsewhere;
  - a `get` that returns `target[prop]` or forwards another receiver;
  - a forward across files;
  - a proxy whose target is a store member (`new Proxy(state.items, …)`).

## Evidence

- **Unit tests** `solid_facts::ast::proxy_get_forward`: five accepted shapes
  (solid-query's trap with its server branch folded, a two-parameter trap, a
  concise arrow, a string key, a throw arm) and 42 refusals.
- **Fixture** `reactive-ir/returned-store-proxy`: 4 violations
  (`ForwardingProxy`, `TwoParameterProxy`, `ArrowProxy`, `Forwarded`) and 14
  silent refusals. `tsc --noEmit` is clean.
- **Fixture** `reactive-ir/dependency-source-proxy-store`: an `isServer`
  branch folded under the browser host is excluded (violation), the same
  branch under `typeof window` is not (silent), and under a node host the
  folded case is silent too.
- **Session test** `incremental_result_cache_follows_a_proxy_trap_and_forward`:
  editing the trap or the forward in the hook file changes the caller's
  retained result. Without the `Proxy` clause in `returns_source_root`, the
  retained program differs from the fresh one.
- **Coverage:** no existing snapshot moved (1,885 findings before the new
  fixtures, 1,890 with them).
- **rc.13 corpus**, release binary, browser host: unchanged in both modes.
  Base mode stays at 562 violations and 3,731 uncertifiable; dependency mode
  (the eleven ADR 0284 packages) stays at 559 and 3,556. Both violation diffs
  against the baselines are empty.
- **What a host constant would recover:** in an APFS clone of
  spotify-desk-thing, solid-query's `var isServer2 = typeof window ===
  "undefined"` was replaced by `import { isServer as isServer2 } from
  "@solidjs/web"` (rc.13, `isServer = false` in every browser target). With
  `--dependency-source @tanstack/solid-query`, violations went from 6 to 8:
  exactly the two contract-mode `prefer-show` findings
  (`PlayerControls.tsx:133:5`, `PlayerControlIcon.tsx:26:5`). Nothing else
  moved. So the forwarding and proxy steps hold on the real package, and the
  `typeof window` guard is the only remaining step.
