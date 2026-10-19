# prop-head-get

**Claim (ADR 0221, G1).** In `props.value.text` the head `props.value` is a Get the runtime performs first; written directly in the exact children callback of a dialect control-flow primitive it is a strict read whenever the prop is backed reactively (ADR 0216 forwarding fixpoint): violations for namespace children, keyed `Show`, cross-file forwarding; uncertifiable for shadowed local `For` (ADR 0326), wrapped children, dormant defaults, merged/aliased/replaced heads, type-only/protocol/dynamic keys. Stubs byte-faithful to published rc.13.
