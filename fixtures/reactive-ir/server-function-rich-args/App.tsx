// SC7007 server-function-rich-argument, probed on the rc.0 server-functions
// client: Date/Map/Set/RegExp/typed-array arguments throw the directed
// rich-args error at the default transport; a lone or trailing Uint8Array is
// a natural HTTP body and does not; unresolvable types stay silent.
import {
  analyze,
  appendChunk,
  saveBoxed,
  saveCounts,
  saveEvent,
  saveIds,
  savePlain,
  saveStamps,
  uploadChunk,
} from "./api";
import type { Boxed, Ids, Stamps } from "./api";
import { recordPattern } from "./server-module";

export function Toolbar() {
  const when = new Date();
  const tags = new Set<string>();
  const counts = new Map<string, number>();
  const pattern = /todo/;
  const bytes = new Uint8Array(8);
  const samples = new Float64Array(4);
  const payload = { title: "hello" };
  // Aliased and imported spellings of the same runtime values. `Stamps`,
  // `Ids`, and `Boxed` each render as their own name, so no text match was
  // possible; the compiler resolves all three to what they actually are.
  const stamps: Stamps = [when];
  const ids: Ids = tags;
  const boxed: Boxed = { title: "hello", when };
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
        await saveStamps(stamps); // finding: Date behind an imported alias
        await saveIds(ids); // finding: Set behind an imported alias
        await saveBoxed(boxed); // silent: the Date is a nested property, not top level
        await saveEvent(new Date(), tags); // first argument inline: unresolvable, silent; tags still reported
      }}
    >
      save
    </button>
  );
}

declare function label(): string;
