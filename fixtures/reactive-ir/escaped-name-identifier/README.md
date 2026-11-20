# escaped-name-identifier

ADR 1152: a name is its decoded value. `al\u0069as` is the identifier `alias`, and `set(2)` in the body of `BodyWrite` is a write during render, a proven SC2001 when the program writes no name with an escape. Many checks compare a name to a literal or take its first letter and read the source text of its span, which for an escaped name is the spelling, so the program is read wrongly: a violation reported at code that is not one, or lost. Neither is proven. A file of the program that writes an identifier with an escape makes every violation of the program uncertifiable (SC2001 with the blocker `escaped-name-unresolved`) and gives each project file one uncertifiable finding that spans it (SC9012, the same blocker), so no site of it reads clean. Nothing here is reported to a user (an uncertifiable finding of subject `file` is internal, ADR 0328).

The twin is `escaped-name-plain-string`, the same component written beside strings with escapes that are no names: its SC2001 stands. `escaped-name-component` is the escape of the component's own name.

The stub reduces the published rc.21 declarations (`createSignal`). The complete source also type-checks against published solid-js rc.21 with strict ES2022 settings. Package bytes are not copied here.
