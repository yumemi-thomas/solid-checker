import {
  createServerReference,
  getServerFunctionMetadata,
  withMeta
} from "@solidjs/web/server-functions";

// Reviewed for @solidjs/web@2.0.0-rc.3, ./server-functions, browser ESM:
// server-functions/dist/client.js, sha256:
// f7e754c2119449c94a01b16760efe1cc9fd4bf53f3f17033392a07b4d6bd00a1.
// Register only exact audited claims; server conditions and rc.0 are not
// covered by this review. The finite samples can contradict parameter-0
// identity; the authenticated exhaustive return census remains the proof.
export async function runProbeSession(_session, harness) {
  for (const name of [undefined, "solid-checker:with-meta-named"]) {
    // The public constructor supplies the required package-owned brand.
    // Construction installs the RPC provider; never invoke the returned
    // callable, whose body would perform a server request.
    const input = createServerReference("solid-checker:with-meta", name);
    const initial = getServerFunctionMetadata(input);
    if (!initial || initial.name !== name) {
      throw new Error("withMeta probe did not construct a valid server reference");
    }

    const tag = "solid-checker:with-meta-sample";
    for (const patch of [
      { name: "first", probeTag: tag, revision: 1 },
      Object.freeze({ name: "second", revision: 2 })
    ]) {
      harness.emit({ marker: "call", kind: "call", phase: "enter" });
      const output = withMeta(input, patch);
      if (!Object.is(output, input)) {
        harness.emit({ marker: "return-outside-identity", kind: "call", phase: "enter" });
      }
      harness.emit({ marker: "call", kind: "call", phase: "exit" });

      // Refuse a vacuous sample: observe an added field, its preservation,
      // and an overwritten field through the public metadata accessor.
      const observed = getServerFunctionMetadata(input);
      if (!observed || observed.probeTag !== tag ||
          observed.name !== patch.name || observed.revision !== patch.revision) {
        throw new Error("withMeta probe did not observe the metadata update");
      }
    }
  }
}
