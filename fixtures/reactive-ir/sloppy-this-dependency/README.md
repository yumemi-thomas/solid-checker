# sloppy-this-dependency

**Claim (ADR 0803, review D4).** `dep-sloppy` (synthetic, not a published package) is a CommonJS script with no `"use strict"`: `install()` called plainly runs with the global object as `this`, and `this.Array.prototype` is patched under a key no literal names. `this` read through to a global's name (`Array`, `Function`, `eval`, `Reflect`, `Object`, `global`) obtains it in any file, so the primitive's slots are withdrawn: `set(2)` in `body-write` is uncertifiable (`tuple-iteration-written`), never SC2001. 2.0.0-rc.21 in Chrome: quiet, the patched iterator hands back a no-op setter. Before: SC2001 violation.

Variants (sibling fixtures, same body):
- `sloppy-this-computed`: `this[name]` under a key no literal names, in sloppy code. Uncertifiable.
- `sloppy-this-handed`: sloppy code hands `this` to a function that patches through its parameter. Uncertifiable.
- `sloppy-this-alias`: `var self = this`, never reassigned, read under a key no literal names. Uncertifiable.
- `sloppy-this-strict` (negative): a strict script reads `this.values[key]` in a method and writes `this.name` in a constructor. Proved SC2001.
