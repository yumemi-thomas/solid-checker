type Runner = { run: () => void };

function helper(o: Runner) {
  o.run();
}

// Code that copies keys into an existing object, or assigns the member by
// name, can replace `run` on any object it reaches: no literal's `run` is
// trusted in this program.
export function AssignedLater() {
  const held = { run() {} };
  Object.assign(held, opaque);
  helper(held);
  return <main />;
}
export function WrittenMember() {
  const held = { run() {} };
  held.run = opaque.run;
  helper(held);
  return <main />;
}
// Control: no copy or write can touch this literal: it is written as the
// argument, which `helper` only reads (ADR 0321).
export function Untouched() {
  helper({ run() {} });
  return <main />;
}
