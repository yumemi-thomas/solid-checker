// The laundering adversary: an installed dependency publishes a semantic
// document that calls `addClickInterceptor` a value even though it forwards
// its caller's callback. Phase 14 permits the document to reach analysis only
// through this fixture's exact accepted catalog entry and proof-issued receipt;
// scanning node_modules by package name is no longer an input path.
//
// The dependency has no typings, so this project cannot prove the kind either
// -- and a claim it cannot prove is not one it may republish.
export { hostValue, addClickInterceptor } from "laundered-dependency";
