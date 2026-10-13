// `@solidjs/diagnostics@2.0.0-rc.3`'s `./vitest`: a matcher-registration module
// whose whole purpose is an import-time side effect, behind an *optional* peer
// dependency. Side-effect-only by design, and still a refusal.
import { expect } from "vitest";

expect.extend({});
