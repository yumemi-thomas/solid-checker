import { onCleanup } from "solid-js";
const Object = { freeze<T>(value: T): T { return value; } };
export const fakeFrozen = Object.freeze({ fakeRun() { onCleanup(() => {}); } });
