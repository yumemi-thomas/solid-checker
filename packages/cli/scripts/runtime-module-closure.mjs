// Which runtime modules an entrypoint's analysis should be seeded with.
//
// This walk decides one thing: the TypeScript project's `files` list in
// `analyzeTarget`, and therefore what the analysis can see. It has to exist,
// because seeding only the entrypoint makes a published ESM barrel's `.js`
// specifiers resolve to the adjacent `.d.ts` files, so the analysis would read
// declarations where it now reads runtime bytes.
//
// It no longer decides the *record*. The per-entrypoint hash set a review
// transfers against is the analyzing program's own module list
// (`--emit-module-inventory`), so a module this walk misses is no longer a
// *false* record -- it is a seeding gap, and the attestation names it. That
// division is the whole design: the walk seeds, the attestation records and
// verifies the seed.
//
// The enumeration is still fail-closed rather than best-effort. Every static
// specifier form a runtime module can carry is either resolved to a file that
// is recorded, resolved to something with no runtime semantics (a declaration
// file), classified as external (a bare specifier, which the package-contract
// boundary owns instead), or reported as a problem. What changed is what happens
// to a problem: every one is returned *structured* as well as as a note, and the
// generator reconciles it against the attestation instead of quoting it blind --
// the compiler can say whether it resolved the same specifier to a file this walk
// never recorded, resolved nothing for it either, or never saw it at all. See
// `attestedClosure` in generate-package-contract.mjs and
// docs/precision-backlog.md.

import { existsSync, readFileSync, statSync } from "node:fs";
import { dirname, extname, join, relative, resolve, sep } from "node:path";

/// Extensions a generated contract's analysis treats as runtime source.
///
/// Deliberately the same list `generate-package-contract.mjs` uses for export
/// targets: a module the analysis would not seed as a root is not part of what
/// the summaries were derived from. `.cjs`/`.cts` are absent because CJS
/// contract generation is unsupported, not because they are harmless.
const RUNTIME_EXTENSIONS = [".js", ".jsx", ".mjs", ".ts", ".tsx", ".mts"];

/// TypeScript resolves an ESM-spelled specifier against the *source* file that
/// produces it. `export { x } from "./impl.js"` in a package whose checkout
/// ships `impl.ts` and no `impl.js` resolves to `impl.ts`, and the analysis
/// reads exactly that file -- so a closure that recorded only the entry was
/// silently omitting the module every summary came from.
const EXTENSION_SUBSTITUTIONS = {
  ".js": [".ts", ".tsx"],
  ".jsx": [".tsx"],
  ".mjs": [".mts"],
  ".cjs": [".cts"]
};

/// Resolved, but carrying no runtime behavior: omitting it from the closure
/// cannot make the record lie about which bytes produced a summary.
const DECLARATION_EXTENSIONS = [".d.ts", ".d.mts", ".d.cts"];

function isRuntimeLeaf(target) {
  if (typeof target !== "string" || target.endsWith(".d.ts")) return false;
  return RUNTIME_EXTENSIONS.includes(extname(target));
}

function isWithinDirectory(root, candidate) {
  return candidate === root || candidate.startsWith(`${root}${sep}`);
}

function isFile(path) {
  return existsSync(path) && statSync(path).isFile();
}

const IDENTIFIER = /[A-Za-z0-9_$]|[\u0080-\uffff]/;

/// Words after which a `/` opens a regular expression rather than dividing.
const REGEX_PRECEDING_KEYWORDS = new Set([
  "return",
  "typeof",
  "instanceof",
  "in",
  "of",
  "new",
  "delete",
  "void",
  "do",
  "else",
  "yield",
  "await",
  "case",
  "throw"
]);

/// Words that end an `export` clause before any `from` can appear.
const CLAUSE_TERMINATORS = new Set([
  "function",
  "class",
  "const",
  "let",
  "var",
  "enum",
  "interface",
  "namespace",
  "module",
  "declare",
  "abstract",
  "async"
]);

function regexAllowed(previous) {
  if (!previous) return true;
  if (previous.kind === "word") return REGEX_PRECEDING_KEYWORDS.has(previous.value);
  if (previous.kind === "string") return false;
  return !")]}".includes(previous.value);
}

