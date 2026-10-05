import { getOwner, onCleanup, runWithOwner } from "solid-js";

// Clean: the component's own owner, carried across a promise. `getOwner()` in
// a component's body is never null.
export function Carried() {
  const owner = getOwner();
  void Promise.resolve().then(() => {
    runWithOwner(owner, () => {
      onCleanup(() => {});
    });
  });
  return <div />;
}

// Uncertifiable: a helper is not proven to run under an owner.
function carryFromHelper() {
  const owner = getOwner();
  void Promise.resolve().then(() => {
    runWithOwner(owner, () => {
      onCleanup(() => {});
    });
  });
}
export function CallsHelper() {
  carryFromHelper();
  return <div />;
}

// Uncertifiable: a `let` may hold something else by the time it is used.
export function Reassignable() {
  let owner = getOwner();
  void Promise.resolve().then(() => {
    runWithOwner(owner, () => {
      onCleanup(() => {});
    });
  });
  owner = null;
  return <div />;
}

// Uncertifiable: `getOwner()` in a nested function returns the owner that
// function is called under, here a timer's: none.
export function NestedGetOwner() {
  const later = () => {
    const owner = getOwner();
    void Promise.resolve().then(() => {
      runWithOwner(owner, () => {
        onCleanup(() => {});
      });
    });
  };
  setTimeout(later, 0);
  return <div />;
}
