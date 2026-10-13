import { startClosed } from "reactive-package";
startClosed(); // loaded module
export default function UncalledDefault() { startClosed(); /* uncalled dynamic default */ }
