import { createPlugin } from "seroval";

// A separate claim-addressed module: the private harness copies each entry
// with create-new semantics, including entries for sibling artifact cases.
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
