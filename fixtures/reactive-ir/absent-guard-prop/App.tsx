import { createSignal, merge } from "solid-js";

const [n] = createSignal(1);
const extra = JSON.parse('{"on":true}') as { on?: boolean };
type Props = { run?: () => number; on?: boolean };

function Absent(p: Props) {
  if (p.on) p.run!();
  return <p />;
}
function Passed(p: Props) {
  if (p.on) p.run!();
  return <p />;
}
function Spread(p: Props) {
  if (p.on) p.run!();
  return <p />;
}
export function Exported(p: Props) {
  if (p.on) p.run!();
  return <p />;
}
function Defaulted(raw: Props) {
  const p = merge({ on: true }, raw);
  if (p.on) p.run!();
  return <p />;
}
function Unknown(p: Props) {
  if (Math.random() > 2) p.run!();
  return <p />;
}

export function App() {
  return [
    <Absent run={() => n()} />,
    <Passed on run={() => n()} />,
    <Spread {...extra} run={() => n()} />,
    <Exported run={() => n()} />,
    <Defaulted run={() => n()} />,
    <Unknown run={() => n()} />,
  ];
}
