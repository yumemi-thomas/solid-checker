# escaped-name-plain-string

ADR 1152: a string written with an escape is a plain value unless its whole value is one word. `"a\nb \u00e9 \t"`, the keys `"\n"` and `"\u00e9"`, a tagged template (the tag reads its raw text), a template with a substitution (dynamic) and `"\u00a0"` are values and no name any check compares, so the program is read as it is written and the SC2001 at `set(2)` stands, a violation, with no notice.

The positive twins are `escaped-name-identifier`, `escaped-name-other-file`, `escaped-name-key` and `escaped-name-event`, where an escaped identifier or a string that is one word makes the same violation uncertifiable.

The stub reduces the published rc.21 declarations (`createSignal`). Package bytes are not copied here.
