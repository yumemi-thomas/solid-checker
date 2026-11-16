// A re-export of the runner's registration: the symbol's alias chain ends in vitest's declaration (ADR 0438), so a callback handed to it is the runner's.
export { it as spec } from "vitest";
