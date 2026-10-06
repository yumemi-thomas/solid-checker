import { createSignal, onCleanup, onSettled } from "solid-js";

function register() {
  onCleanup(() => {});
}

function each(list: number[]) {
  list.forEach(() => register());
}

// Violation: `forEach` runs `register` before it returns, inside the leaf
// scope.
export function InlineIdentifier() {
  const [items] = createSignal([1, 2]);
  onSettled(() => {
    items().forEach(register);
  });
  return <div />;
}

// Violation: the same through a function literal.
export function InlineLiteral() {
  const [items] = createSignal([1, 2]);
  onSettled(() => {
    items().forEach(() => onCleanup(() => {}));
  });
  return <div />;
}

// Violation: the same one helper down.
export function InlineInHelper() {
  const [items] = createSignal([1, 2]);
  onSettled(() => {
    each(items());
  });
  return <div />;
}

// Clean: the inline callback only logs, and `console.log` reads its
// arguments.
export function InlineClean() {
  const [items] = createSignal([1, 2]);
  onSettled(() => {
    items().forEach((item) => console.log(item));
  });
  return <div />;
}

// Clean: `setTimeout` runs its callback from a host queue, after the leaf
// scope is gone.
export function FreshStack() {
  onSettled(() => {
    setTimeout(() => console.log("later"));
  });
  return <div />;
}

// Clean: a listener runs after the call returns. Synchronous dispatch is not
// modeled (ADR 0210).
export function Listener() {
  onSettled(() => {
    document.body.addEventListener("click", register);
  });
  return <div />;
}

function pending(): PromiseLike<number> {
  return Promise.resolve(1);
}

// Uncertifiable: a `PromiseLike` is any object with a `then`, which may call
// back before it returns.
export function Thenable() {
  onSettled(() => {
    pending().then(register);
  });
  return <div />;
}

// Uncertifiable: no audited timing says when `Promise` runs its executor.
export function UnauditedCallback() {
  onSettled(() => {
    new Promise<void>((resolve) => {
      register();
      resolve();
    });
  });
  return <div />;
}

// Violation: a setter runs its updater before it returns.
export function SetterUpdater() {
  const [count, setCount] = createSignal(0);
  onSettled(() => {
    setCount((previous) => {
      onCleanup(() => {});
      return previous + count();
    });
  });
  return <div />;
}

// Clean: an updater that only computes.
export function SetterUpdaterClean() {
  const [, setCount] = createSignal(0);
  onSettled(() => {
    setCount((previous) => previous + 1);
  });
  return <div />;
}

// Clean: `bind` returns a bound function and runs nothing.
export function BindDoesNotRun() {
  onSettled(() => {
    const later = register.bind(null);
    document.body.dataset.ready = String(typeof later);
  });
  return <div />;
}

// Violation: `call` runs its receiver before it returns.
export function CallRunsReceiver() {
  onSettled(() => {
    register.call(null);
  });
  return <div />;
}
