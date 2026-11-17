# fresh-written-object-shadowed

ADR 1024 negative (shadowed): as `fresh-written-object`, but `Object` in `extractSearchParams` is a local whose `create`
hands back a parameter. That write stays open, so `Number.isFinite` stays `standard-member-impostor`.
