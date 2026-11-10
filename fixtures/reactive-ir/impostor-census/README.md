# impostor-census

ADR 0840: a function at `then` on the program's object makes a promise-typed `then` unproven: a write in its callback is open
(`Unproven`). A receiver the library made keeps the proof (`Fresh`, `Wrapped`, `Held`: a `const` used only as a receiver); one
handed on (`Escaped`, `Object.assign` puts the impostor on it) or rebound (`Rebound`) does not. `forEach` keeps it (`OtherMember`).
