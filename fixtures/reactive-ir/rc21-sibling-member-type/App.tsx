import { createContext, createMemo, createSignal, useContext, type Accessor } from "solid-js";
import { render } from "@solidjs/web";

type Api = { state: Accessor<number>; go: (n: number) => void };
const Sibling = createContext<Api>({ state: () => 0, go: () => {} });
const Alone = createContext<{ go: (n: number) => void }>({ go: () => {} });
const Reads = createContext<{ state: Accessor<number> }>({ state: () => 0 });

// `go` sits beside an accessor: the sibling's type must not make it a read.
function WithSibling() {
  const api = useContext(Sibling);
  const v = createMemo(() => {
    api.go(1);
    return api.state();
  });
  return <p>{v()}</p>;
}

// The same call with no sibling, the control.
function NoSibling() {
  const api = useContext(Alone);
  const v = createMemo(() => {
    api.go(1);
    return 1;
  });
  return <p>{v()}</p>;
}

// The accessor itself stays what it was.
function OnlyRead() {
  const api = useContext(Reads);
  const v = createMemo(() => api.state());
  return <p>{v()}</p>;
}

function App() {
  const [s, setS] = createSignal(0);
  return (
    <Sibling value={{ state: s, go: setS }}>
      <Alone value={{ go: setS }}>
        <Reads value={{ state: s }}>
          <WithSibling />
          <NoSibling />
          <OnlyRead />
        </Reads>
      </Alone>
    </Sibling>
  );
}

render(() => <App />, document.body);
