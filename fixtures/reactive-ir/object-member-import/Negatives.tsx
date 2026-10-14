import { mutable, written, replaced, assigned, computed, getter, spread, escaped, nullable, viaThis, remotelyWritten, sibling } from "./negative";
import { fakeFrozen } from "./fake-freeze";
import { values } from "./namespace";
export function Refused() {
  return <button onClick={async () => {
    await Promise.resolve();
    mutable.mutableRun(); written.writtenRun(); replaced.replacedRun(); assigned.assignedRun();
    computed.computedRun(); getter.getterRun(); spread.spreadRun(); escaped.escapedRun();
    values.mutable.mutableRun(); nullable?.nullableRun(); viaThis.thisRun();
    remotelyWritten.remoteRun(); fakeFrozen.fakeRun();
    sibling.exposeSibling(); sibling.siblingRun();
  }}>refused</button>;
}
