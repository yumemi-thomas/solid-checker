// `@solid-devtools/ext-adapter@0.17.0`'s shape: a value import and a top-level
// call, and no export at all. Its export surface is empty, which is exactly why
// an empty-surface rule cleared it and had to be reverted. The emission premise
// refuses it on the import alone.
import { start } from "./start.js";

start();
