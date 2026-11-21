import { createStore } from "solid-js";

// ADR 1178's premise (ADRs 0840, 1129): the program replaces `Object.values`, so the call may run project code with the store as
// its argument, where no row stands: the reference keeps its row. The array spread runs no replaced member.
Object.values = ((value: object) => [value]) as typeof Object.values;

function Patched() {
  const [row] = createStore({ a: { v: 1 } });
  const [list] = createStore([1, 2]);
  return (
    <button
      onClick={() => {
        Object.values(row);
        void [...list];
      }}
    />
  );
}

export function Page() {
  return <Patched />;
}
