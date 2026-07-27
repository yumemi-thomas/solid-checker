import { createDevices } from "@solid-primitives/devices";

const devices = createDevices();

export function Good() {
  return <div>{devices().length}</div>;
}

export function Bad() {
  const count = devices().length;
  return <div>{count}</div>;
}
