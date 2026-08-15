// The enableRichArguments twin: the rich-args entry is imported (and called
// at startup), which installs the codec's write half as serializeArgs — the
// transport throw is gone everywhere (probed), so SC7007 stays silent.
import { enableRichArguments } from "@solidjs/web/server-functions/rich-args";
import { saveEvent } from "./api";

enableRichArguments();

export function Toolbar() {
  const when = new Date();
  const tags = new Set<string>();
  return (
    <button onClick={() => saveEvent(when, tags)}>save</button>
  );
}
