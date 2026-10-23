import { createSignal } from "solid-js";

const [count] = createSignal(0);

// Positive: rendered by its tag and by an alias that only tags name
// (`createComponent(Badge, {})`).
function Aliased() {
  const value = count();
  return <b>{value}</b>;
}
const Badge = Aliased;

// Uncertifiable: the alias is also called plainly in tracked JSX.
function AliasCalled() {
  const value = count();
  return <b>{value}</b>;
}
const Called = AliasCalled;

// Uncertifiable: an exported alias can be imported and called anywhere.
function AliasExported() {
  const value = count();
  return <b>{value}</b>;
}
export const Exported = AliasExported;

export function AliasPage() {
  return (
    <div>
      <Aliased />
      <Badge></Badge>
      <AliasCalled />
      <Called />
      {Called()}
      <AliasExported />
      <Exported />
    </div>
  );
}
