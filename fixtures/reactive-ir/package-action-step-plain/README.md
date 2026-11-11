# package-action-step-plain

**Claim (ADR 0961), the negative of `package-action-step`.** The dependency `fz-act` returns the generator function it is handed (`runPlain(cb) { return cb; }`) and makes no action, and the project names none: a generator is a plain function a handler steps, `flush` is legal there, and nothing is reported (a program that names no `action` has no site of the generator obligation).
