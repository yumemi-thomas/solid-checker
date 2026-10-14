import { timer, reader } from "dep-object";
import { vetoed } from "dep-veto";
import { local } from "./local";
export function MissingOwner() {
  return <button onClick={async () => {
    await Promise.resolve();
    timer.packageTicker(() => {});
    vetoed.vetoedTicker(() => {});
    local.localTicker(() => {});
  }}>start</button>;
}
export function Reads() { const n = reader.packageRead(); return <p>{n}</p>; }
export function Owned() { timer.packageTicker(() => {}); return <p>owned</p>; }
