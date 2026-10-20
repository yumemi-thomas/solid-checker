# release-triple-static-dynamic-async-rc3

On rc.3 a promise-valued `dynamic` source with `{ static: true }` gets no `SC2007`: rc.3 has no static form, so the source is the compute of the lazy memo `dynamic` builds, which settles a Promise like any async source (`dynamic(source)` at `types/index.d.ts:81`, so the two-argument calls are TS2554 against the byte-faithful stub and real rc.3). Expected: the `SC9014` release notice alone, nothing else; `diagnostics_process.rs` asserts that the notice is the only finding. rc.9 counterpart, where the same calls report: `rc9-static-dynamic-async`.
