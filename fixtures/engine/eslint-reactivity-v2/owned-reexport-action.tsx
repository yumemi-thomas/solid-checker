import { reAction, reMemo } from "./solid-reexports";
const save = reAction(function* () {});
reMemo(() => save());
