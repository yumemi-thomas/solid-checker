// SC7007 server-function-rich-argument, probed on the rc.0 server-functions
// client: Date/Map/Set/RegExp/typed-array arguments throw the directed
// rich-args error at the default transport; a lone or trailing Uint8Array is
// a natural HTTP body and does not; unresolved JSON safety stays explicit.
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

// Same spelling and same property shape as the real configuration API, but a
// local function. It must not suppress any SC7007 finding.
function configureServerFunctionsClient(_options: {
  serializeArgs: (args: unknown[]) => string;
}) {}
configureServerFunctionsClient({ serializeArgs: JSON.stringify });

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
        await analyze(samples, label()); // violation for Float64Array; label result uncertifiable
        await uploadChunk(bytes); // silent: lone Uint8Array is a request body
        await appendChunk("chunk", bytes); // silent: trailing Uint8Array after JSON-safe leading
        await savePlain(payload); // uncertifiable: the available fact cannot close the object graph
        await saveStamps(stamps); // finding: Date behind an imported alias
        await saveIds(ids); // finding: Set behind an imported alias
        await saveBoxed(boxed); // uncertifiable: nested Date is outside the top-level fact
        await saveEvent(new Date(), tags); // two findings: inline Date, Set
      }}
    >
      save
    </button>
  );
}

declare function label(): string;
