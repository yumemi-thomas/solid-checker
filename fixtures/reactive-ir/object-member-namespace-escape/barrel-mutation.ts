import * as barrel from "./barrel";
import { replace } from "opaque-mutate";
replace(barrel["timer"], { useTicker() {} });
