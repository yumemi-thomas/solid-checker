# An unknown runtime kind does not certify non-callability

ADR 0011's fixture contains both an actual callable and an actual object whose
`Object.assign` result is typed `any`, plus an independent ordinary JS `noop`.
The declarations intentionally state `any`; they provide no runtime-kind proof.
These are contract evidence tests, not additional TypeScript diagnostics.

Generation must preserve both uncertain exports as `shape: "unknown"` with
all call domains open. Native snapshot replay retains all three bindings and
the complete runtime module. The noop census and mandatory veto can certify
its creates closure independently. A forged plain-value claim for the hidden
callable must refuse; a closed creates claim for it must also refuse. There
is no trust in Object spelling or in the published declaration of either value.

The native tracer consumes these exact fixture bytes. No main snapshot is
added; the phase19 stable-main census is unaffected.
