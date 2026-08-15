import {
  action,
  createMemo,
  createOptimistic,
  createSignal,
  createTrackedEffect,
  onSettled,
  refresh,
  untrack,
} from "solid-js";

const [value, setValue] = createSignal(0, { ownedWrite: true });
createMemo(() => setValue(value() + 1));

const [, setOptimistic] = createOptimistic(0, { ownedWrite: true });
createMemo(() => setOptimistic(1));

onSettled(() => {
  refresh(value);
  return () => console.log("clean");
});

// Children-forbidden leaf scopes are legal write/action regions: the rc.0
// write guard exempts them ("leaf imperative scopes ... stay legal"), so a
// plain (non-ownedWrite) setter, a refresh, and an action are all fine here.
const [plain, setPlain] = createSignal(0);
const save = action(function* () {});
createTrackedEffect(() => {
  plain();
  setPlain(1);
  refresh(value);
  save();
});

// untrack in an unowned scope stays legal — no owner is live at module
// evaluation, and untrack does not create one.
untrack(() => setPlain(2));
