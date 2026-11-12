import { createContext, createEffect, onCleanup, useContext } from "solid-js";

// The follow-up to the review of #220: a function that returns JSX, entered by a reference no tag is (a timer's, a handler's, an effect apply's call through a variable), where no owner
// is current (the program of fixtures/reactive-ir/owner-emission-component-by-reference, every component exported, a context read added). Chrome rc.21 reports NO_OWNER_CLEANUP,
// NO_OWNER_EFFECT and NO_OWNER_CONTEXT at each; main certified every one: the owner emission's `named_layer_entry_open` stopped at a function the census proves a component before it
// looked at its references (the component-render premise). Now each keeps an uncertifiable SC4001 row. The control, a component only a tag renders, is quiet.

let byTimer: (() => unknown) | undefined;
function viewTimer() {
  onCleanup(() => {});
  return <p>timer</p>;
}
export function ByTimer() {
  byTimer = viewTimer;
  setTimeout(() => {
    byTimer!();
  }, 0);
  return <p>by timer</p>;
}

let byHandler: (() => unknown) | undefined;
function viewHandler() {
  onCleanup(() => {});
  return <p>handler</p>;
}
export function ByHandler() {
  byHandler = viewHandler;
  return (
    <button
      onClick={() => {
        byHandler!();
      }}
    >
      go
    </button>
  );
}

let byApply: (() => unknown) | undefined;
function viewApply() {
  onCleanup(() => {});
  return <p>apply</p>;
}
export function ByApply() {
  byApply = viewApply;
  createEffect(
    () => 0,
    () => {
      byApply!();
    },
  );
  return <p>by apply</p>;
}

// The same with an effect created with no owner.
let effectByTimer: (() => unknown) | undefined;
function viewEffect() {
  createEffect(
    () => 0,
    () => {},
  );
  return <p>effect</p>;
}
export function EffectByTimer() {
  effectByTimer = viewEffect;
  setTimeout(() => {
    effectByTimer!();
  }, 0);
  return <p>effect by timer</p>;
}

// And a context read with no owner.
const Ctx = createContext(1);
let contextByTimer: (() => unknown) | undefined;
function viewContext() {
  useContext(Ctx);
  return <p>context</p>;
}
export function ContextByTimer() {
  contextByTimer = viewContext;
  setTimeout(() => {
    contextByTimer!();
  }, 0);
  return <p>context by timer</p>;
}

// The control: a component only a tag renders runs under the owner its render sets.
function Tagged() {
  onCleanup(() => {});
  return <p>tag</p>;
}
export function TaggedOnly() {
  return (
    <p>
      <Tagged />
    </p>
  );
}
