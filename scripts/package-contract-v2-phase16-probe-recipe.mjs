// Deterministic current-protocol recipe used only to measure the isolated
// runtime-probe process envelope. Rust semantic evaluation and proof authority
// remain separate; this witness cannot create closure or acceptance.

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "phase16-current-probe", kind: "callback", ordinal: 0 });
}
