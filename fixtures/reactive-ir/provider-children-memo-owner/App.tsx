import { children, createContext, createEffect, createMemo, createSignal, createTrackedEffect, flush, onCleanup } from "solid-js";

// ADR 1155 (b), the Opus review of ADR 1147 (N1): rc.21 evaluates an exact provider's children only inside its `children()` memo, whose owner is that memo, so a
// creation or a cleanup written in a child is owned there, whatever code creates the provider element. The child's owner rows go; the tag's own row stays
// (Chrome reports at the tag, PRIMITIVE_IN_FORBIDDEN_SCOPE or PRIMITIVE_IN_EFFECT_CALLBACK, or nothing). Stack questions (a flush) are not answered.

const Ctx = createContext<number>();

// p14b: the provider created in a timer callback; the cleanup in its child is owned by the memo (Chrome: NO_OWNER_CLEANUP only at the control below it).
export function TimeoutApp() {
  setTimeout(() => {
    const made = <Ctx value={1}>{(onCleanup(() => {}), 1)}</Ctx>;
    children(() => made)();
    onCleanup(() => {});
  }, 0);
  return <p />;
}

// p9: the provider created in an effect's apply (Chrome: PRIMITIVE_IN_EFFECT_CALLBACK at the tag).
export function ApplyApp() {
  const [n] = createSignal(0);
  createEffect(
    () => n(),
    () => {
      const made = <Ctx value={1}>{(onCleanup(() => {}), 1)}</Ctx>;
      void made;
    },
  );
  return <p />;
}

// p8: the provider created in a tracked effect (Chrome: PRIMITIVE_IN_FORBIDDEN_SCOPE at the tag).
export function TrackedApp() {
  const [n] = createSignal(0);
  createTrackedEffect(() => {
    n();
    const made = <Ctx value={1}>{createMemo(() => 1)()}</Ctx>;
    void made;
  });
  return <p />;
}

// p2: the provider rendered by a component that calls its render prop in an effect's apply (the review's package, here the project's).
function RenderApply(props: { render: () => unknown }) {
  createEffect(
    () => 0,
    () => {
      props.render();
    },
  );
  return null;
}
export function RenderApplyApp() {
  return <RenderApply render={() => <Ctx value={1}>{createMemo(() => 1)()}</Ctx>} />;
}

// Negative: a flush in the child of a provider created in a tracked effect: the stack's question, its row stays.
export function FlushApp() {
  const [n] = createSignal(0);
  createTrackedEffect(() => {
    n();
    const made = <Ctx value={1}>{(flush(), 1)}</Ctx>;
    void made;
  });
  return <p />;
}

// Negative: a cleanup in a function the child hands to a timer runs there, with no owner: its row stays.
export function HandedApp() {
  setTimeout(() => {
    const made = <Ctx value={1}>{(setTimeout(() => onCleanup(() => {}), 0), 1)}</Ctx>;
    children(() => made)();
  }, 0);
  return <p />;
}

// Negative: a written `let` context is no exact provider: the child's row stays.
let W = createContext<number>();
W = createContext<number>();
export function WrittenApp() {
  setTimeout(() => {
    const made = <W value={1}>{(onCleanup(() => {}), 1)}</W>;
    children(() => made)();
  }, 0);
  return <p />;
}

// Negative (the review of 1155, N2, Chrome: NO_OWNER_CLEANUP): a child with no call in it is evaluated at the tag by the compiler, not in the children memo,
// so a cleanup a constructor runs there is the timer's: no owner, its row stays.
class Make {
  constructor() {
    onCleanup(() => {});
  }
}
export function NewApp() {
  setTimeout(() => {
    const made = <Ctx value={1}>{(new Make(), 1)}</Ctx>;
    children(() => made)();
  }, 0);
  return <p />;
}
