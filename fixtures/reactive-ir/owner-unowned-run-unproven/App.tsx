import { onCleanup } from "solid-js";

// Each hook is called once under the component's owner and once from module
// scope or a timer: a mixed owner context.

// Violation: an unconditional module-scope call runs with no owner.
function useModuleCall() {
  onCleanup(() => {});
  return 1;
}
export const story = { args: { value: useModuleCall() } };

// Violation: an unconditional timer callback runs with no owner.
function useTimer() {
  onCleanup(() => {});
}

// Obligation: the module-scope call sits in a branch, so no unowned run is
// proven; the owned call runs on render.
function useDeadBranch() {
  onCleanup(() => {});
}
if (Math.random() > 2) useDeadBranch();

// Obligation: the helper's call is conditional, so the module's call of the
// helper proves no unowned run of the hook.
function useHelperBranch() {
  onCleanup(() => {});
}
function boot(enabled: boolean) {
  if (enabled) useHelperBranch();
}
boot(Math.random() > 2);

// Obligation: a `&&` operand may not be evaluated.
function useLogical() {
  onCleanup(() => {});
}
const enabled = Math.random() > 2;
enabled && useLogical();

// Obligation: the timer is set in a branch.
function useTimerBranch() {
  onCleanup(() => {});
}

// Clean: owned calls only.
function useOwned() {
  onCleanup(() => {});
}

export function App() {
  useModuleCall();
  useTimer();
  setTimeout(() => useTimer(), 0);
  useDeadBranch();
  useHelperBranch();
  useLogical();
  useTimerBranch();
  if (enabled) setTimeout(() => useTimerBranch(), 0);
  useOwned();
  return <i>{story.args.value}</i>;
}
