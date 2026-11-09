# Inferred host: an application manifest that does not parse

`inferred-host-alias-congruent` (the healthy twin) with `package.json` cut
short. Discovery cannot read it: one `SC9005` `host-input-unreadable`, never
certified (ADR 0915). `variants/enclosed` reads it as its enclosing
manifest: the same. Dangling links and denied access are built at run time
(`host_inputs_unobservable_process.rs`).
