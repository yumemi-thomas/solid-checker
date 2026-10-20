# Dependency source object members (ADR 0299)

Claim: the runtime redirect of a named object import selects its exact own function, not its `.d.ts` receiver or
composite callee entity. `MissingOwner`'s `packageTicker` is attributed to the project call after await;
`packageRead`'s reactive read reaches `Reads`; `Owned` stays clean. `dep-veto`'s wildcard write in another file vetoes
its own object but neither `dep-object`'s members nor the project's `localTicker` (ADR 0284 decision 8). Solid
signatures are the same published-rc.13 claim-bearing stub as `dependency-source-owner`; tsc is clean.
