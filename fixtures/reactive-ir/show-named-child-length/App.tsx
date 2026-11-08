import { Show, createSignal } from "solid-js";

// A named function of `length` 0 is `Show`'s value, not a callback: `Show`
// returns it and the handler runs it, with no strict read in Chrome (rc.21).
export function NamedValue() {
  const [count] = createSignal(1);
  const value = () => {
    count();
  };
  const shown = (<Show when={1}>{value as any}</Show>) as any;
  return <button onClick={() => shown()()}>go</button>;
}

// A default parameter does not count toward `length`.
export function NamedDefaultParameter() {
  const [count] = createSignal(1);
  const value = (_unused = 1) => {
    count();
  };
  const shown = (<Show when={1}>{value as any}</Show>) as any;
  return <button onClick={() => shown()()}>go</button>;
}

// With a parameter `Show` calls it under `untrack`: a strict read.
export function NamedCallback() {
  const [count] = createSignal(1);
  const callback = (_item: any) => {
    count();
  };
  return <Show when={1}>{callback as any}</Show>;
}

// The inline twin of the first case.
export function InlineValue() {
  const [count] = createSignal(1);
  const shown = (
    <Show when={1}>
      {
        (() => {
          count();
        }) as any
      }
    </Show>
  ) as any;
  return <button onClick={() => shown()()}>go</button>;
}

// A value run in the component body reads a prop strictly (rc.21, Chrome): the
// child's position proves no run, so the read stays uncertifiable.
export function NamedValueInBody(props: { name: number }) {
  const value = () => {
    props.name;
  };
  const shown = (<Show when={1}>{value as any}</Show>) as any;
  shown()();
  return <span />;
}

// Reviewer R57: the inline twin of the case above. `Show` returns the value
// and the body runs it: the same strict read.
export function InlineValueInBody(props: { name: number }) {
  const shown = (
    <Show when={1}>
      {
        (() => {
          props.name;
        }) as any
      }
    </Show>
  ) as any;
  shown()();
  return <span />;
}

// Reviewer R59: a plain function's return is no insert: `make`'s caller runs
// the value in its body, a strict read (Chrome).
function make(props: { name: number }) {
  return (
    <Show when={1}>
      {
        (() => {
          props.name;
        }) as any
      }
    </Show>
  );
}
export function ReturnedByPlainFunction(props: { name: number }) {
  (make(props) as any)()();
  return <span />;
}

// Reviewer R59: part of a returned value, not the value itself.
function makeHolder(props: { name: number }) {
  return {
    s: (
      <Show when={1}>
        {
          (() => {
            props.name;
          }) as any
        }
      </Show>
    ),
  };
}
export function ReturnedInObject(props: { name: number }) {
  (makeHolder(props).s as any)()();
  return <span />;
}

// Reviewer R62: a component element's children are a prop it may call in its
// body: no insert (Chrome: a strict read in Comp's body).
function Comp(props: any) {
  props.children()();
  return <span />;
}
export function ComponentParent(props: { name: number }) {
  return (
    <Comp>
      {
        <Show when={1}>
          {
            (() => {
              props.name;
            }) as any
          }
        </Show>
      }
    </Comp>
  );
}

// Reviewer R62: a held fragment is a value (an array): its child runs where
// the body calls it.
export function HeldFragment(props: { name: number }) {
  const f = (
    <>
      <span />
      <Show when={1}>
        {
          (() => {
            props.name;
          }) as any
        }
      </Show>
    </>
  ) as any;
  f[1]()();
  return <span />;
}

// Inserted, and the tracked role stands: an intrinsic parent, and a
// fragment a component returns.
export function IntrinsicParent(props: { name: number }) {
  return (
    <div>
      <Show when={1}>
        {
          (() => {
            props.name;
          }) as any
        }
      </Show>
    </div>
  );
}
export function ReturnedFragment(props: { name: number }) {
  return (
    <>
      <Show when={1}>
        {
          (() => {
            props.name;
          }) as any
        }
      </Show>
    </>
  );
}
