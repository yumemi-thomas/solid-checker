import { createSignal } from "solid-js";

const registry: Array<() => number> = [];
function register(f: () => number) {
  registry.push(f);
}

export function Child() {
  let total = 0;
  for (const f of registry) total += f();
  return <p>{total}</p>;
}

function rowProps(id: string) {
  return {
    onKeyDown() {
      console.log(id);
    },
  };
}

export function A() {
  const [count] = createSignal(0);
  const row = {
    onKeyDown() {
      register(function () {
        return count();
      });
    },
  };
  return <div {...row}>a</div>;
}

export function B() {
  const [count] = createSignal(0);
  const row = {
    ...rowProps("x"),
    onFocus() {
      register(function () {
        return count();
      });
    },
  };
  return <div {...row}>b</div>;
}

function useRow(read: () => number) {
  return {
    onKeyDown() {
      register(function () {
        return read();
      });
    },
  };
}

export function C() {
  const [count] = createSignal(0);
  const row = useRow(() => count());
  return <div {...row}>c</div>;
}
