import { createContext, createSignal, Show, useContext } from "solid-js";

const Ctx = createContext<number>();

// ADR 1177: an `onLoad` under a `Show` a timer opens may run on an owned flush or as a delivery with no owner
// (SC4001 `owner-varies-by-caller`). With no owner Solid throws NO_OWNER_CONTEXT before it looks for a provider
// (Chrome: the delivery comes first), so SC4002's ContextNotFoundError is not proven: uncertifiable.
export function Varies() {
  const [t1, setT1] = createSignal(0, { ownedWrite: true });
  setTimeout(() => {
    setT1(1);
  }, 0);
  return (
    <div>
      <Show when={t1()}>
        <iframe
          data-act="b1"
          onLoad={() => {
            useContext(Ctx);
          }}
        />
      </Show>
    </div>
  );
}

// The read in the component body runs under the render's owner: ContextNotFoundError, an SC4002 violation.
export function Body() {
  useContext(Ctx);
  return <p>body</p>;
}

// ADR 0577: a function the body calls and a handler runs: the body's owned run comes first and throws ContextNotFoundError
// before the handler exists. SC4002 stays a violation (the read is no handler literal).
export function BodyAndHandler() {
  const read = () => useContext(Ctx);
  read();
  return <button onClick={read}>go</button>;
}
