import { touch } from "patchpkg";

// ADR 1135: `patchpkg` names solid-js, so a run with a declared host reads it as source and sees its write of
// `Array.prototype[key]`; the standard `at` below is then not proven standard (SC9012). A run with feedback facts reads the
// same program and must give the same findings.
export function C() {
  touch();
  void [4242].at(0);
  return null;
}

export function App() {
  return <C />;
}
