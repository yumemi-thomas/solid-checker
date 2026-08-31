// `./notify.js` is shipped, and this entry imports it twice: once as a module,
// once through a bundler's `?raw` loader. The two bindings are not the same
// value -- the loader's product is the file's source text -- so only the module
// import may be walked into.
import notifySource from "./notify.js?raw";
import { notify } from "./notify.js";

// Whether this invokes the callback, and when, depends on a binding this
// artifact case cannot know. It must stay open, not proven and not refused.
export function runOpaque(callback) {
  notifySource(callback);
}

// The same shipped file, imported as a module: the callback runs on this stack.
export function runModule(callback) {
  notify(callback);
}
