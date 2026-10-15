// Records every module URL the parity driver's threads resolve (Node only),
// so the gate can show nothing resolved outside the isolated install.
import { appendFileSync } from "node:fs";
import { registerHooks } from "node:module";

const log = process.env.WASM_PARITY_URL_LOG;
registerHooks({
  resolve(specifier, context, nextResolve) {
    const result = nextResolve(specifier, context);
    appendFileSync(log, `${result.url}\n`);
    return result;
  },
});
