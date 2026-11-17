# handed-root-patch-definer-made-param

ADR 1045 negative (parameter target): as `handed-root-patch-definer-made`, but `linkProps` defines its parameter, which
may be `Number`. `ReflectiveDefinition(*)` holds, so `Number.isFinite` stays `standard-member-impostor`.
