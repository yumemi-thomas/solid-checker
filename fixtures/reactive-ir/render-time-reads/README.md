# render-time-reads

**Claim (ADR 0204).** Reads that run while a component renders are proven `strict-read-untracked` violations: `outer()` (direct call chain), `withDefault()` (omitted argument runs the default), `<Render label={() => count()} />` (`Render` calls `props.label()` in its body), `useBoth()`; none for `withDefault(2)`, `lazyDefault()`, `deferred()`, `useFinisher()`, `<RenderLive label={...} />` (JSX tracks, ADR 0218); `<Render ... {...rest} />` is uncertifiable (a spread could replace the prop). Stubs from `feedback-tiers`.
