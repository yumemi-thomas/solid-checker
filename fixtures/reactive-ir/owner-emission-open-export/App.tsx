import { onCleanup } from "solid-js";
import { render } from "@solidjs/web";

// ADR 0970, R2 and R3 of the re-review of #221. The program is OPEN (`.solid-checker/runtime.json`: a package whose entry other code imports): the namespace census (ADR 0347) is a
// closed program's, and code the program does not hold may call any export. An exported component a local tag renders was taken for entered by that render alone, so its owner
// operation was certified, though an importer's `Exported()` runs it with no owner (Chrome rc.21: NO_OWNER_CLEANUP; oracle cases owner-emission-open-importer and
// owner-emission-open-namespace). The entry census asks the boundary for an export now, as the read walks do (`entered_by_unseen_code`, ADR 0934).

// R3. Exported, rendered by a tag, no file of the program calls it.
export function Exported() {
  onCleanup(() => {});
  return <p>exported</p>;
}

// Control. Not exported: no importer reaches it, so rendering is the only entry.
function Local() {
  onCleanup(() => {});
  return <p>local</p>;
}

function App() {
  return (
    <p>
      <Exported />
      <Local />
    </p>
  );
}
render(() => <App />, document.body);
