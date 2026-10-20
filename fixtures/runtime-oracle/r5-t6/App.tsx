type Runner = { run: () => void };
function helper(o: Runner) {
  o.run();
}
class K {
  run() {}
}
export function C() {
  helper(new K());
  const held = { run() {} };
  helper(held);
  return <main />;
}
