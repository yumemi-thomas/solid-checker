# fresh-target-definer-shadowed

ADR 1036 negative (shadowed): `Object` is a local whose `defineProperties` defines on a parameter; the call is no
standard definer's, so `Number.isFinite` stays `standard-member-impostor`.
