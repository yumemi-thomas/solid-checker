# The same export name is a different *kind* under each condition

`conditionalShape` is a function taking a callback in the `development` branch
and an inert number in the `default` branch. Both branches are published by the
one `.` entrypoint, so the contract carries two artifact cases: one whose
summary is `callable` with a same-stack callback operation on argument 0, and
one whose summary is `plain`.

Nothing merges them. Publishing a single summary would have to pick a shape
that is wrong in one of the two environments -- `callable` invents a callback
obligation for the consumer who resolves `default`, and `plain` erases a real
one for the consumer who resolves `development`.

`conditional-returns-divergence` pins the same split for the returns domain and
`conditional-export-absence` for a name that exists in only one branch; this
fixture is the one where the *shape* itself diverges.
