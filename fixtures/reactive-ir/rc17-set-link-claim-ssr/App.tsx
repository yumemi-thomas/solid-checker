// @solidjs/web@2.0.0-rc.17's setLinkClaim(handler): the client builds ignore
// the handler (dist/web.dev.js:534); the server builds store it on the live
// render context and call it while serializing links (dist/server.js:4125,
// :3508-3512). Only server rendering runs it, so the scoped SC9014 gap is due
// unless the analysis declares client-only rendering.
import { createSignal } from "solid-js";
import { setLinkClaim } from "@solidjs/web";

const [active] = createSignal("/");

export function Links() {
  setLinkClaim((attrs) => (attrs.href === active() ? " data-active" : ""));
  return null;
}
