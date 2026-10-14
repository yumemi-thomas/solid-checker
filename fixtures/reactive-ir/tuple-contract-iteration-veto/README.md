# Contract tuple iteration veto

ADR 0282: an authored contract describes the factory result, not the slots
that a replaced iterator binds. The call to `start` is SC9012 uncertifiable;
its returned-member read graph must not retain SC1001.

This copies the exact contract, manifest, declarations and authorization of
`package-effectful-member-consumer`, whose intact `StartInBody` is the negative
control retaining a proven SC1001. The fixture issuer describes a modeled
package, not an audited published artifact. The consumer and mutator are also
checked with real Solid rc.13 typings and a matching local runtime factory;
the real raf contract reproducer is checked separately.
