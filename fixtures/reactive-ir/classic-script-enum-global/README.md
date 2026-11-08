# classic-script-enum-global

**Claim (ADR 0906, review of ADR 0904).** A classic script's top-level non-const `enum Promise` declares a global `var Promise`, replacing the standard one for every module (TypeScript reports the clash; the patch census does not depend on that). No fact records an enum, so `classic_script_globals` reads `enum` and `import X =` declarations from the text: `Promise.resolve().then` in the module is no standard promise.
