// The engine of the `escape` rewrite (rewrite.mjs): the same program with its identifiers, string literals and template pieces written with escapes.
//
// An identifier written `evaluate`, a string `"\x65valuate"` and a template `` `\u{65}valuate` `` mean what `evaluate`, `"evaluate"` and `` `evaluate` `` mean (ECMAScript
// reads the cooked value: StringValue of an IdentifierName, the cooked text of a string or template piece). Every check of the checker that compares a name has to compare that
// value, never the source text; ADR 1148's round 2 found the veto that did not, and the sweep this module serves (ADR 1152) asks the question of every name the checker reads.
//
// The source is parsed (Babel, from the fuzzer's pinned install: no hand-written lexer decides what a token is) and the places a name can be written with an escape are
// rewritten by position, so no line moves (an escape adds no line break): a site keeps its line, which is how verdicts are matched to sites (checker.mjs `siteVerdicts`).
//
// Where an escape is not equivalent (or not written), nothing is touched:
// - a keyword or a reserved word (`default` is no `default`), a contextual keyword used as syntax (they are no `Identifier` node: `async`, `of`, `get`, `set` as keywords);
// - a `MetaProperty` (`import.meta`, `new.target`: their parts must be written plain), a private name, a JSX name (a JSX identifier has no escapes), a JSX attribute's string
//   (it is not a JavaScript string: no escape is read in it), a directive (`"use strict"` is one only as written), a TypeScript type position (types are erased; tsrs reads them
//   too, but nothing the checker reads there is a name of the program), a tagged template (its tag reads the raw text), the name `__h` and any `__` name (the instrumented twin's counter:
//   its strip must still find it, hits.mjs), and a module specifier unless asked (`specifiers`).
// `sameProgram` is the proof of each rewrite: the escaped text parses to the same tree (cooked values and structure; positions, raw spellings and parenthesization aside).
import { createRequire } from "node:module";

const install = new URL("./install/package.json", import.meta.url);
let loaded = null;
/** Babel's parser, from the fuzzer's pinned install (loaded on first use: a run that escapes nothing needs no install). */
const babel = () => (loaded ??= createRequire(install)("@babel/parser"));

/** Reserved words (ECMAScript, strict mode and module code): never an identifier an escape may write. */
const RESERVED = new Set((
  "break case catch class const continue debugger default delete do else enum export extends false finally for function if import in instanceof new null return super switch this throw true try typeof var void while with "
  + "implements interface let package private protected public static yield await"
).split(" "));

/** Keys of a node that hold no code the checker reads a name in: positions and comments, and every TypeScript type position. */
const SKIP_KEYS = new Set([
  "loc", "start", "end", "range", "extra", "leadingComments", "trailingComments", "innerComments", "comments", "tokens", "errors",
  "typeAnnotation", "returnType", "typeParameters", "typeArguments", "superTypeParameters", "superTypeArguments", "implements", "predicate", "decorators", "accessibility",
]);

/** TypeScript wrappers whose `expression` is code (the rest of a `TS*` node is a type or a declaration: skipped). */
const TS_CODE_WRAPPERS = new Set(["TSAsExpression", "TSSatisfiesExpression", "TSNonNullExpression", "TSTypeAssertion", "TSInstantiationExpression"]);

/**
 * TypeScript's keyword tokens, reserved and contextual (its scanner's keyword table). An identifier spelled like one and written with an escape is a TS1260 ("Keywords cannot contain
 * escape characters"), a syntactic error of the type checker (`constructor` of a class, `type`, `get`, `async`, `of`, `from`, `undefined`...), so a variant with one fails the fuzzer's
 * type check and is dropped: they are left alone unless `keywords` asks (the sweep asks, in a pass of its own, to see what the checker does with them).
 */
export const TS_KEYWORDS = new Set((
  "abstract accessor any as asserts assert async await bigint boolean break case catch class continue const constructor debugger declare default delete do else enum export extends false finally for from function get global "
  + "if implements import in infer instanceof interface intrinsic is keyof let module namespace never new null number object of out override package private protected public readonly require return satisfies set static "
  + "string super switch symbol this throw true try type typeof undefined unique unknown using var void while with yield"
).split(" "));

const PLAIN_NAME = /^[A-Za-z_$][A-Za-z0-9_$]*$/;

/** The parser options for a file by its extension. */
function optionsFor(filename) {
  const typescript = /\.(tsx?|mts|cts)$/.test(filename);
  const jsx = /\.(tsx|jsx|js|mjs|cjs)$/.test(filename);
  return { sourceType: "unambiguous", plugins: [...(jsx ? ["jsx"] : []), ...(typescript ? ["typescript"] : [])], errorRecovery: false };
}

/** Whether `filename` is a file this rewrite reads (a JavaScript or TypeScript source). */
export const escapable = (filename) => /\.(tsx?|jsx?|mjs|cjs|mts|cts)$/.test(filename) && !/\.d\.[cm]?ts$/.test(filename);

