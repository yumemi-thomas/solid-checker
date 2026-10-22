// An admitted browser host (ADR 0270) proves this module runs in the browser,
// where rc.17's `setLinkClaim` ignores its handler (dist/web.dev.js:534). It
// does not prove the sources are never server-rendered: a script that never
// names Vite, or another program importing them, may call `renderToString`,
// which runs the handler (dist/server.js:4125, :3508-3512). The gap stays.
import { setLinkClaim } from "@solidjs/web";

setLinkClaim((attrs) => (attrs.href === "/" ? " data-active" : ""));
