// vitest's `setupFiles`: the test process loads the writers (the any-name one by its file), the page never does.
import "fz-prealm-named";
// @ts-ignore
import "./node_modules/fz-prealm-any/index.js";
