import { reMemo, reSignal } from "./solid-reexports";
const [, setValue] = reSignal(0);
reMemo(() => setValue(1));
