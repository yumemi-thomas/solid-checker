# A type query is not a runtime entry

In an application (`"private": true`, ADR 0193/0203) an export's caller-supplied-member obligation drops when every entry is a resolved call: `describe`, also named by `typeof describe` in type aliases (erased), clears (its reactive `label` at the `App` call stays SC1001); `observed` (a runtime `typeof observed` value expression) and `escaped` (put in an exported array) stay `SC9012`. Type-checks clean against published `solid-js@2.0.0-rc.13` and the local stub.
