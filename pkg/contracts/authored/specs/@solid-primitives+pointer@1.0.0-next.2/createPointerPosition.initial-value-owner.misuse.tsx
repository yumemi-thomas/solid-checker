import { onSettled } from "solid-js";
import { createPointerPosition, type PointerStateWithActive } from "@solid-primitives/pointer";
const initial: PointerStateWithActive = {
  x: 0, y: 0, pointerId: 0, pressure: 0, tiltX: 0, tiltY: 0,
  width: 0, height: 0, twist: 0, pointerType: null, isActive: false
};
export default function App() {
  // A callable with the required data fields satisfies the real object type.
  const value = Object.assign(() => initial, initial);
  onSettled(() => {
    try { createPointerPosition({ value }); } catch { /* PRIMITIVE_IN_FORBIDDEN_SCOPE from function-form state; no listeners were reached. */ }
  });
  return <p>ready</p>;
}
