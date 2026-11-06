import { assigned, box, iifeRead, iifeWrite, letRead } from "./HeldSlots";

export function HeldUses() {
  void iifeRead();
  iifeWrite(2);
  void letRead();
  void assigned();
  void box.r();
  box.setR(2);
  // A handler's write and a tracked read are legal: no row here.
  return <button onClick={() => box.setR(3)}>{letRead()}</button>;
}
