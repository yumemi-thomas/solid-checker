import { createPlugin } from "seroval";

// Finite contradiction test for the exact whole-parameter return claim.
// The package receives plugin values only, never the transcript interfaces.
export async function runProbeSession(_session, harness) {
  const plugin = () => ({
    tag: "solid-checker:identity-probe",
    test: () => false,
    parse: {},
    serialize: () => "null",
    deserialize: () => null
  });
  for (const input of [plugin(), Object.freeze(plugin())]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const output = createPlugin(input);
    if (!Object.is(output, input)) {
      harness.emit({ marker: "return-outside-identity", kind: "call", phase: "enter" });
    }
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
}
