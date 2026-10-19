# forwarded-prop-backing

**Claim (ADR 0216).** A prop a parent forwards (`<Inner value={props.value} />`) is exactly as reactive in the child as the parent's own prop: reactive when some call passes a reactive value, unresolved when the parent's prop is unresolved or the parent escapes enumeration, static otherwise (the `PlainInner` shape raised no `STRICT_READ_UNTRACKED` on rc.13). `merge` is copied from `closed-export-member-dispatch` (byte-faithful to `@solidjs/signals@2.0.0-rc.13`).
