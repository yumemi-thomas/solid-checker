import * as S from "solid-js";
import { Errored, createEffect, createMemo, createTrackedEffect, onCleanup } from "solid-js";
import { render } from "@solidjs/web";

// ADR 1153: `Errored` calls its `fallback` function, written in place as the whole value of the prop, in a computation it creates under the owner that encloses the tag (rc.21
// `@solidjs/signals` `createBoundary`, `fallback(b)`: `runWithOwner(out._parent, () => computed(() => b._show(b), ...))`; `solid-js` `Errored`: `f(err, reset)`). So a creation site
// written directly in that function has an owner, whoever owns the tag, and the rows that read the owner of such a site (SC4001, and SC3001 and SC6002 where the program holds a leaf
// opener or an effect's apply) end in the silence they have for a `For` row. The cases that keep their rows are the ones that are not that function, or may run elsewhere.
// Chrome: fixtures/runtime-oracle/errored-fallback-* (quiet where the edge applies; NO_OWNER_CLEANUP, covered, where it does not).

function Broken(): never {
  throw new Error("boom");
}

// Applies: the textbook shape (`id-async-memo-loading`), a render effect for the dynamic child.
function Message() {
  return (
    <Errored fallback={(error) => <p>{String(error())}</p>}>
      <Broken />
    </Errored>
  );
}

// Applies: parentheses around the literal are no other function.
function Parenthesized() {
  return (
    <Errored fallback={((error) => <p>{String(error())}</p>)}>
      <Broken />
    </Errored>
  );
}

// Applies: the primitives a computation owns, created directly in the function.
function Creates() {
  return (
    <Errored
      fallback={(error) => {
        onCleanup(() => {});
        const label = createMemo(() => String(error()));
        createEffect(
          () => label(),
          () => {},
        );
        return <p>{label()}</p>;
      }}
    >
      <Broken />
    </Errored>
  );
}

// Applies: a callee only the fallback calls runs in the fallback's computation.
function release() {
  onCleanup(() => {});
}
function ViaHelper() {
  return (
    <Errored
      fallback={() => {
        release();
        return <p>helper</p>;
      }}
    >
      <Broken />
    </Errored>
  );
}

// Applies: the namespace form names the same exported primitive.
function ViaNamespace() {
  return (
    <S.Errored fallback={(error) => <p>{String(error())}</p>}>
      <Broken />
    </S.Errored>
  );
}

// Applies: a named function expression that never uses its name cannot hand itself on.
function NamedQuiet() {
  return (
    <Errored
      fallback={function show(error) {
        return <p>{String(error())}</p>;
      }}
    >
      <Broken />
    </Errored>
  );
}

// Keeps its row: a spread. `props.fallback` is a merge, and that this literal is what the tag calls is not proved.
const rest = {};
function Spread() {
  return (
    <Errored {...rest} fallback={(error) => <p>{String(error())}</p>}>
      <Broken />
    </Errored>
  );
}

// Keeps its row: an `async` function. The code after its first `await` runs with no owner (Chrome rc.21: NO_OWNER_CLEANUP), a route the owner graph's edge does not show.
function Async() {
  return (
    <Errored
      fallback={async (error) => {
        await Promise.resolve();
        onCleanup(() => {});
        return <p>{String(error())}</p>;
      }}
    >
      <Broken />
    </Errored>
  );
}

// Keeps its row: a function written elsewhere and handed by name is a value the tag may call or not, and has other callers.
const show = (error: () => unknown) => <p>{String(error())}</p>;
function ByName() {
  return (
    <Errored fallback={show}>
      <Broken />
    </Errored>
  );
}

// Keeps its row: a function that is not the whole value of the prop.
function Conditional(props: { flag: boolean }) {
  return (
    <Errored fallback={props.flag ? (error) => <p>{String(error())}</p> : <p>static</p>}>
      <Broken />
    </Errored>
  );
}

// Keeps its row: a named function expression that stores itself runs where it is called (ADR 0971, B1).
let pickFallback: ((error: () => unknown, reset: () => void) => unknown) | undefined;
function NamedSelf() {
  return (
    <Errored
      fallback={function fb(error: () => unknown, _reset: () => void) {
        pickFallback = fb;
        return <p>{String(error())}</p>;
      }}
    >
      <Broken />
    </Errored>
  );
}

// Keeps its row: the layers written inside the fallback are their own. A primitive created in an effect's apply raises PRIMITIVE_IN_EFFECT_CALLBACK and an `onCleanup` in a tracked
// effect's callback CLEANUP_IN_FORBIDDEN_SCOPE (Chrome rc.21, fixtures/runtime-oracle/errored-fallback-nested); only the creation of the two effects, written directly in the fallback, is owned.
function Nested() {
  return (
    <Errored
      fallback={() => {
        createEffect(
          () => 0,
          () => {
            createMemo(() => 1);
          },
        );
        createTrackedEffect(() => {
          onCleanup(() => {});
        });
        return <p>nested</p>;
      }}
    >
      <Broken />
    </Errored>
  );
}

// Keeps its row: a user component's `fallback` prop is a function its own code calls, anywhere (here in a handler, with no owner).
function Guard(props: { fallback: () => unknown; children?: unknown }) {
  return (
    <button type="button" onClick={() => props.fallback()}>
      {props.children}
    </button>
  );
}
function UserComponent() {
  return (
    <Guard
      fallback={() => {
        onCleanup(() => {});
        return <p>user</p>;
      }}
    >
      <p>child</p>
    </Guard>
  );
}

// Keeps its row: a local component of the same name is not the dialect's primitive.
function Shadowed() {
  function Errored(props: { fallback: () => unknown; children?: unknown }) {
    return (
      <button type="button" onClick={() => props.fallback()}>
        {props.children}
      </button>
    );
  }
  return (
    <Errored
      fallback={() => {
        onCleanup(() => {});
        return <p>shadowed</p>;
      }}
    >
      <p>child</p>
    </Errored>
  );
}

function Host() {
  createEffect(
    () => 0,
    () => {
      pickFallback!(() => 0, () => {});
    },
  );
  return <p>host</p>;
}

function App() {
  return (
    <p>
      <Message />
      <Parenthesized />
      <Creates />
      <ViaHelper />
      <ViaNamespace />
      <NamedQuiet />
      <Spread />
      <Async />
      <ByName />
      <Conditional flag={true} />
      <NamedSelf />
      <Nested />
      <UserComponent />
      <Shadowed />
      <Host />
    </p>
  );
}
render(() => <App />, document.body);
