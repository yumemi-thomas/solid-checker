# member-value-alternatives

**Claim (ADR 0320).** A callee invoking a parameter member has its dispatch decided per call site, trusted only when every alternative resolves (conditional arms, factory returns), the literal member is the last own static-key property with no later spread/getter/computed or `__proto__` key/receiver-using function, a holding `const` is unexported and only member-read or passed to exact read-only callees, and the callee only so uses or forwards its parameter; otherwise `SC9012`. `member-value-copiers` pins the program-wide census.
