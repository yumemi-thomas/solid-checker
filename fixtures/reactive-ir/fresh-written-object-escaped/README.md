# fresh-written-object-escaped

ADR 1024 negative (member path): as `fresh-written-object`, but `createMatcher` hands `match` to a call before writing
`match.params[…]`; the callee may rebind `params`. That write stays open, so `Number.isFinite` stays
`standard-member-impostor`.
