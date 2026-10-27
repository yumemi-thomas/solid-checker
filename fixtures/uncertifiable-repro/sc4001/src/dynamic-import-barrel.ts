// The barrel re-exports dynamic-import-holder.ts, and its loader reads one name of it with a dynamic
// import (solid-relay's tests read `useRelayEnvironment` of the package index so): the name is not a
// function the barrel defines, so the load hands out the whole barrel.
export { createLoader, createPlain } from "./dynamic-import-holder";

export async function loadPlain() {
  const { createPlain } = await import("./dynamic-import-barrel");
  return createPlain;
}
