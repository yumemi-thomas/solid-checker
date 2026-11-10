import { createMemo } from "solid-js";
import arrayed from "./DxArray";
import object from "./DxObject";
import created from "./DxCreated";

// Positive: each reads the default export's accessor through a throw, in
// another module, called from a component body (Chrome: STRICT_READ_UNTRACKED
// at the `e()` call). The row stands at the call.
export const readArray = () => { try { throw arrayed[0]; } catch (e) { void (e as () => number)(); } };
export const readObject = () => { try { throw object.c; } catch (e) { void (e as () => number)(); } };
export const readCreated = () => { try { throw created; } catch (e) { void (e as () => number)(); } };

// Negative: the same reads in a memo's compute, a tracked read: no row at
// the call (the `throw` keeps its own).
export const trackedArray = () => createMemo(() => { try { throw arrayed[0]; } catch (e) { return (e as () => number)(); } });
export const trackedCreated = () => createMemo(() => { try { throw created; } catch (e) { return (e as () => number)(); } });
