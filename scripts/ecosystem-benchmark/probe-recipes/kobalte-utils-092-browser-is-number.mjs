// Finite samples; observes own-global additions during calls only.
import { isNumber as subject } from "@kobalte/utils";
export async function runProbeSession(_session, harness) {
 harness.emit({marker:"call",kind:"call",phase:"enter"});
 const keys=Reflect.ownKeys(globalThis);
 for (const [value,expected] of [[0,true],[NaN,true],[Infinity,true],["1",false],[null,false]]) { if(subject(value)!==expected) throw new Error("number sample disagrees"); }
 if(Reflect.ownKeys(globalThis).some(key=>!keys.includes(key))) harness.emit({marker:"create-operation",kind:"call",phase:"enter"});
 harness.emit({marker:"call",kind:"call",phase:"exit"});
}
