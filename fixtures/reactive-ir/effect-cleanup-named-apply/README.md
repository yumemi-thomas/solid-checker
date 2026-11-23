# effect-cleanup-named-apply

**Claim (ADR 1200).** An effect's apply named by an unexported `const` whose one reference is the effect's apply argument (nothing writes it, no direct `eval`) is the inline apply of ADRs 1037, 1167 and 1183: no other code can call it, so its returned cleanup runs only from the effect. `NamedWrite` is clean (a write legal on every run of the cleanup), `NamedRerunRead` is an SC1001 violation (a reached click reruns the effect), `NamedDisposalRead` is silent (the compute tracks nothing); all uncertifiable on main. Uncertifiable as before: an apply also called elsewhere (`NamedCalledElsewhere`), exported (`NamedExported`), or handed to two effects (`NamedTwice`).

Chrome: `fixtures/runtime-oracle/effect-cleanup-named-apply`. Stubs: `effect-cleanup-write-role`'s. `tsc --noEmit` clean.
