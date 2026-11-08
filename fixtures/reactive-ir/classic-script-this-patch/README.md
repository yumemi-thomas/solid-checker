# classic-script-this-patch

**Claim (ADR 0906, review R64).** A classic script's top-level `this` is the global object, so `this.setTimeout = f` replaces the window's `setTimeout` (`("", setTimeout)`). The module's `setTimeout(cb, 0)` is no standard timer. Before, `standard_root` knew the global object only by name and certified the program.
