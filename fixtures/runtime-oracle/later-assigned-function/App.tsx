import { action, createEffect, createMemo, createSignal, flush } from "solid-js";

const [n] = createSignal(0);

// Batch 11 (ADR 0535 amended; fuzz cells `la-*`). A function the program ASSIGNS to a variable after its declaration holds the value written
// last, and every call of the name resolves to the declaration's initializer (or to nothing). The assigned function has no binding name, so every
// walk that asks who calls a function read "nobody", and a `flush()` or a primitive created in it was placed nowhere: certified clean while
// Chrome (rc.21) reports FLUSH_IN_EFFECT_CALLBACK, FLUSH_IN_ACTION and PRIMITIVE_IN_EFFECT_CALLBACK at the site. The member forms (`o.f = …`) were
// rowed since ADR 0535. A function written as the value of any assignment is one nothing places: an uncertifiable row (`function-run-unplaced`)
// at the site, never a violation (nothing here proves the call runs it) and never silence.

// An effect's apply, the three ways a variable holds a function assigned later.
export function LetLater() {
  createEffect(() => n(), () => {
    let h = () => {};
    h = () => flush();
    h();
  });
  return <p>{n()}</p>;
}

export function LetNoInit() {
  createEffect(() => n(), () => {
    let h: () => void;
    h = () => flush();
    h();
  });
  return <p>{n()}</p>;
}

export function LetOther() {
  createEffect(() => n(), () => {
    let h = () => {};
    const set = () => {
      h = () => flush();
    };
    set();
    h();
  });
  return <p>{n()}</p>;
}

// A primitive created in it (SC6002).
export function LetCreates() {
  createEffect(() => n(), () => {
    let h = () => {};
    h = () => {
      createMemo(() => 1);
    };
    h();
  });
  return <p>{n()}</p>;
}

// An action's step (SC2006).
const stepLater = action(function* () {
  let h = () => {};
  h = () => flush();
  h();
  yield 1;
});
export function StepLater() {
  return <button onClick={() => stepLater()}>go</button>;
}

// Called before the reassignment: the call is of a binding the walk does not resolve (rowed before this change too).
export function LetEarly() {
  createEffect(() => n(), () => {
    let h = () => flush();
    h();
    h = () => {};
  });
  return <p>{n()}</p>;
}

// A member and an element written the same way (rowed since ADR 0535).
export function MemberLater() {
  createEffect(() => n(), () => {
    const o = { f: () => {} };
    o.f = () => flush();
    o.f();
  });
  return <p>{n()}</p>;
}

export function ElementLater() {
  createEffect(() => n(), () => {
    const a = [() => {}];
    a[0] = () => flush();
    a[0]();
  });
  return <p>{n()}</p>;
}

// Silent: the same shape with no site in the function assigned (nothing to place).
export function NothingToFind() {
  createEffect(() => n(), () => {
    let h = () => {};
    h = () => 1;
    h();
  });
  return <p>{n()}</p>;
}
