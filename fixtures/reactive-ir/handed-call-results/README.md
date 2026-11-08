# handed-call-results

**Claim (ADR 0689).** A value a caller supplied stays handed however it travels: a hook's result used on the spot, called again, iterated, chosen by a conditional, held by an operand, a `let`, a `new`, a filled container or an IIFE. Each read is SC9012, none a violation, and a hook that only hands back closed data over a primitive parameter stays certified.
