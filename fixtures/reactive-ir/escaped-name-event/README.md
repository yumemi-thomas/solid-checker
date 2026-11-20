# escaped-name-event

ADR 1152: a string anywhere whose whole value is a word may be read as a name. `"c\u006cick"` is the event `click`, registered on the window, which a check that matches an event by its spelling does not see, so the program is not proven: the SC2001 at `set(2)` is uncertifiable (`escaped-name-unresolved`) and the file carries the notice that spans it.

The twin is `escaped-name-plain-string`, where the strings written with escapes are no word: the SC2001 stands.

The stub reduces the published rc.21 declarations (`createSignal`). Package bytes are not copied here.
