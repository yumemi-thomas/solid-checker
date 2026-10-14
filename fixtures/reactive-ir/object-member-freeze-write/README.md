# A written Object.freeze is not the literal initializer (ADR 0299)

Claim: the wrapper needs the standard library's exact runtime freeze member,
including the same visible member-write veto as selected object members.
Object.assign replaces freeze while TypeScript retains its published signature.
Neither the original owner operation nor the original reactive read executes.
No violation may be manufactured from the object passed to the old signature.
Solid's claim-bearing declarations are dependency-source-owner's published
2.0.0-rc.13 signatures; this project is tsc-clean.
