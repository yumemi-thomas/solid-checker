// Generated dependency packages (the `packages` profile).
//
// A program's `helpers` may be package exports: `{ k: "helper", name, form: "p:<behaviour>:<exportForm>", pkg }`. The program imports
// the export from a package in its own `node_modules` (`fz-lib`), the package imports `solid-js` from the program's install (a peer,
// never a copy), and the checker reads the package's source the way a project's dependency is read (ADRs 0291, 0297, 0300). This file
// is the one place that knows what each behaviour does at run time and what the package looks like on disk; the model only knows the
// result shape, the printer only the callee text and the import line.
//
//   behaviour   what the exported function does with its callback `cb`            result
//   inline      calls it and returns its value                                    value
//   repeat      calls it 2 or 3 times, returns the last value                     value
//   timer       setTimeout(cb, 0)                                                 num
//   micro       queueMicrotask(cb)                                                num
//   then        Promise.resolve().then(cb)                                        num
//   untrack     untrack(cb), returns its value                                   value
//   ownerNull   runWithOwner(null, cb), returns its value                         value
//   ownerNow    runWithOwner(getOwner(), cb), returns its value                   value
//   root        createRoot(cb), returns its value                                 value
//   effectCompute createEffect(cb, () => {})                                      num
//   effectApply createEffect(() => 1, cb)                                         num
//   memo        createMemo(cb) (read once so that it computes)                    num
//   cleanup     onCleanup(cb)                                                     num
//   settled     onSettled(cb)                                                     num
//   never       does not call it                                                  num
//   returnCb    returns it                                                        fn
//   returnWrap  returns a wrapper that calls it                                   fn
//   storeObj    returns { cb }                                                    obj
//   register    stores it (module variable, array or object; the owner too, when `owned`), and a second export, `fire`, calls it later
//   fire        calls what `register` stored                                      num
//
// `wrap` hands the callback over inside a fresh arrow instead of directly. Direct passing is withheld where the callback would receive
// arguments or its return value would be read as something else (a createRoot dispose, an apply's cleanup).
// Export forms: named (function declaration), arrow (const), default, reexport (an index re-exporting `impl.js`), reexportRenamed,
// star (`export *`), class (instance method), static (static method), object (a method of a const object), namespace (`import * as`).
import { createHash } from "node:crypto";

export const PACKAGE_NAME = "fz-lib";
export const PKG_PREFIX = "p:";

export const EXPORT_FORMS = ["named", "arrow", "default", "reexport", "reexportRenamed", "star", "class", "static", "object", "namespace"];
/** Forms that define the function in `impl.js` and re-export it from the entry. */
const REEXPORTS = ["reexport", "reexportRenamed", "star"];
const CONTAINERS = ["class", "static", "object"];

/** result: how the call's value types in the program. direct: may the callback be handed over as it is. */
export const BEHAVIOURS = {
  inline: { result: "value", direct: true },
  repeat: { result: "value", direct: true },
  timer: { result: "num", direct: true },
  micro: { result: "num", direct: true },
  then: { result: "num", direct: true },
  untrack: { result: "value", direct: true },
  ownerNull: { result: "value", direct: true },
  ownerNow: { result: "value", direct: true },
  root: { result: "value", direct: false },
  effectCompute: { result: "num", direct: true },
  effectApply: { result: "num", direct: false },
  memo: { result: "num", direct: true },
  cleanup: { result: "num", direct: false },
  settled: { result: "num", direct: false },
  never: { result: "num", direct: true },
  returnCb: { result: "fn", direct: true },
  returnWrap: { result: "fn", direct: true },
  storeObj: { result: "obj", direct: true },
  register: { result: "num", direct: true },
  fire: { result: "fire", direct: true },
};

export const isPackageForm = (form) => typeof form === "string" && form.startsWith(PKG_PREFIX);
export const formOf = (behaviour, exportForm) => `${PKG_PREFIX}${behaviour}:${exportForm}`;
/** The behaviour of a package form (`p:timer:named` -> `timer`). */
export const behaviourOf = (form) => form.slice(PKG_PREFIX.length).split(":")[0];
export const resultOf = (form) => BEHAVIOURS[behaviourOf(form)]?.result;

