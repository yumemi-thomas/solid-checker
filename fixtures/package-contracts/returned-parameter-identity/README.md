# Generic parameter identity is independent of concrete value shape

The published implementation returns an exact input binding. Its generic
declaration intentionally leaves the concrete value shape unknown. The native
verifier must establish the parameter identity from authenticated implementation
facts, and reject a proposal claiming that `second` returns argument 0.

This is a native certification fixture, not a generated-main snapshot. The test
constructs the exact parameter-return proposal and authenticates all three
published files. Producer controls also reject reassignment, aliases, defaults,
rest/destructuring, async/generator wrapping, dynamic eval/arguments mutation
(including initializers), and duplicate parameter names. An unchanged shadowed
binding remains eligible. No concrete callable shape is inferred from identity.

ADR 0024 adds mutated: a numeric result must not discharge a forged unchanged
parameter relation after assignment. The native test refuses that claim even
though the declaration proves a numeric return type.
