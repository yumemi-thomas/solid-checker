# release-triple-static-dynamic-async-rc3

A promise-valued `dynamic` source with `{ static: true }` on the rc.3 triple
gets no SC2007 `static-dynamic-async-source`, because rc.3's runtime has no
static form. Its bundles never read a second argument (no build before rc.9
contains `options?.static`): the source is the compute of the lazy memo
`dynamic` builds, which settles a Promise like any async source.

`@solidjs/web@2.0.0-rc.3` declares `dynamic(source)` with one parameter
(`types/index.d.ts:81`), so both two-argument calls are
`TS2554: Expected 1 arguments, but got 2`, against this fixture's stub
(`solid-js.d.ts`, byte-faithful for `dynamic` and the types it names) and
against the real rc.3 triple alike. That says the call has one argument too
many; it does not say the source is refused, and on rc.3 it is not.

Expected: **nothing**. `AsyncStatic`, `ResolvedStatic` and their one-argument
twin `AsyncDefault` are all the default form. No SC9014: the rc.3 triple is
audited.

`node_modules/` holds the three package manifests at `2.0.0-rc.3`. The rc.9
counterpart is `rc9-static-dynamic-async`.
