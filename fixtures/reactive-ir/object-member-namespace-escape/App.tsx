import "./mutation";
import { timer, reader } from "./source";
export function App() {
  const n = reader.readCount();
  return <button onClick={async () => {
    await Promise.resolve();
    timer.useTicker();
  }}>{n}</button>;
}
