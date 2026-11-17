# fresh-target-definer-open

ADR 1036 negative: `Object.defineProperties(target, d)` on a parameter may define any key of a root, so `Number.isFinite`
stays `standard-member-impostor` (`ReflectiveDefinition(*)`).
