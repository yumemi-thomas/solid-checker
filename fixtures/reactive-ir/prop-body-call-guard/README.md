# prop-body-call-guard

**Claim (ADR 0204, hardened in ADR 0326).** A function passed as a component prop runs during render, and its reads are a proven `SC1001`, only when the component's own body calls that prop directly and nothing can have replaced it before the call (`component_invokes_prop_in_body`: exact member tag, props never written/redeclared, no `arguments`/`eval`, call not in a class); aliased, deleted, `this`-replaced, `arguments`-reached, loop-head-written, redeclared, class-field and swapped-tag shapes are uncertifiable (ADR 0328).
