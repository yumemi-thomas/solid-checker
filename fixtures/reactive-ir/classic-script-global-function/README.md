# classic-script-global-function

**Claim (ADR 0904).** `globals.js` is a classic script (no import or export), so its top-level `function setTimeout` replaces the global object's. The patch census takes every top-level declaration of a classic script as a write of the global of that name (`("", setTimeout)`), so the module's `setTimeout(cb, 0)` is no standard timer: its callback's timing is unproven, never a later run. Before, the declaration raised no patch and the write in the callback was certified as outside the owned scope.