const upper = (text) => text[0].toUpperCase() + text.slice(1);

/** The identifiers of a helper's export, from its number: `{ exportName, alias, container, member }`. */
export function namesOf(helper) {
  const { behaviour, exportForm } = helper.pkg;
  const number = helper.name.replace(/\D/g, "");
  const exportName = `${behaviour}${number}`;
  const alias = `${exportName}Alias`;
  if (!CONTAINERS.includes(exportForm)) return { exportName, alias, container: null, member: null };
  const container = helper.pkg.container ?? (exportForm === "object" ? `lib${number}` : `${upper(behaviour)}${number}`);
  return { exportName, alias, container, member: behaviour === "fire" ? "fire" : behaviour === "register" ? "register" : "run" };
}

/**
 * The callee text and import line of a helper.
 * A `fire` helper shares its register's container (`pkg.container`, set when it is created).
 */
export function accessOf(helper) {
  const { exportForm } = helper.pkg;
  const names = namesOf(helper);
  switch (exportForm) {
    case "default": return { callee: helper.name, importLine: `import ${helper.name} from "${PACKAGE_NAME}";` };
    case "namespace": return { callee: `ns.${names.exportName}`, importLine: `import * as ns from "${PACKAGE_NAME}";` };
    case "class": return { callee: `new ${names.container}().${names.member}`, importLine: `import { ${names.container} } from "${PACKAGE_NAME}";` };
    case "static": return { callee: `${names.container}.${names.member}`, importLine: `import { ${names.container} } from "${PACKAGE_NAME}";` };
    case "object": return { callee: `${names.container}.${names.member}`, importLine: `import { ${names.container} } from "${PACKAGE_NAME}";` };
    case "reexportRenamed": return { callee: names.alias, importLine: `import { ${names.alias} } from "${PACKAGE_NAME}";` };
    default: return { callee: names.exportName, importLine: `import { ${names.exportName} } from "${PACKAGE_NAME}";` };
  }
}

/** The state declarations a `register` helper needs, and the statements of its body and its `fire`'s. */
function registerParts(helper) {
  const id = helper.name.replace(/\D/g, "");
  const state = `saved${id}`;
  const { store, owned } = helper.pkg;
  const decls = [];
  const own = owned ? `owner${id}` : null;
  if (owned) decls.push(`let ${own} = null;`);
  let save; let call;
  switch (store) {
    case "array": decls.push(`const ${state} = [];`); save = `${state}.push(cb);`; call = [`let last = 1;`, `for (const f of ${state}) last = ${owned ? `runWithOwner(${own}, f)` : "f()"};`, "return last;"]; break;
    case "object": decls.push(`const ${state} = { cb: null };`); save = `${state}.cb = cb;`; call = [`if (!${state}.cb) return 1;`, `return ${owned ? `runWithOwner(${own}, ${state}.cb)` : `${state}.cb()`};`]; break;
    default: decls.push(`let ${state} = null;`); save = `${state} = cb;`; call = [`if (!${state}) return 1;`, `return ${owned ? `runWithOwner(${own}, ${state})` : `${state}()`};`];
  }
  return { decls, registerBody: [...(owned ? [`${own} = getOwner();`] : []), save, "return 1;"], fireBody: call, uses: owned ? ["getOwner", "runWithOwner"] : [] };
}

/** The signature of a behaviour: generics, parameter list and return type, for the declaration file. */
function signatureOf(behaviour) {
  switch (BEHAVIOURS[behaviour].result) {
    case "value": return { generic: "<T>", params: "cb: () => T", ret: "T" };
    case "fn": return { generic: "<T>", params: "cb: () => T", ret: "() => T" };
    case "obj": return { generic: "<T>", params: "cb: () => T", ret: "{ cb: () => T }" };
    case "fire": return { generic: "", params: "", ret: "number" };
    default: return { generic: "", params: "cb: () => unknown", ret: "number" };
  }
}

