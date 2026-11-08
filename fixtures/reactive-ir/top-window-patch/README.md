# top-window-patch

**Claim (ADR 0906, review R64).** `top`, `parent` and `frames` are the window of an unframed page, so `top.setTimeout = f` replaces the window's `setTimeout`. The module's `setTimeout(cb, 0)` is no standard timer. Before, `standard_root` knew only `globalThis`, `window`, `self` and `global` and certified the program. Twin `top-window-local`: a `top` the module declares is its own.