/** An identifier node that is a name of the program an escape may write (see the module's text). */
function escapableIdentifier(node, parent, key, keywords) {
  if (!PLAIN_NAME.test(node.name) || RESERVED.has(node.name) || node.name.startsWith("__")) return false;
  if (!keywords && TS_KEYWORDS.has(node.name)) return false;
  if (parent.type === "MetaProperty" || parent.type === "PrivateName") return false;
  // A type-only import or export names a type: erased.
  if ((parent.type === "ImportSpecifier" || parent.type === "ExportSpecifier") && (parent.importKind === "type" || parent.exportKind === "type")) return false;
  if (parent.type === "ImportAttribute" || key === "typeName") return false;
  return true;
}

/** The end of an identifier's own text: Babel's range of a parameter or declarator name also holds its type annotation and its `?`, which are types (erased, and a keyword type takes no escape). */
function identifierEnd(source, node) {
  if (!node.typeAnnotation && !node.optional) return node.end;
  const cut = node.typeAnnotation ? node.typeAnnotation.start : node.end;
  return node.start + source.slice(node.start, cut).replace(/[\s?!]+$/, "").length;
}

/** Whether a string literal is written where a name is: a property key (object, class, pattern), a computed member's literal key, an import or export name. */
function isKey(parent, key) {
  const keyed = ["ObjectProperty", "ObjectMethod", "ClassMethod", "ClassProperty", "ClassAccessorProperty", "ClassPrivateProperty"];
  if (keyed.includes(parent.type)) return key === "key";
  if (parent.type === "MemberExpression" || parent.type === "OptionalMemberExpression") return key === "property" && parent.computed === true;
  if (["ImportSpecifier", "ExportSpecifier", "ExportNamespaceSpecifier"].includes(parent.type)) return key === "imported" || key === "exported" || key === "local";
  return false;
}

/** A string literal that is a JavaScript string whose escapes are read and whose value is no module specifier (those are `specifier` places). */
function stringRole(node, parent, key) {
  if (parent.type === "JSXAttribute" || parent.type === "ImportAttribute") return null;
  if ((parent.type === "ImportDeclaration" || parent.type === "ExportNamedDeclaration" || parent.type === "ExportAllDeclaration") && key === "source") return "specifier";
  if (parent.type === "CallExpression" && key === "arguments" && parent.arguments[0] === node
    && (parent.callee.type === "Import" || (parent.callee.type === "Identifier" && parent.callee.name === "require"))) return "specifier";
  if (parent.type === "ImportExpression" && key === "source") return "specifier";
  return "string";
}

/**
 * The places in `source` a name can be written with an escape: `[{ role, name, start, end }]`, `name` the cooked text, by position. `role` is `identifier`, `string`, `template`
 * (a piece of an untagged template) or `specifier` (a module specifier). Returns `null` when the source does not parse.
 */
export function escapeCandidates(source, { filename = "App.tsx", keywords = false } = {}) {
  let ast;
  try { ast = babel().parse(source, optionsFor(filename)); } catch { return null; }
  const found = [];
  const seen = new Set();
  const add = (place) => { const id = `${place.start}:${place.end}`; if (!seen.has(id)) { seen.add(id); found.push(place); } };
  const visit = (node, parent, key) => {
    if (!node || typeof node.type !== "string") return;
    if (node.type.startsWith("TS") && !TS_CODE_WRAPPERS.has(node.type) && node.type !== "TSParameterProperty") return;
    switch (node.type) {
      case "Identifier":
        if (escapableIdentifier(node, parent, key, keywords)) add({ role: "identifier", name: node.name, start: node.start, end: identifierEnd(source, node) });
        return;
      case "StringLiteral": {
        const role = stringRole(node, parent, key);
        if (role) add({ role, name: node.value, start: node.start, end: node.end, keyed: isKey(parent, key) });
        return;
      }
      case "TemplateLiteral":
        if (!(parent.type === "TaggedTemplateExpression" && key === "quasi")) {
          for (const piece of node.quasis) if (piece.value.cooked != null) add({ role: "template", name: piece.value.cooked, start: piece.start, end: piece.end });
        }
        for (const expression of node.expressions) visit(expression, node, "expressions");
        return;
      case "ImportDeclaration": if (node.importKind === "type" || node.importKind === "typeof") return; break;
      case "ExportNamedDeclaration": if (node.exportKind === "type") return; break;
      case "PrivateName": case "MetaProperty": case "JSXIdentifier": case "JSXText": case "Directive": case "DirectiveLiteral": return;
      default: break;
    }
    for (const field of Object.keys(node)) {
      if (SKIP_KEYS.has(field)) continue;
      const value = node[field];
      if (Array.isArray(value)) for (const item of value) visit(item, node, field);
      else if (value && typeof value === "object") visit(value, node, field);
    }
  };
  visit(ast.program, { type: "Root" }, "program");
  return found.sort((a, b) => a.start - b.start);
}

