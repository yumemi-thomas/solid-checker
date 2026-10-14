# Deferred effect apply

`defer: true` skips initial apply. A dependency-free compute cannot schedule
apply later: its reads and owner operations produce no finding. A compute
that reads a writable source on every completion supplies the later-run
premise. Unknown compute/defer values remain uncertifiable. Omitted options,
an absent defer key and literal false retain initial execution.

Controls cover `createRenderEffect`, namespace imports, transparent wrappers,
named callbacks (including one also supplied to an eager effect), `untrack`,
cross-file compute helpers, a shadowed source name, bundled handlers,
conditional reads and unknown/spread/computed options.
Ref callbacks created inside apply cover the same invocation premise for
directive creation: skipped applies stay silent, unknown applies cannot
publish a violation, and a literal false control retains the diagnostics.

The effect, signal value, untrack and cleanup signatures and supporting types
are copied from published `@solidjs/signals@2.0.0-rc.13` declarations, without
JSDoc. The unused signal overloads and Solid's hydration augmentation are
omitted; no case depends on them. `$REFRESH` is declared locally with the same
unique-symbol brand shape. JSX only supplies the button render witness.
All cases are also checked with the real installed rc.13 declarations.

The combined snapshot has 12 violations and 18 uncertifiable findings.
The latter include the existing exported-prop obligations and the global
SC9014 notice for the manifest-only dialect stub's missing companion packages.
No strict-read or owner finding remains inside a proven dependency-free
deferred apply. The original reproduction against the actual installation
has no findings in either source or contract mode.
