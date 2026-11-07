# impostor-host-intact

ADR 0874's negative: an object literal with an `addEventListener`, and a write under an unknown key to a `const` bound to a literal, reach
no host object, so `document`, `window` and `document.body` keep their proof while no writer stands. `box[0].other = 1` writes a member the
census does not hold. A shadowing `document` parameter is no host global.