/// A token stream, string literals cooked, comments gone.
///
/// The predecessor of this function stripped `/* */` with a regular expression
/// and then dropped `//` lines, neither of which knows what a string is: one
/// `"https://example.com/*"` in a module's first line ate every import below
/// it, and the closure recorded the entry alone with nothing to say it had. A
/// scanner that tracks which construct it is inside is the only shape that
/// cannot do that, so the extraction below reads tokens rather than text.
///
/// Templates are tracked because `import(\`./a.js\`)` is a literal specifier
/// and `import(\`./\${name}.js\`)` is not, and the difference has to reach the
/// caller as a note rather than as silence.
export function tokenize(source) {
  const tokens = [];
  const templates = [];
  // "brace" for an ordinary block/object, "template-sub" for the `${` of a
  // template literal, whose matching `}` returns the scanner to template text.
  const stack = [];
  let mode = "code";
  let index = 0;
  let unterminated = "";
  const emit = token => {
    tokens.push(token);
    return token;
  };
  const previous = () => tokens[tokens.length - 1];

  while (index < source.length) {
    if (mode === "template") {
      const template = templates[templates.length - 1];
      const character = source[index];
      if (character === "\\") {
        template.cooked += source[index + 1] ?? "";
        index += 2;
        continue;
      }
      if (character === "`") {
        index += 1;
        templates.pop();
        emit(
          template.substituted
            ? { kind: "other", value: "`" }
            : { kind: "string", value: template.cooked }
        );
        mode = "code";
        continue;
      }
      if (character === "$" && source[index + 1] === "{") {
        template.substituted = true;
        stack.push("template-sub");
        mode = "code";
        index += 2;
        continue;
      }
      template.cooked += character;
      index += 1;
      continue;
    }

    const character = source[index];
    if (character === "/" && source[index + 1] === "/") {
      const end = source.indexOf("\n", index);
      index = end === -1 ? source.length : end + 1;
      continue;
    }
    if (character === "/" && source[index + 1] === "*") {
      const end = source.indexOf("*/", index + 2);
      if (end === -1) {
        unterminated = "block comment";
        index = source.length;
        continue;
      }
      index = end + 2;
      continue;
    }
    if (character === '"' || character === "'") {
      let value = "";
      let cursor = index + 1;
      let closed = false;
      while (cursor < source.length) {
        const inner = source[cursor];
        if (inner === "\\") {
          value += source[cursor + 1] ?? "";
          cursor += 2;
          continue;
        }
        if (inner === character) {
          closed = true;
          cursor += 1;
          break;
        }
        if (inner === "\n") break;
        value += inner;
        cursor += 1;
      }
      if (!closed) {
        unterminated = "string literal";
        index = source.length;
        continue;
      }
      emit({ kind: "string", value });
      index = cursor;
      continue;
    }
    if (character === "`") {
      templates.push({ cooked: "", substituted: false });
      mode = "template";
      index += 1;
      continue;
    }
    if (character === "/" && regexAllowed(previous())) {
      let cursor = index + 1;
      let inClass = false;
      let closed = false;
      while (cursor < source.length) {
        const inner = source[cursor];
        if (inner === "\\") {
          cursor += 2;
          continue;
        }
        if (inner === "\n") break;
        if (inner === "[") inClass = true;
        else if (inner === "]") inClass = false;
        else if (inner === "/" && !inClass) {
          closed = true;
          cursor += 1;
          break;
        }
        cursor += 1;
      }
      if (!closed) {
        // Not a regular expression after all; fall through to punctuation.
        emit({ kind: "other", value: "/" });
        index += 1;
        continue;
      }
      while (cursor < source.length && IDENTIFIER.test(source[cursor])) cursor += 1;
      emit({ kind: "other", value: "/regexp/" });
      index = cursor;
      continue;
    }
    if (IDENTIFIER.test(character)) {
      let cursor = index;
      while (cursor < source.length && IDENTIFIER.test(source[cursor])) cursor += 1;
      emit({ kind: "word", value: source.slice(index, cursor) });
      index = cursor;
      continue;
    }
    if (character === "{") {
      stack.push("brace");
      emit({ kind: "other", value: "{" });
      index += 1;
      continue;
    }
    if (character === "}") {
      const top = stack.pop();
      emit({ kind: "other", value: "}" });
      index += 1;
      if (top === "template-sub") mode = "template";
      continue;
    }
    if (character.trim() === "") {
      index += 1;
      continue;
    }
    emit({ kind: "other", value: character });
    index += 1;
  }
  if (templates.length) unterminated = unterminated || "template literal";
  return { tokens, unterminated };
}

