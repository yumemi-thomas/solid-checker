// A classic script (no import or export): its top-level function is the
// global object's `setTimeout`, so the module's timer runs its callback now.
function setTimeout(f) {
  f();
  return 0;
}
