# A consumer of a returned memo accessor

ADR 0162: an accepted contract whose closed `returns` hands back a described callable reading an owned memo leaves nothing open and the consumer treats it as an accessor: `UntrackedDoubled` is `SC1001`, `TrackedDoubled` clean, `DoubledOpen` (open return) is `SC9005` and still `SC1001` (a partial claim keeps the positive fact it states). Ships `.solid-checker/authorize-contract.json`; manifest, `solid-js`, `@solidjs/signals` byte-identical to `package-described-callable-consumer`'s.
