# fact-mode-parity

ADR 1135. `patchpkg` names `solid-js` (a peer dependency, and its entry imports it), so a run with a declared host
(`.solid-checker/runtime.json`, the Solid Vite plugin installed) reads it as source by ADR 0297's default and sees its write of
`Array.prototype[key]` under a key built at run time: the standard `[4242].at(0)` in `App.tsx` is then not proven standard
(SC9012 `standard-member-impostor`). On main dba9d3cee a run with `--feedback-facts` (the fuzz judge's mode) did not take that
default, read no package, resolved the call and reported none of those rows. Both modes now read the same program;
`fact_mode_parity_process.rs` runs them and fails on any difference in findings, and on a feedback model of a package file.
`node_modules/patchpkg` is written for this fixture, not published bytes; the other stubs are those of
`action-in-owned-scope-unread-packages`.