const hex = (code, width) => code.toString(16).toUpperCase().padStart(width, "0");
/** One character as an escape, by its position in the token: `\uXXXX`, `\u{X}`, then (strings and templates) `\xXX` and the escape of a character that needs none. */
function spell(char, index, role) {
  const code = char.codePointAt(0);
  const form = index % (role === "identifier" ? 2 : 4);
  if (form === 0) return `\\u${hex(code, 4)}`;
  if (form === 1) return `\\u{${hex(code, 1)}}`;
  if (form === 2 && code < 256) return `\\x${hex(code, 2)}`;
  // The escape of a character that needs none is its own character (`\a` is `a`); the letters with a meaning of their own (`b f n r t v u x`) take a code escape.
  if (/[acdeghijklmopqswyzA-Z]/.test(char)) return `\\${char}`;
  return `\\u${hex(code, 4)}`;
}

/** The raw text of one place with every letter, digit and underscore written as an escape (an escape already in a string keeps its own spelling). */
function escapeText(raw, role) {
  let out = "";
  let at = 0;
  let index = 0;
  while (at < raw.length) {
    const char = raw[at];
    if (char === "\\") {
      // An escape the source already has: copied whole (`\u{...}`, `\uXXXX`, `\xXX`, a line continuation, one character).
      const rest = raw.slice(at);
      const existing = rest.match(/^\\(u\{[0-9A-Fa-f]+\}|u[0-9A-Fa-f]{4}|x[0-9A-Fa-f]{2}|\r\n|[\s\S])/);
      out += existing[0];
      at += existing[0].length;
      continue;
    }
    if (/[A-Za-z0-9_]/.test(char) || (role === "identifier" && char === "$")) { out += spell(char, index, role); index += 1; } else out += char;
    at += 1;
  }
  return out;
}

/**
 * `source` with its places written with escapes. `names`: only the places whose cooked text is in the set (default every place); `roles`: the kinds of place (default identifier,
 * string and template; `specifier` is a module specifier and is off unless named); `keywords`: also the identifiers spelled like a TypeScript keyword (TS1260). Returns `{ text, escaped: Map<cooked text, count>, parsed }`; a source that does not parse is
 * returned as it is, `parsed: false`.
 */
export function escapeSource(source, { filename = "App.tsx", names = null, roles = ["identifier", "string", "template"], keywords = false } = {}) {
  const places = escapeCandidates(source, { filename, keywords });
  if (!places) return { text: source, escaped: new Map(), parsed: false };
  const escaped = new Map();
  let text = source;
  for (const place of [...places].reverse()) {
    if (!roles.includes(place.role) || (names && !names.has(place.name))) continue;
    // A string or template piece is escaped inside its quotes: the quote characters and `${`-adjacent text are never touched (only letters, digits and `_` change).
    const raw = source.slice(place.start, place.end);
    const spelled = place.role === "identifier" ? escapeText(raw, "identifier") : escapeText(raw, place.role === "template" ? "template" : "string");
    if (spelled === raw) continue;
    text = text.slice(0, place.start) + spelled + text.slice(place.end);
    escaped.set(place.name, (escaped.get(place.name) ?? 0) + 1);
  }
  return { text, escaped, parsed: true };
}

/** A tree with what an escape may change taken out: positions, raw spellings, parenthesization and the raw text of a template piece (its cooked text stays). */
function normalized(node) {
  if (Array.isArray(node)) return node.map(normalized);
  if (!node || typeof node !== "object") return node;
  const copy = {};
  for (const [key, value] of Object.entries(node)) {
    if (["loc", "start", "end", "range", "extra", "leadingComments", "trailingComments", "innerComments", "comments", "tokens", "errors"].includes(key)) continue;
    copy[key] = key === "value" && value && typeof value === "object" && "cooked" in value ? { cooked: value.cooked } : normalized(value);
  }
  return copy;
}

/** Whether `escaped` parses to the same tree as `source`: the proof that the escape changed no meaning (an identifier's name, a string's value, every structure). */
export function sameProgram(source, escaped, { filename = "App.tsx" } = {}) {
  try {
    const a = normalized(babel().parse(source, optionsFor(filename)).program);
    const b = normalized(babel().parse(escaped, optionsFor(filename)).program);
    return JSON.stringify(a) === JSON.stringify(b);
  } catch { return false; }
}

/** The distinct cooked names of a source's places, with their counts, by role: what a variant can choose among. */
export function escapeNames(source, options = {}) {
  const places = escapeCandidates(source, options) ?? [];
  const names = new Map();
  for (const place of places) {
    if (place.role === "specifier") continue;
    const slot = names.get(place.name) ?? { count: 0, roles: new Set() };
    slot.count += 1;
    slot.roles.add(place.role);
    names.set(place.name, slot);
  }
  return names;
}
