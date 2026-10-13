// Finite samples; observes own-global additions during calls only.
import { clamp as subject } from "@kobalte/utils";
export async function runProbeSession(_session, harness) {
 harness.emit({marker:"call",kind:"call",phase:"enter"});
 const keys=Reflect.ownKeys(globalThis);
 for (const [v, lo, hi, expected] of [[-1,0,10,0],[5,0,10,5],[11,0,10,10]]) { if (subject(v,lo,hi)!==expected) throw new Error("clamp sample disagrees"); }
 if(Reflect.ownKeys(globalThis).some(key=>!keys.includes(key))) harness.emit({marker:"create-operation",kind:"call",phase:"enter"});
 harness.emit({marker:"call",kind:"call",phase:"exit"});
}
