// The worker the owned checker runs in. See owned-worker-core.mjs.
import { parentPort } from "node:worker_threads";
import { serve } from "./owned-worker-core.mjs";

serve(parentPort);
