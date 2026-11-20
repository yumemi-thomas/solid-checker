import { CalledDirectly, CalledOnAny, TemplateSubstitution, Unwritten } from "./App";
// @ts-ignore
import polyfill from "fz-prealm-any/index.js?url";

// The URL of an installed script, put in a script element: the page runs it, and no import names it.
const script = document.createElement("script");
script.type = "module";
script.src = polyfill;
document.head.append(script);

export const product = [CalledDirectly, CalledOnAny, TemplateSubstitution, Unwritten];
