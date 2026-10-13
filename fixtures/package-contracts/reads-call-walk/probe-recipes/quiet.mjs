// Test-only veto for the graph half of `reads_call_walk_*`: it runs nothing
// and observes nothing, so the verdict is the census's and the dependency
// receipt's alone. It is copied into a scratch corpus by the test and is never
// part of a real corpus, where a veto that cannot see a read would be the
// vacuous gate ADR 0163 exists to replace.
export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
