export { hostValue } from "untyped-dependency";
// The other half of closing the laundering channel below: re-deciding a
// carried kind has to *decide* it, not merely refuse. `laundered-typed-
// dependency` ships an unreviewed `inferred` contract in its own
// `node_modules` directory calling `addTypedInterceptor` a `value`, but it also
// ships declarations, so this project can prove the kind -- and the wrong
// carried negative is corrected here to `function` with callbacks unknown.
export { addTypedInterceptor } from "laundered-typed-dependency";
