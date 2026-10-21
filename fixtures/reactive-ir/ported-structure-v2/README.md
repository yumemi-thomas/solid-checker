# ported-structure-v2

Pins the three rule arms ported to Solid 2.0: signal-backed `prefer-for` and `prefer-show`, plus the intrinsic
content-competition arm of `jsx-no-duplicate-props` (`innerHTML`, `innerText`, `textContent`, and children).
Negative controls keep a static map stored outside JSX, attribute conditionals, component content, a lone
`innerText`, and the removed 1.x DOM-alias folding outside their domains. The
rule-options file explicitly enables the two opt-in style preferences (default certification excludes them). The
`solid-js` stub selects the audited `2.0.0-rc.0` dialect and preserves only the real JSX declarations these proofs
require.
