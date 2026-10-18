// A script, not a module: no import or export, so its top-level
// declarations are globals any file can call.
function relayGlobal(callback: () => void) {
  return { onClick: () => callback(), now: () => callback() };
}
function fireGlobal(run: () => void) {
  run();
}
