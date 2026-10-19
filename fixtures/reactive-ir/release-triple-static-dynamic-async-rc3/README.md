# release-triple-static-dynamic-async-rc3

On rc.3 a promise-valued `dynamic` source with `{ static: true }` gets no SC2007: rc.3 has no static form, so the source is the compute of the lazy memo `dynamic` builds, which settles a Promise like any async source (`dynamic(source)` at `types/index.d.ts:81`, so the two-argument calls are TS2554 against the byte-faithful stub and real rc.3). Expected: nothing, no SC9014. rc.9 counterpart: `rc9-static-dynamic-async`.
