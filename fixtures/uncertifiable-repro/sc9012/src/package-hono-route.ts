// sensor-sim packages/server/src/routes/status.ts:12 (and 23 more route
// handlers): a route callback handed to `Hono#get` calls a method of its
// context parameter `c`. The handler is an exported value a package's router
// runs, so no project call site fixes which implementation `c.json` selects.
import { Hono } from "hono";

export const statusApp = new Hono().get("/", (c) => c.json({ status: "ok" }));
