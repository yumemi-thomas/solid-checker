import { GET } from "@solidjs/web/server-functions";
import { query } from "@solidjs/router";

// Negative: without a module-level directive nothing here is a directive file.
export const a = GET(async () => 1);
export const b = query(async () => 2, "b");
export const LIMIT = 10;
export { helper } from "./mine";
