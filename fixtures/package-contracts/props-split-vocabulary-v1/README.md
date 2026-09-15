# A props split is a props split under either dialect's spelling — 1.x half

The control for `props-split-vocabulary`. Same source shape, same erased types,
1.x's `splitProps` instead of 2.0's `omit`.

This half has always been correct, and that is what makes it worth committing:
the suppression it exercises was written as a comparison against
`Primitive::SplitProps`, so a fixture on this side could never have caught the
defect. Only the pair shows it. See the sibling's README for the differential
and for why the artifact carries no typings.
