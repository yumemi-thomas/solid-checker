import { action, createEffect, createMemo, createSignal, createStore } from "solid-js";

// ADR 0481: `thing.key`, where `thing` holds an object-literal arrow's fresh
// result and is only ever read that way, is the `const` the key names.

// Violation: a signal write in a memo through the member (rc.17 throws).
export function SetterMemberInMemo() {
  const [count, setCount] = createSignal(0);
  const useCounter = () => ({ set: setCount });
  const counter = useCounter();
  const m = createMemo(() => {
    counter.set(1);
    return 1;
  });
  return <div>{count()}{m()}</div>;
}

// Clean: the same write in a handler lands.
export function SetterMemberInHandler() {
  const [count, setCount] = createSignal(0);
  const useCounter = () => ({ set: setCount });
  const counter = useCounter();
  return <button onClick={() => counter.set(1)}>{count()}</button>;
}

// Violation: an action called in a memo through the member.
export function ActionMemberInMemo() {
  const [count, setCount] = createSignal(0);
  const save = action(function* () {
    setCount(1);
  });
  const useSaver = () => ({ save });
  const saver = useSaver();
  const m = createMemo(() => {
    saver.save();
    return 1;
  });
  return <div>{count()}{m()}</div>;
}

// Not an alias: the holder is handed out, so its property may be replaced.
export function HolderPassed() {
  const [count, setCount] = createSignal(0);
  const useCounter = () => ({ set: setCount });
  const counter = useCounter();
  console.log(counter);
  const m = createMemo(() => {
    counter.set(1);
    return 1;
  });
  return <div>{count()}{m()}</div>;
}

// ADR 0482: a store written through such a member.
// Violation: outside its setter, the write is dropped (rc.17 warns).
export function StoreMemberWrite() {
  const [state, setState] = createStore({ v: 0, items: [0] });
  const useState = () => ({ state, setState });
  const holder = useState();
  holder.state.v = 1;
  holder.state.items.push(1);
  return <div>{state.v}</div>;
}

// Clean: inside its own setter, called directly or through the member.
export function StoreMemberInSetter() {
  const [state, setState] = createStore({ v: 0 });
  const useState = () => ({ state, setState });
  const holder = useState();
  return (
    <button
      onClick={() => {
        setState(() => {
          holder.state.v = 2;
        });
        holder.setState(() => {
          holder.state.v = 3;
        });
      }}
    >
      {state.v}
    </button>
  );
}

// Clean (review case): an alias of an imported store, read through a member,
// written inside that store's own setter.
import { setShared, shared } from "./store";
const sharedAlias = shared;
const useShared = () => ({ state: sharedAlias });
export function ImportedStoreAliasInSetter() {
  const holder = useShared();
  return (
    <button
      onClick={() => {
        setShared(() => {
          holder.state.v = 4;
        });
      }}
    ></button>
  );
}

// Violation (ADR 0461 through ADR 0482): deleting an optional store property
// through such a member, outside its setter, is dropped.
export function StoreMemberDelete() {
  const [opt] = createStore<{ v?: number }>({ v: 0 });
  const useOpt = () => ({ opt });
  const holder = useOpt();
  delete holder.opt.v;
  return <div></div>;
}

// Violation (SC1002 through ADR 0481): the member read runs in a closure after
// the memo's await, untracked.
export function MemberReadAfterAwaitInClosure() {
  const [count] = createSignal(0);
  const useCount = () => ({ read: count });
  const holder = useCount();
  const m = createMemo(async () => {
    await Promise.resolve();
    const peek = () => {
      holder.read();
    };
    peek();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div></div>;
}

declare const flag: boolean;

// ADR 0499: a store link read for a write in a closure the body calls once
// runs during that call, in the body's strict-read window (violation), also
// through a member alias.
export function StoreLinkInClosure() {
  const [state] = createStore({ v: 0, nested: { v: 0 } });
  const write = () => {
    state.nested.v = 1;
  };
  write();
  return <div>{state.v}</div>;
}

export function StoreLinkInClosureThroughMember() {
  const [state, setState] = createStore({ v: 0, nested: { v: 0 } });
  const useState = () => ({ state, setState });
  const holder = useState();
  const write = () => {
    holder.state.nested.v = 1;
  };
  write();
  return <div>{state.v}</div>;
}

// Uncertifiable: the write is on one path of the closure only.
export function StoreLinkInClosureConditional() {
  const [state] = createStore({ v: 0, nested: { v: 0 } });
  const write = () => {
    if (flag) state.nested.v = 1;
  };
  write();
  return <div>{state.v}</div>;
}

// Negative: the closure runs only from a handler.
export function StoreLinkInHandlerClosure() {
  const [state] = createStore({ v: 0, nested: { v: 0 } });
  const write = () => {
    state.nested.v = 1;
  };
  return <button onClick={() => write()}>{state.v}</button>;
}