/** The body lines and the solid-js names of a behaviour. */
function bodyOf(helper) {
  const { behaviour, wrap, n, store, owned } = helper.pkg;
  const spec = BEHAVIOURS[behaviour];
  const direct = spec.direct && !wrap;
  const value = direct ? "cb" : "() => cb()";
  const quiet = direct ? "cb" : "() => { cb(); }";
  switch (behaviour) {
    case "inline": return { lines: ["return cb();"], uses: [] };
    case "repeat": return { lines: ["let last;", `for (let i = 0; i < ${n ?? 2}; i += 1) last = cb();`, "return last;"], uses: [] };
    case "timer": return { lines: [`setTimeout(${quiet}, 0);`, "return 1;"], uses: [] };
    case "micro": return { lines: [`queueMicrotask(${quiet});`, "return 1;"], uses: [] };
    case "then": return { lines: [`Promise.resolve().then(${quiet});`, "return 1;"], uses: [] };
    case "untrack": return { lines: [`return untrack(${value});`], uses: ["untrack"] };
    case "ownerNull": return { lines: [`return runWithOwner(null, ${value});`], uses: ["runWithOwner"] };
    case "ownerNow": return { lines: [`return runWithOwner(getOwner(), ${value});`], uses: ["runWithOwner", "getOwner"] };
    case "root": return { lines: ["return createRoot(() => cb());"], uses: ["createRoot"] };
    case "effectCompute": return { lines: [`createEffect(${quiet}, () => {});`, "return 1;"], uses: ["createEffect"] };
    case "effectApply": return { lines: ["createEffect(() => 1, () => { cb(); });", "return 1;"], uses: ["createEffect"] };
    case "memo": return { lines: [`const m = createMemo(${quiet});`, "m();", "return 1;"], uses: ["createMemo"] };
    case "cleanup": return { lines: ["onCleanup(() => { cb(); });", "return 1;"], uses: ["onCleanup"] };
    case "settled": return { lines: ["onSettled(() => { cb(); });", "return 1;"], uses: ["onSettled"] };
    case "never": return { lines: ["return 1;"], uses: [] };
    case "returnCb": return { lines: ["return cb;"], uses: [] };
    case "returnWrap": return { lines: ["return () => cb();"], uses: [] };
    case "storeObj": return { lines: ["return { cb };"], uses: [] };
    case "register": return { lines: registerParts({ ...helper, pkg: { ...helper.pkg, store, owned } }).registerBody, uses: registerParts(helper).uses };
    default: throw new Error(`unknown package behaviour ${behaviour}`);
  }
}

/**
 * The files of the package for the helpers a program uses: `{ "package.json": text, "index.js": text, "index.d.ts": text, ... }`
 * (`null` when there are none). A pure function of the helpers, so two equal trees give byte-equal packages.
 */
