# inline-carrier-named

**Claim (ADR 1214, slice B).** A function bound to a name whose only use is being handed whole to a standard-library call's inline-callback slot (`[1].forEach(f)`: the call resolves to the standard declaration, the census neither withholds it nor finds its member written by a package, and `argument_behavior` says the slot runs inline) runs on that call's stack, as a literal written there does (`write_roles::layer_run`, `sole_inline_carrier_of_named_function`). ADR 1090's climb then places what the function holds at the call.

Clean: `TrackedEach` (the carrier in a tracked JSX expression). Violation: `BodyEach` (the carrier in the component body; main: uncertifiable). Kept uncertifiable: `AlsoKept` (the function is also pushed into an array), `OwnForEach` (the member variant: a project object's `forEach`), `Reassigned` (the shadowed variant: a `let` reassigned before the carrier runs it).
