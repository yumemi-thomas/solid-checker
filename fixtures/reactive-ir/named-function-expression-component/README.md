# named-function-expression-component

ADR 0377: a named function expression a `const` initializes (`const X = function X(props) {}`) is a component reached through that `const`, as an unnamed one is (ADR 0214, 0373). Its own name is bound only inside it, so its references are walked too: each must be its declaration or a resolved tag (a self-render, whose attributes join the census); handed out as a value it escapes, and a missing reference list is no census.

`Card.tsx`: `Static` and `Renamed` (inner name `Inner`) are only rendered with string labels, so their reads are quiet. `Reactive` is rendered with a signal read, so its read is a proven SC1001 violation. Kept as obligations: `Tree`, whose self-render through the inner name passes a reactive label; `Handed`, whose inner name is pushed into an array; `Mutable`, bound by a `let`.
