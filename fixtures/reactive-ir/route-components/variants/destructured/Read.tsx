// A component key read back through a defaulted key of an assignment
// pattern: the value read back could be called by the project.
let Invoke: (() => unknown) | undefined;
export function pick(options: { component?: () => unknown }) {
  ({ component: Invoke = undefined } = options);
  return Invoke;
}
