# fresh-written-object-factory-patched

ADR 1024 negative (factory form): as `fresh-written-object`, but the program replaces `JSON.parse`, a factory the form
trusts. Its sites are open writes again, so `Number.isFinite` stays `standard-member-impostor`.
