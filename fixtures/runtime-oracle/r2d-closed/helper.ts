// A helper whose call sites the walk counts; it is also the module's default
// export, so another module can call it anywhere.
function runIt(f: () => void) {
  return () => f();
}
export default runIt;
