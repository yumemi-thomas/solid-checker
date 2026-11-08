# props-destructure-pending

**Claim (ADR 0823).** A destructure of a props object reads every slot it binds, so it asks ADR 0509's pending question as `props.v` does: a caller's async getter destructured in a leaf owner (`Pending`, and `Helper` through an IIFE in a helper the effect calls) is SC9012 `async-prop-getter`; an exported component's destructure in `onSettled` is `prop-pending-unproven` (`Open`); a signal destructured in a tracked effect stays clean (`Settled`).
