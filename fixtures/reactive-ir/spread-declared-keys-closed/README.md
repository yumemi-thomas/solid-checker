# spread-declared-keys-closed

Claim (ADR 0394): a spread onto an intrinsic element holds no event handler when the compiler resolves its declared type to an object type whose property names are all known and the dialect lowers none of them as a listener. No spread here reports: a project helper's return type (`attrs(...)`), a plain props type, `omit(props, "class")` in place and through a `const rest`, an `omit` of the only `onClick` key, a union of two handler-free object types, and an intersection of two. The object literal keeps the exact resolution of ADR 0358.

The twin `spread-declared-keys-open` holds the spreads without that proof. The stub packages carry only the version (2.0.0-rc.17) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.
