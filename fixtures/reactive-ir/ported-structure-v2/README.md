# ported-structure-v2

Pins the three rule arms ported to Solid 2.0: `prefer-for`, `prefer-show`, and
the intrinsic content-competition arm of `jsx-no-duplicate-props`. Negative
controls keep non-rendered maps, attribute conditionals, component content,
and the removed 1.x DOM-alias folding outside their domains.

The `solid-js` package stub selects the audited `2.0.0-rc.0` dialect. Its JSX
surface preserves only the real declarations these proofs require.
