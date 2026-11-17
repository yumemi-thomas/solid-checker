# projection-draft-assign-rebound

ADR 1053 negative: the draft parameter is assigned a parameter inside the callback, so the write may reach a root and
`Number.isFinite` stays `standard-member-impostor`.
