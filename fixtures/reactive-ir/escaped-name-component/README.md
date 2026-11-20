# escaped-name-component

ADR 1152: whether a function is a component is decided from the first letter of its name's source text (a capital letter), and `\u0042odyWrite` is the component `BodyWrite` whose first character in the source is a backslash. A check that reads the spelling does not take it for a component, so the write during render in its body is not placed: the SC2001 at `set(2)` is an obligation (`component-identity-unproven`), not the violation it is when the name is written `BodyWrite`. The file also writes an identifier with an escape, so the program carries the notice of ADR 1152 (SC9012 `escaped-name-unresolved`), which spans the file: that is what holds a site clean-looking elsewhere, where a misread name hid the site from the analysis altogether, an obligation.

The twin is `escaped-name-plain-string`, the same component written plainly: its SC2001 stands, a violation.

The stub reduces the published rc.21 declarations (`createSignal`). Package bytes are not copied here.
