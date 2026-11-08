# show-named-child-length

`Show` and `Match` call a function child only when its `length` is above 0;
otherwise it is a value, run wherever it is read. A named child follows the rule
an inline child follows: `length` 0, written or by a default parameter, is not a
callback, and its read is quiet (rc.21, Chrome). With a parameter it is called
under `untrack`, a strict read. An inline child of length 0 held and run in the body (`InlineValueInBody`, reviewer R57) reads strictly in Chrome: uncertifiable, as its named twin. A `<Show>` a plain function returns, or one that is part of a returned value, is not inserted either (reviewer R59): only a component's rendered return, or the inserted value itself, is.
