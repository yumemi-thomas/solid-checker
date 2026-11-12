import { onCleanup } from "solid-js";

// ADR 0970, R3 of the re-review of #221: an OPEN program (a publishing manifest), the package's entry. `Exported` is exported and a tag renders it, and no file of the program calls
// it. `importer.tsx` is loaded by the page and is no part of the program (oracle.json `external`): an importer the package cannot see calls `Exported` with no owner and Chrome
// rc.21 reports NO_OWNER_CLEANUP. The owner emission certified it (an exported component a local tag renders was closed in an open program, though a function no tag renders was open).
// `Local` is no export: no importer reaches it, and it is quiet.
export function Exported() {
  onCleanup(() => {});
  return <p>exported</p>;
}
function Local() {
  onCleanup(() => {});
  return <p>local</p>;
}
export function Host() {
  return (
    <p>
      <Exported />
      <Local />
    </p>
  );
}
