import "./mutation";
import { timer, reader } from "dep-object";
export function App() {
  const n = reader.readCount();
  return <button onClick={async () => {
    await Promise.resolve();
    timer.useTicker();
  }}>{n}</button>;
}
