// SC7007 server-function-rich-argument, probed on the rc.0 server-functions
// client: Date/Map/Set/RegExp/typed-array arguments throw the directed
// rich-args error at the default transport; a lone or trailing Uint8Array is
// a natural HTTP body and does not; unresolvable types stay silent.
import { analyze, appendChunk, saveCounts, saveEvent, savePlain, uploadChunk } from "./api";
import { recordPattern } from "./server-module";

export function Toolbar() {
  const when = new Date();
  const tags = new Set<string>();
  const counts = new Map<string, number>();
  const pattern = /todo/;
  const bytes = new Uint8Array(8);
  const samples = new Float64Array(4);
  const payload = { title: "hello" };
  return (
    <button
      onClick={async () => {
        await saveEvent(when, tags); // two findings: Date, Set
        await saveCounts(counts); // finding: Map
        await recordPattern(pattern); // finding: RegExp (module-level directive)
        await analyze(samples, label()); // finding: Float64Array has no natural encoding
        await uploadChunk(bytes); // silent: lone Uint8Array is a request body
        await appendChunk("chunk", bytes); // silent: trailing Uint8Array after JSON-safe leading
        await savePlain(payload); // silent: plain JSON-safe object
        await saveEvent(new Date(), tags); // first argument inline: unresolvable, silent; tags still reported
      }}
    >
      save
    </button>
  );
}

declare function label(): string;
