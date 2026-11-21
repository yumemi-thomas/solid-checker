# escaped-name-key

ADR 1152: a string in a key position is a name when its whole value is a word. `"ke\u0079"` is the key `key`, which a check that reads an option's key or a member's name by its spelling does not see, so the program is not proven: the SC2001 at `set(2)` is uncertifiable (`escaped-name-unresolved`) and the file carries the notice that spans it.

The twin is `escaped-name-plain-string`, where the keys written with escapes (`"\n"`, `"\u00e9"`) are no word and no check holds a name for them: the SC2001 stands.

The stub reduces the published rc.21 declarations (`createSignal`). Package bytes are not copied here.
