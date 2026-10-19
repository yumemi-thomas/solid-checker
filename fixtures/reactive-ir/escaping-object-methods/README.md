# escaping-object-methods

**Claim (ADR 0320, review round 6).** An object literal's shorthand method is a named helper whose reads are claimed where it is called only while every use of the literal is a resolved member call (call receiver, `const` with only such references, or argument of an exact callee that only reads members); otherwise (unknown code, enumeration, returns, storage) it is a stored callback and its reads are uncertifiable (`HandedToUnknown`, `HeldThenHanded`, `Enumerated`); `DispatchedOnly` is the violation control. Scaffolding from `invocation-flow` (private).
