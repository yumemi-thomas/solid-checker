import { put } from "./put";
window.addEventListener("resize", () => console.log("resized"));
const timer = window.setTimeout(() => console.log("later"), 0);
window.clearTimeout(timer);
put({}, "a", 1);
export {};
