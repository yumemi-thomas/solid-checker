# projection-draft-assign-shadowed

ADR 1053 negative (shadowed): `createProjection` is a local function that hands its callback the caller's object, so the
write may reach a root and `Number.isFinite` stays `standard-member-impostor`.
