# A consumer of returns of argument containers

ADR 0115/0116: an accepted contract closing `returns` over the caller's argument, a fresh array of the caller's arguments, or an invocation's result leaves nothing open (open `returns` raise `SC9005`); a closed union with no shared leaf projects as no reactive return (hides, never invents). `count`/`asArray`, `greeting`/`accessWith` clean; `countOpen`, `greetingOpen` `SC9005`. Declarations match `@solid-primitives/utils@7.0.0-next.4`; manifest byte-identical to `package-plain-return-consumer`'s; ships `.solid-checker/authorize-contract.json`.
