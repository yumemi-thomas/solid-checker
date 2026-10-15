# ADR 0299 review: namespace-base

The retained reproducer, analyzed on the default source route (which selects no dependency source here, so no installed package is analyzed from source), hands exported receivers to opaque code
through computed string namespace accesses. A second namespace goes through an
`export *` barrel. The original bodies are replaced at runtime: neither the
read at App nor the original cleanup is a proven violation. ba3d9085d reports
both; the fix leaves the exported cleanup's definition-site obligation.
The SC1001 read-summary path predates ADR 0299's new owner edges.
Callback also keeps an obligation: a refused receiver cannot prove that its
replacement invokes the caller's closure inline (or invokes it at all).

Solid signatures are copied verbatim from object-member-import's published
rc.13 declarations. opaque-mutate includes its full runtime and faithful
declaration. All application sources pass tsc against the real rc.13 typings.