/// Every static module specifier a runtime module names, plus what could not
/// be read as one.
///
/// A problem is `{ kind, reason }`, not a sentence, because the two kinds
/// reconcile differently against an attested module inventory. A `scan` problem
/// is a specifier this scanner could not read, so the compiler's own import list
/// settles whether anything was missed. A `dynamic-import` problem is a
/// specifier *nothing* can resolve statically -- the compiler resolves no file
/// for it either -- so the attested record is complete and what stays unproven
/// is what the runtime may load, which is a different claim and carries its own
/// note kind.
export function moduleSpecifiers(source) {
  const { tokens, unterminated } = tokenize(source);
  const specifiers = [];
  const problems = [];
  const problem = (kind, reason) => problems.push({ kind, reason });
  if (unterminated) {
    problem("scan", `the module could not be scanned: unterminated ${unterminated}`);
  }
  const isPunctuation = (token, value) => token?.kind === "other" && token.value === value;

  for (let index = 0; index < tokens.length; index++) {
    const token = tokens[index];
    if (token.kind !== "word") continue;
    if (token.value === "require") {
      if (!isPunctuation(tokens[index + 1], "(")) continue;
      if (tokens[index + 2]?.kind === "string" && isPunctuation(tokens[index + 3], ")")) {
        specifiers.push(tokens[index + 2].value);
      }
      // A computed `require()` is a CJS shape this generator never analyzes as
      // a runtime target, so it is not a gap in an ESM closure.
      continue;
    }
    if (token.value !== "import" && token.value !== "export") continue;
    const next = tokens[index + 1];
    if (!next) continue;
    if (token.value === "import" && isPunctuation(next, ".")) continue;
    if (token.value === "import" && isPunctuation(next, "(")) {
      if (tokens[index + 2]?.kind === "string" && isPunctuation(tokens[index + 3], ")")) {
        specifiers.push(tokens[index + 2].value);
      } else {
        problem("dynamic-import", "a dynamic import() whose specifier is not a string literal");
      }
      continue;
    }
    if (next.kind === "string") {
      specifiers.push(next.value);
      index += 1;
      continue;
    }
    // `export default …` never carries a module specifier, and letting the
    // clause scan run past it would attribute the next statement's `from` to
    // this one.
    if (token.value === "export" && next.kind === "word" && next.value === "default") continue;

    let depth = 0;
    for (let cursor = index + 1; cursor < tokens.length && cursor < index + 300; cursor++) {
      const clause = tokens[cursor];
      if (clause.kind === "other") {
        if ("{[(".includes(clause.value)) depth += 1;
        else if ("}])".includes(clause.value)) depth -= 1;
        else if (depth === 0 && (clause.value === ";" || clause.value === "=")) break;
        continue;
      }
      if (depth !== 0) continue;
      if (clause.kind !== "word") continue;
      if (clause.value === "from") {
        const specifier = tokens[cursor + 1];
        if (specifier?.kind === "string") specifiers.push(specifier.value);
        else problem("scan", "an import/export whose module specifier is not a string literal");
        index = cursor + 1;
        break;
      }
      if (CLAUSE_TERMINATORS.has(clause.value)) break;
    }
  }
  const seen = new Set();
  return {
    specifiers: [...new Set(specifiers)],
    problems: problems.filter(entry => {
      const key = `${entry.kind} ${entry.reason}`;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    })
  };
}

function patternCapture(pattern, candidate) {
  const star = pattern.indexOf("*");
  if (star === -1) return pattern === candidate ? "" : undefined;
  if (pattern.indexOf("*", star + 1) !== -1) return undefined;
  const prefix = pattern.slice(0, star);
  const suffix = pattern.slice(star + 1);
  if (
    !candidate.startsWith(prefix) ||
    !candidate.endsWith(suffix) ||
    candidate.length < prefix.length + suffix.length
  ) {
    return undefined;
  }
  return candidate.slice(prefix.length, candidate.length - suffix.length);
}

function substituteStar(pattern, capture) {
  const star = pattern.indexOf("*");
  if (star === -1) return pattern;
  return `${pattern.slice(0, star)}${capture}${pattern.slice(star + 1)}`;
}

/// Every runtime target an `imports`/`exports` value can select, with the
/// conditions that select it. `require` is skipped for the same reason the
/// export walker skips it: only ESM runtime leaves are ever analyzed.
function conditionalTargets(target, active) {
  if (typeof target === "string") return [target];
  if (Array.isArray(target)) return target.flatMap(value => conditionalTargets(value, active));
  if (!target || typeof target !== "object") return [];
  return Object.entries(target).flatMap(([condition, value]) => {
    if (condition === "types" || condition === "require") return [];
    if (active && !(condition === "default" || active.has(condition))) return [];
    return conditionalTargets(value, active);
  });
}

