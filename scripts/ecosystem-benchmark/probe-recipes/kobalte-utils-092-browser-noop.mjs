// Finite samples; observes own-global additions during calls only.
import { noop as subject } from "@kobalte/utils";
export async function runProbeSession(_session, harness) {
 harness.emit({marker:"call",kind:"call",phase:"enter"});
 const keys=Reflect.ownKeys(globalThis);
 if(subject()!==undefined) throw new Error("noop sample disagrees");
 if(Reflect.ownKeys(globalThis).some(key=>!keys.includes(key))) harness.emit({marker:"create-operation",kind:"call",phase:"enter"});
 harness.emit({marker:"call",kind:"call",phase:"exit"});
}
