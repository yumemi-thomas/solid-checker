// The enableRichArguments twin: called at module scope in a component file
// with no render, so nothing proves it runs on the client before the call
// below (ADR 0707): SC7007 is uncertifiable, never a violation.
// server-function-rich-args-entry is the proven twin.
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