/// Resolves the specifiers a runtime module names, the way the analysis does.
///
/// Three answers, and the third is the point: a file (record it), `external`
/// (a bare specifier, which the package-contract boundary owns and no closure
/// hash could pin anyway), or a problem (the caller notes it, and the
/// entrypoint stops being transferable).
export function createModuleResolver({ packageRoot, manifest = {}, conditions = [] }) {
  const active = conditions.length ? new Set(conditions) : undefined;

  const probe = (base, spelled) => {
    const extension = extname(base);
    if (!extension) {
      const candidates = [
        base,
        ...RUNTIME_EXTENSIONS.map(candidate => `${base}${candidate}`),
        ...RUNTIME_EXTENSIONS.map(candidate => join(base, `index${candidate}`))
      ];
      const found = candidates.find(
        candidate =>
          isWithinDirectory(packageRoot, candidate) && isRuntimeLeaf(candidate) && isFile(candidate)
      );
      if (found) return { file: found };
      const declaration = DECLARATION_EXTENSIONS.flatMap(candidate => [
        `${base}${candidate}`,
        join(base, `index${candidate}`)
      ]).find(candidate => isFile(candidate));
      if (declaration) return { external: true };
      return {
        problem: `${spelled} names no runtime module inside the package (looked for ${candidates
          .map(candidate => relative(packageRoot, candidate).replaceAll(sep, "/"))
          .join(", ")})`
      };
    }
    const stem = base.slice(0, -extension.length);
    const candidates = [
      base,
      ...(EXTENSION_SUBSTITUTIONS[extension] ?? []).map(candidate => `${stem}${candidate}`)
    ];
    const found = candidates.find(
      candidate =>
        isWithinDirectory(packageRoot, candidate) && isRuntimeLeaf(candidate) && isFile(candidate)
    );
    if (found) return { file: found };
    // A declaration is resolved and carries no runtime behavior, so leaving it
    // out of the closure cannot make the record claim the wrong bytes.
    const declaration = [
      ...DECLARATION_EXTENSIONS.map(candidate => `${stem}${candidate}`),
      ...(base.endsWith(".d.ts") ? [base] : [])
    ].find(candidate => isFile(candidate));
    if (declaration) return { external: true };
    return {
      problem: `${spelled} names no runtime module inside the package (looked for ${candidates
        .map(candidate => relative(packageRoot, candidate).replaceAll(sep, "/"))
        .join(", ")})`
    };
  };

  const resolveTarget = (target, spelled) => {
    if (!target.startsWith("./") && !target.startsWith("../")) {
      // A package-imports entry may name another package; that boundary is the
      // dependency contract's, not this closure's.
      return { external: true };
    }
    const base = resolve(packageRoot, target);
    if (!isWithinDirectory(packageRoot, base)) {
      return { problem: `${spelled} resolves to ${target}, which escapes the package root` };
    }
    return probe(base, spelled);
  };

  const resolveImportsEntry = specifier => {
    const map = manifest.imports;
    if (!map || typeof map !== "object" || Array.isArray(map)) {
      return {
        problem: `${specifier} is a package-imports specifier and the manifest declares no "imports" map`
      };
    }
    let target = Object.prototype.hasOwnProperty.call(map, specifier) ? map[specifier] : undefined;
    let capture;
    if (target === undefined) {
      for (const [key, value] of Object.entries(map)) {
        if (!key.includes("*")) continue;
        const captured = patternCapture(key, specifier);
        if (captured === undefined) continue;
        target = value;
        capture = captured;
        break;
      }
    }
    if (target === undefined) {
      return { problem: `${specifier} matches no entry of the package's "imports" map` };
    }
    const selected = active ? conditionalTargets(target, active) : [];
    const candidates = [
      ...new Set(
        (selected.length ? selected : conditionalTargets(target, undefined)).map(value =>
          capture === undefined ? value : substituteStar(value, capture)
        )
      )
    ];
    if (candidates.length === 0) {
      return { problem: `${specifier} selects no runtime target from the package's "imports" map` };
    }
    if (candidates.length > 1) {
      // Guessing which conditional branch this generation resolves would put
      // one branch's bytes behind every branch's summaries.
      //
      // Which of those branches *is* a runtime module that exists is a fact,
      // not a guess, and the caller needs it: the compiler resolves nothing for
      // an unselected conditional specifier, so reconciliation would otherwise
      // read "the analysis read no file for it, the record is complete" and say
      // nothing -- while Node, under its own conditions, loads one of these
      // files. `runtimeTargets` is what lets `attestedClosure` separate that
      // from `./styles.css` and `./gone.js`, which name no runtime module at
      // all and which therefore no runtime loads either.
      return {
        problem:
          `${specifier} resolves to ${candidates.length} conditional targets ` +
          `(${candidates.join(", ")}) and this generation selects none of them; ` +
          "regenerate with --conditions to fix the branch",
        runtimeTargets: candidates.flatMap(value => {
          const branch = resolveTarget(value, specifier);
          return branch.file ? [branch.file] : [];
        })
      };
    }
    return resolveTarget(candidates[0], specifier);
  };

  return {
    resolve(importer, specifier) {
      if (typeof specifier !== "string" || specifier.length === 0) {
        return { problem: "an empty module specifier" };
      }
      if (specifier.startsWith("#")) return resolveImportsEntry(specifier);
      if (!specifier.startsWith(".") && !specifier.startsWith("/")) return { external: true };
      const pathPart = specifier.split(/[?#]/, 1)[0];
      const base = specifier.startsWith("/")
        ? pathPart
        : resolve(dirname(importer), pathPart);
      if (!isWithinDirectory(packageRoot, base)) {
        return { problem: `${specifier} resolves outside the package root` };
      }
      return probe(base, specifier);
    }
  };
}

/// The entry module plus every runtime module it statically pulls in, and the
/// reasons that set may be incomplete.
///
/// `notes` is the fail-closed channel, and an empty `notes` is still a claim, so
/// nothing may be dropped from this walk without adding one. What reads it
/// changed: the review plan's own notes come from `attestedClosure`'s
/// reconciliation of `problems` below, and `notes` is what a caller quotes when
/// it has no attestation to reconcile against.
///
/// `problems` carries the same set structured -- `{ file, specifier, kind,
/// reason, runtimeTargets }` -- so the generator can reconcile each one against
/// the analyzing program's own module inventory instead of quoting it blind.
/// `runtimeTargets` is the one field a reconciler cannot derive: the existing
/// runtime modules a *runtime* could still select for a specifier the compiler
/// resolved nothing for. `notes` is
/// derived from `problems` and nothing else, so a caller that reconciles cannot
/// disagree with a caller that quotes. `noteFor` renders one problem in exactly
/// the spelling `notes` uses, which is what lets a reconciled note keep the
/// sentence a reviewer already knows.
export function noteFor(problem) {
  return `${problem.spelled}: closure could not be fully enumerated: ${problem.reason}`;
}

export function runtimeModuleClosure({ packageRoot, entryFile, excludedFiles, resolver }) {
  const files = [];
  const problems = [];
  const seen = new Set();
  const pending = [entryFile];
  const visited = new Set();
  const spell = file => relative(packageRoot, file).replaceAll(sep, "/") || file;
  const record = (file, kind, reason, specifier, runtimeTargets) => {
    const problem = {
      file,
      spelled: spell(file),
      kind,
      reason,
      ...(specifier ? { specifier } : {}),
      // Existing runtime modules inside this package that a runtime could
      // select for the specifier and the analysis did not read. Empty is the
      // normal answer and means what it says: nothing on disk answers this
      // specifier, so no runtime loads a module for it either.
      ...(runtimeTargets?.length ? { runtimeTargets } : {})
    };
    const key = noteFor(problem);
    if (seen.has(key)) return;
    seen.add(key);
    problems.push(problem);
  };
  while (pending.length) {
    const file = pending.pop();
    if (visited.has(file) || (file !== entryFile && excludedFiles.has(file))) continue;
    visited.add(file);
    files.push(file);
    let scanned;
    try {
      scanned = moduleSpecifiers(readFileSync(file, "utf8"));
    } catch (error) {
      record(
        file,
        "unreadable",
        `the module could not be read (${String(error?.message ?? error)})`
      );
      continue;
    }
    for (const problem of scanned.problems) {
      record(file, problem.kind, problem.reason);
    }
    for (const specifier of scanned.specifiers) {
      const resolved = resolver.resolve(file, specifier);
      if (resolved.external) continue;
      if (resolved.problem) {
        record(file, "specifier", resolved.problem, specifier, resolved.runtimeTargets);
        continue;
      }
      if (resolved.file && !visited.has(resolved.file)) pending.push(resolved.file);
    }
  }
  return { files, problems, notes: [...new Set(problems.map(noteFor))].sort() };
}
