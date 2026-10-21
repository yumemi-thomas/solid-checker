A project function whose declaration is written later in its module is no
identity wrapper (ADR 0346). `helpers.js` declares `wrap` as a function that
returns its parameter and then assigns `wrap = function (cb) { return () => {}; }`:
the binding no longer holds the declared function, so `wrap(() => body)` does
not evaluate to the literal and the checker stays silent. The unwritten JS
wrapper `keep` in the same file is the positive control: a JavaScript identity
wrapper that is never reassigned names its literal, an SC4001 violation.
