import * as reactive from "reactive-package";
import { createConstant, createCount, createCount as makeCounter } from "reactive-package";

const count = createCount();
const counter = makeCounter();
const fromNamespace = reactive.createCount();
const constant = createConstant();

// Negative: the read is in a tracked JSX expression.
export function Good() {
  return <p>{count()}</p>;
}

// Positive: the deleted package-return-consumer fixture's Bad case. The
// package source returns a signal getter; reading it in the component body is
// untracked.
export function Bad() {
  const value = count();
  return <p>{value}</p>;
}

// Positive: the same export through an import alias.
export function BadAlias() {
  const value = counter();
  return <p>{value}</p>;
}

// Positive: the same export through a namespace member.
export function BadNamespace() {
  const value = fromNamespace();
  return <p>{value}</p>;
}

// Positive: the accessor is created in the component body.
export function BadLocal() {
  const local = createCount();
  const value = local();
  return <p>{value}</p>;
}

// Negative: same declared type, but the package source returns a function
// that reads no signal.
export function Constant() {
  const value = constant();
  return <p>{value}</p>;
}
