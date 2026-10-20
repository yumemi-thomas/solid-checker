# helper-prop-read

**Claim (ADR 0222).** A component-local helper's own prop Get counts at each exact local call proven to run the helper's body (the call's role, the authored Get as origin, `PropsReactivityIndex` backing); unproven uses and tracked/sampled/event-only helpers add nothing. Body/apply/mixed-role/forwarded/literal-key/wrapped-root/passed/returned/stored/async-prefix/default/declaration shapes are SC1001 violations; root/alias/written/merged/dynamic-key shapes uncertifiable. Stubs byte-copied from `prop-head-get`.
