// Negative: a default parameter initializer runs before the body and may
// throw. `helper()` runs before `late` exists, so `late()` throws
// ReferenceError in the default and the body never runs; the class variant
// constructs `Later` before its declaration. A default that is more than a
// literal withholds every mention that would reach the body.
import { createSignal } from "solid-js";

const [, setCount] = createSignal(0);
export function App() {
  const helper = (value = late()) => {
    setCount(1);
  };
  helper();
  const late = () => 0;
  return <button />;
}
export function ClassDefault() {
  const helper = (value = new Later()) => {
    setCount(2);
  };
  helper();
  class Later {}
  return <button />;
}
export function Destructured() {
  const helper = ({ a = late() } = {}) => {
    setCount(3);
  };
  helper();
  const late = () => 0;
  return <button />;
}
// Positive: a literal default cannot throw, so the body is proven to run.
export function Control() {
  const helper = (value = 0) => {
    setCount(4);
  };
  helper();
  return <button />;
}
