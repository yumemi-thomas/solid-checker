// The laundering channel: a dependency contract nobody reviewed, found on
// disk. `dependencyContracts()` walks `node_modules/<dep>/solid-reactivity.json`
// upward and supplies whatever it finds, with no `--contract` flag from the
// user -- so an `inferred` contract written by any earlier solid-checker,
// including one with the `Unknown => value` defect this rule exists to close,
// arrives here indistinguishable from a reviewed one unless provenance is
// checked. `addClickInterceptor` forwards its caller's callback and that
// contract calls it a `value`.
//
// The dependency has no typings, so this project cannot prove the kind either
// -- and a claim it cannot prove is not one it may republish.
export { hostValue, addClickInterceptor } from "laundered-dependency";
