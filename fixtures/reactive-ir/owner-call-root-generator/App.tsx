import { createEffect, createMemo, createRoot } from "solid-js";
import { render } from "@solidjs/web";

// Batch 11, slice 6, the review of #220 (F3): `EffectCreationOwnerSetAround` ended a creation written in the callback of an exact primitive that creates an owner
// (`createRoot`) without asking whether the callback runs in the primitive's synchronous run. A generator's body runs where an iterator is stepped, after the root's
// owner is gone; a parameter default runs where a call omits the argument; code after an `await` runs from a later task. None has the owner the primitive set, so none
// is a plain site, and none is ended here (the owner stage's `generator-body` decline is the same question).

// The root's callback is a generator an apply steps: the creation runs on the apply's stack, with no owner (PRIMITIVE_IN_EFFECT_CALLBACK, Chrome rc.21).
function RootGenerator() {
  const outer = createMemo(() =>
    createRoot(function* () {
      createMemo(() => 1);
      yield 1;
    }),
  );
  createEffect(
    () => outer(),
    (it) => {
      it.next();
    },
  );
  return <p>generator</p>;
}

// The control: a plain callback runs under the owner the root creates, in the memo's compute (`EffectCreationOwnerSetAround`).
function RootPlain() {
  const outer = createMemo(() =>
    createRoot(() => {
      createMemo(() => 2);
      return 2;
    }),
  );
  return <p>{outer()}</p>;
}

// After an await in the root's async callback the creation runs from a later task, outside the root's run: no effect's apply is on the stack (legal in Chrome), but the
// owner the root set is gone too, and nothing here proves the task is no apply's, so the row stands.
function RootAsync() {
  const outer = createMemo(() =>
    createRoot(async () => {
      await Promise.resolve();
      createMemo(() => 3);
    }),
  );
  return <p>{String(outer())}</p>;
}

function App() {
  return (
    <p>
      <RootGenerator />
      <RootPlain />
      <RootAsync />
    </p>
  );
}
render(() => <App />, document.body);
