import * as Web from "@solidjs/web";
import { createSignal } from "solid-js";
import { dynamic } from "@solidjs/web";

const [, setTag] = createSignal("div");

const namedSource = () => {
  setTag("span");
  return "span";
};

dynamic(namedSource);
Web.dynamic(() => {
  setTag("section");
  return "section";
});
