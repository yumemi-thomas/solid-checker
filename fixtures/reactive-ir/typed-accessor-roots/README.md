# typed-accessor-roots

**Claim (ADR 0294).** A dialect type alias never supplies the reactive-read premise: `Accessor<T>` is `() => T`, which any plain function satisfies, so a value so typed is a source only by what it provably holds (`const` alias, closed literal member, exact factory tuple slot); a function literal it holds is decided by its own body; otherwise it is a typed candidate whose read is `SC9012`, never `SC1001`. Ambient stub with `Accessor` byte-faithful to `@solidjs/signals@2.0.0-rc.13` `dist/types/signals.d.ts:55`.
