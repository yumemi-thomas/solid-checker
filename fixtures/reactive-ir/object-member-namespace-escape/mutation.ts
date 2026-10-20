import * as ns from "./source";
import { replace } from "opaque-mutate";
replace(ns["timer"], { useTicker() {} });
replace(ns["reader"], { readCount() { return 123; } });
replace(ns["callbacks"], { invoke(_fn: () => void) {} });
