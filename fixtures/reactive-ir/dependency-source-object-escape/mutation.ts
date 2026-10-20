import * as ns from "./barrel";
import { replace } from "opaque-mutate";
replace(ns.timer, { useTicker() {} });
replace(ns.reader, { readCount() { return 0; } });
