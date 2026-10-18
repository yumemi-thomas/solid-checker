// The module control: the same functions, exported.
export function relayModule(callback: () => void) {
  return { onClick: () => callback(), now: () => callback() };
}
export function fireModule(run: () => void) {
  run();
}

// A module function attached to `globalThis`: callable from anywhere by that
// name, though no reference in the program calls it.
export function relayThroughGlobal(callback: () => void) {
  (globalThis as unknown as { fireStored?: () => void }).fireStored = () => callback();
  return { onClick: () => callback() };
}
