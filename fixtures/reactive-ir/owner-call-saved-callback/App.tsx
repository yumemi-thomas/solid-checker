import { createEffect, createMemo, untrack } from "solid-js";
import { render } from "@solidjs/web";

// Batch 11, slice 6, the review of #220 (F2): the role engine places a handler, a timer callback or the body of a component off every effect's apply, from where the
// function is written. It does not follow a function that is also stored (a variable, an array) and called from an apply, so the lexical role alone ended these
// sites (`EffectCreationRunsOffApply`). The silence now rests on the role AND on the same three facts the graph's silences ask: the site is plain (not in a generator,
// a parameter list or after an await), no package the build does not read may run the function (B3), and no layer around the site is entered by a reference that is no
// direct call (a function value no binding names that is neither run in place nor a whole argument of a call, or a binding with any other reference).

// The handler is also stored in `saved` (an assignment is an expression: the attribute still yields the arrow), and an apply calls `saved`: the creation runs on the
// apply's stack, with no owner (PRIMITIVE_IN_EFFECT_CALLBACK, Chrome rc.21).
let saved: (() => void) | undefined;
function Saved() {
  createEffect(
    () => 0,
    () => {
      saved!();
    },
  );
  return <button onClick={(saved = () => { createMemo(() => 1); })}>saved</button>;
}

// A timer callback saved the same way.
let savedTimer: (() => void) | undefined;
function SavedTimer() {
  createEffect(
    () => 0,
    () => {
      savedTimer!();
    },
  );
  setTimeout((savedTimer = () => { createMemo(() => 2); }), 1000);
  return <p>timer</p>;
}

// A named handler an array also holds, called from an apply.
function InArray() {
  const handler = () => {
    createMemo(() => 3);
  };
  const all = [handler];
  createEffect(
    () => 0,
    () => {
      all[0]!();
    },
  );
  return <button onClick={handler}>array</button>;
}

// A local generator the apply steps: its body runs where the apply steps the iterator, on the apply's stack, and a generator's body is no plain site.
function StepGenerator() {
  function* steps() {
    createMemo(() => 4);
    yield 1;
  }
  const it = steps();
  createEffect(
    () => 0,
    () => {
      it.next();
    },
  );
  return <p>generator</p>;
}

// A generator literal `untrack` hands back, stepped the same way: `it.next` is a standard method no census follows, so a package may run the generator (B3).
function StepUntrack() {
  const it = untrack(function* () {
    createMemo(() => 8);
    yield 1;
  });
  createEffect(
    () => 0,
    () => {
      it.next();
    },
  );
  return <p>untrack</p>;
}

// Controls. A timer callback that is a whole argument of the call: nothing else holds it, so its entry is the timer (`EffectCreationRunsOffApply`).
function Timer() {
  setTimeout(() => {
    createMemo(() => 5);
  });
  return <p>timer</p>;
}

// A component body, rendered by a tag only: rendering runs it under the owner its render sets.
function Body() {
  const memo = createMemo(() => 6);
  return <p>{memo()}</p>;
}

// An inline handler on a native element: nothing else holds the arrow, and the browser runs it on its own stack. The entry test reads a function value no binding names
// that is not a whole argument of a call as an object slot or an array element, so the creation keeps its row: the cost of asking the entry test of the role (the proof
// batch: a literal the compiler binds as the element's event handler is entered only by the event).
function Handler() {
  return <button onClick={() => { createMemo(() => 7); }}>handler</button>;
}

function App() {
  return (
    <p>
      <Saved />
      <SavedTimer />
      <InArray />
      <StepGenerator />
      <StepUntrack />
      <Timer />
      <Body />
      <Handler />
    </p>
  );
}
render(() => <App />, document.body);
