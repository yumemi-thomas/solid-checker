import { makeSize } from "reactive-package";

Object.prototype.toJSON = () => null;
declare global {
  interface Object { toJSON?: () => null; }
}

export function Serialize() {
  const location = makeSize();
  const text = JSON.stringify(location);
  return <div>{text}</div>;
}
