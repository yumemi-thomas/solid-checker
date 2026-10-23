import { createSignal } from "solid-js";
import { Dynamic } from "@solidjs/web";

const [count] = createSignal(0);

// Uncertifiable: the map is also read as `called.run()`, a plain call in
// tracked JSX, where no strict-read window opens.
function Run() {
  const value = count();
  return <b>{value}</b>;
}
const called = { run: Run };

// Uncertifiable: an exported map can be imported and called anywhere.
function Shared() {
  const value = count();
  return <b>{value}</b>;
}
export const shared = { shared: Shared };

// Uncertifiable: a spread copies the map's values into an object nothing
// censuses.
function Spread() {
  const value = count();
  return <b>{value}</b>;
}
const spread = { spread: Spread };
export const copied = { ...spread };

// Uncertifiable: a getter runs project code on every read.
function Gotten() {
  const value = count();
  return <b>{value}</b>;
}
const gotten = {
  get gotten() {
    return Gotten;
  },
};

// Uncertifiable: the stored value is handed to a project callee.
function Handed() {
  const value = count();
  return <b>{value}</b>;
}
const handed = { handed: Handed };
function keep(value: unknown) {
  return value;
}
keep(handed.handed);

export function Escapes(props: { key: "run" }) {
  return (
    <div>
      <Run />
      <Shared />
      <Spread />
      <Gotten />
      <Handed />
      <Dynamic component={called[props.key]} />
      {called.run()}
      <Dynamic component={shared.shared} />
      <Dynamic component={spread.spread} />
      <Dynamic component={gotten.gotten} />
      <Dynamic component={handed.handed} />
    </div>
  );
}
