import { reMemo, reRefresh } from "./solid-reexports";
const value = reMemo(() => 1);
reMemo(() => reRefresh(value));