export function packageFiles(helpers) {
  if (!helpers.length) return null;
  const mk = () => ({ js: [], dts: [], solid: new Set(), state: [] });
  const direct = mk();
  const impl = mk();
  const index = [];
  const indexDts = [];
  const containers = new Map();
  const byName = new Map(helpers.map((helper) => [helper.name, helper]));
  let defaultTaken = false;
  let star = false;

  for (const helper of helpers) {
    const { behaviour, exportForm } = helper.pkg;
    const into = REEXPORTS.includes(exportForm) ? impl : direct;
    const names = namesOf(helper);
    let body; let signature; let params = "cb";
    if (behaviour === "fire") {
      const registered = byName.get(helper.pkg.of);
      if (!registered) throw new Error(`fire ${helper.name} has no register`);
      const parts = registerParts(registered);
      body = { lines: parts.fireBody, uses: parts.uses.includes("runWithOwner") ? ["runWithOwner"] : [] };
      signature = signatureOf("fire");
      params = "";
    } else {
      body = bodyOf(helper);
      signature = signatureOf(behaviour);
      if (behaviour === "register") into.state.push(...registerParts(helper).decls);
    }
    for (const name of body.uses) into.solid.add(name);
    const dtsSig = `${signature.generic}(${signature.params}): ${signature.ret}`;
    const lines = body.lines.map((line) => `  ${line}`);
    switch (exportForm) {
      case "named": case "namespace": case "reexport": case "star": case "reexportRenamed": {
        const local = exportForm === "reexportRenamed" ? `${names.exportName}Impl` : names.exportName;
        into.js.push(`export function ${local}(${params}) {`, ...lines, "}", "");
        into.dts.push(`export declare function ${local}${dtsSig};`);
        if (exportForm === "reexport") { index.push(`export { ${local} } from "./impl.js";`); indexDts.push(`export { ${local} } from "./impl.js";`); }
        if (exportForm === "reexportRenamed") { index.push(`export { ${local} as ${names.alias} } from "./impl.js";`); indexDts.push(`export { ${local} as ${names.alias} } from "./impl.js";`); }
        if (exportForm === "star") star = true;
        break;
      }
      case "arrow":
        direct.js.push(`export const ${names.exportName} = (${params}) => {`, ...lines, "};", "");
        direct.dts.push(`export declare const ${names.exportName}: ${signature.generic}(${signature.params}) => ${signature.ret};`);
        break;
      case "default":
        if (defaultTaken) throw new Error("a package has one default export");
        defaultTaken = true;
        direct.js.push(`export default function ${helper.name}(${params}) {`, ...lines, "}", "");
        direct.dts.push(`declare function ${helper.name}${dtsSig};`, `export default ${helper.name};`);
        break;
      default: {
        if (!containers.has(names.container)) containers.set(names.container, { form: exportForm, members: [] });
        containers.get(names.container).members.push({ name: names.member, lines, params, dtsSig });
      }
    }
  }
  if (star) { index.push('export * from "./impl.js";'); indexDts.push('export * from "./impl.js";'); }

  for (const [name, { form, members }] of containers) {
    const body = (prefix) => members.flatMap((m) => [`  ${prefix}${m.name}(${m.params}) {`, ...m.lines.map((l) => `  ${l}`), "  }"]);
    if (form === "object") {
      direct.js.push(`export const ${name} = {`, ...members.flatMap((m, i) => [`  ${m.name}(${m.params}) {`, ...m.lines.map((l) => `  ${l}`), `  }${i < members.length - 1 ? "," : ""}`]), "};", "");
      direct.dts.push(`export declare const ${name}: {`, ...members.map((m) => `  ${m.name}${m.dtsSig};`), "};");
    } else {
      const prefix = form === "static" ? "static " : "";
      direct.js.push(`export class ${name} {`, ...body(prefix), "}", "");
      direct.dts.push(`export declare class ${name} {`, ...members.map((m) => `  ${prefix}${m.name}${m.dtsSig};`), "}");
    }
  }

  const head = (part) => [...(part.solid.size ? [`import { ${[...part.solid].sort().join(", ")} } from "solid-js";`, ""] : []), ...(part.state.length ? [...part.state, ""] : [])];
  const text = (lines) => `${lines.join("\n").replace(/\n+$/, "")}\n`;
  const files = {
    "package.json": `${JSON.stringify({
      name: PACKAGE_NAME, version: "1.0.0", type: "module",
      peerDependencies: { "solid-js": "^2.0.0-rc.17" },
      exports: { ".": { types: "./index.d.ts", default: "./index.js" } },
    }, null, 2)}\n`,
    "index.js": text([...head(direct), ...direct.js, ...index]),
    "index.d.ts": text([...direct.dts, ...indexDts]),
  };
  if (impl.js.length) {
    files["impl.js"] = text([...head(impl), ...impl.js]);
    files["impl.d.ts"] = text(impl.dts);
  }
  return files;
}

/** The distinct package helpers a program references, in declaration order (a helper nothing calls has no export). */
export function usedPackageHelpers(program, referencedNames) {
  return (program.helpers ?? []).filter((helper) => helper.pkg && referencedNames.has(helper.name));
}

export const digest = (files) => createHash("sha1").update(JSON.stringify(files)).digest("hex").slice(0, 10);
