// A script, not a module: its declarations are globals every file can call.
function invokeIt(f: () => void) {
  f();
}
