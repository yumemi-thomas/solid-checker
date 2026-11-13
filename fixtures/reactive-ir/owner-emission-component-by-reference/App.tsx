import { createEffect, onCleanup } from "solid-js";
import { render } from "@solidjs/web";

// Batch 11, the follow-up to the review of #220: the owner emission (`owners/owner_emission.rs`, ADR 0868) asks `named_layer_entry_open` whether a layer around an owner-requiring
// operation (`onCleanup`, an effect, a context read) may be entered by a route the owner graph holds no edge for, and stopped at a function the census proves a component (it returns
// JSX and no call enters it; one a reference also enters included) before it looked at the references: the component-render premise. The graph gives such a function an owned
// context, so the operation was certified (no row) although Chrome rc.21 reports NO_OWNER_CLEANUP / NO_OWNER_EFFECT where a timer, a handler or an apply calls it. A component is now
// entered by renders alone only where every reference of its binding is a JSX tag; a reference that enters it is a layer like any other, and the operation keeps an uncertifiable row.

// A component only a tag renders: its render sets the owner, so the cleanup is owned (silent).
function Tagged() {
  onCleanup(() => {});
  return <p>tagged</p>;
}

// A function that returns JSX, no tag renders it, and a timer calls it through a variable: no owner is current at the cleanup (NO_OWNER_CLEANUP). SC4001 uncertifiable.
let byTimer: (() => unknown) | undefined;
function viewTimer() {
  onCleanup(() => {});
  return <p>timer</p>;
}
function Timed() {
  byTimer = viewTimer;
  setTimeout(() => {
    byTimer!();
  }, 0);
  return <p>timed</p>;
}

// The same through a handler.
let byHandler: (() => unknown) | undefined;
function viewHandler() {
  onCleanup(() => {});
  return <p>handler</p>;
}
function Handled() {
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

// The same through an effect's apply, whose stack holds no owner either.
let byApply: (() => unknown) | undefined;
function viewApply() {
  onCleanup(() => {});
  return <p>apply</p>;
}
function Applied() {
  byApply = viewApply;
  createEffect(
    () => 0,
    () => {
      byApply!();
    },
  );
  return <p>applied</p>;
}

// An effect created with no owner (NO_OWNER_EFFECT).
let effectByTimer: (() => unknown) | undefined;
function viewEffect() {
  createEffect(
    () => 0,
    () => {},
  );
  return <p>effect</p>;
}
function EffectTimed() {
  effectByTimer = viewEffect;
  setTimeout(() => {
    effectByTimer!();
  }, 0);
  return <p>effect timed</p>;
}

// A component a tag renders AND a reference enters (an escaped component): the render is one entry, the timer's call another with no owner.
let both: (() => unknown) | undefined;
function Both() {
  onCleanup(() => {});
  return <p>both</p>;
}
function UsesBoth() {
  both = Both;
  setTimeout(() => {
    both!();
  }, 0);
  return (
    <p>
      <Both />
    </p>
  );
}

// The twin: the function is held and never called, so no cleanup runs and Chrome is quiet; the row is the cost of not proving that no reference calls it.
let held: (() => unknown) | undefined;
function viewHeld() {
  onCleanup(() => {});
  return <p>held</p>;
}
function Holds() {
  held = viewHeld;
  return <p>holds</p>;
}

// The root: each component above is rendered from here, so `Tagged` has an owner edge.
function App() {
  return (
    <p>
      <Tagged />
      <Timed />
      <Handled />
      <Applied />
      <EffectTimed />
      <UsesBoth />
      <Holds />
    </p>
  );
}
render(() => <App />, document.body);
