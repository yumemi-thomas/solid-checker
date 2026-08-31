// Classifies one `contract generate` (or install/verify) invocation's exit
// status and captured output into a stable failure class.
//
// The benchmark runs thousands of invocations against real, unmodified
// packages. Most will fail for reasons the checker already understands and
// reports through its own diagnostic text; this module turns that text back
// into a small, closed set of classes so the report can group and count
// failures instead of dumping raw stderr. An `unclassified` result always
// keeps the complete original stderr in `raw.stderr` — losing it would mean
// losing the only lead on a shape this module does not know yet, so nothing
// here is allowed to truncate or discard the captured stream, only to add a
// classification on top of it.

// Exact ids, in report order (matches the "Classes (exact ids)" list in
// INTERFACES.md).
export const FAILURE_CLASSES = [
  "success",
  // Generation exited 0 and wrote a contract, but refused one or more of the
  // package's entrypoints and said so on stdout. That contract describes a
  // strict subset of the package: a consumer importing a refused entrypoint
  // gets an uncertifiable result, not the claim this row would suggest. It is
  // not a failure -- a usable contract exists -- and it is emphatically not
  // `success`, which this report uses to mean a COMPLETE contract. Counting
  // it as success is how a 28-of-44-entrypoint corpus reported "6/6 (100%)".
  "partial-success",
  "unsupported-package-shape",
  "no-esm-runtime-target",
  // Split out from `no-esm-runtime-target`: that class means the package
  // declared a runtime target the generator could not use, which is usually a
  // publishing mistake. This one means the opposite -- the ESM target resolved
  // and parsed fine, and simply exports nothing, as a side-effect-only module
  // does. Grouping them hid a well-formed package among broken publishes.
  "no-exported-surface",
  // Sibling of the two above and deliberately next to them: the runtime and
  // declaration axes each resolved and each exports the name, but no exact
  // pair of runtime+declaration bindings backs it -- the legacy dual-root
  // shape, where `main` and `types` describe different module graphs. It is
  // neither "no target" nor "no exports", and landed `unclassified` until now.
  "missing-export-binding",
  // The stable-v1 artifact-case resolver's two "the published tarball does not
  // contain what the manifest promises" facts, kept next to the three above
  // because they are the same family of "the declared target did not survive
  // publication". They are honest refusals under standard conditions, not a
  // shape this module failed to recognize, and both landed `unclassified`
  // until now:
  //   - the entrypoint's own runtime/declaration target resolves to a path
  //     that is not a file;
  //   - a module the resolved declaration file imports is absent from the
  //     package's own closure.
  // Distinct from `no-esm-runtime-target`, which is the older whole-entrypoint
  // verdict; these two name the exact resolved path the case refused on.
  "unavailable-published-target",
  "missing-closure-module",
  // Every artifact case in the census was recorded inapplicable and none
  // refused, so there is no certifiable case and also no refusal reason to
  // name. Distinct from every failure class above, which all carry a cause.
  "all-cases-inapplicable",
  "cjs-only-entrypoint",
  "conditional-export-incompatible",
  "incompatible-conditional-summaries",
  "unresolved-parameter-behavior",
  "reactive-dispatch-unresolved",
  "reactive-source-uncaptured",
  "dependency-contract-obligation",
  // Discovered from the real corpus rather than guessed up front: these two
  // consumer-side contract obligations were the single largest bucket landing
  // in `unclassified` on the first full run. `unclassified` exists precisely
  // to surface a shape classification does not know yet, and a shape that
  // covers a fifth of the corpus has earned its own class.
  "package-contract-environment-dependent",
  "package-contract-export-missing",
  // The runtime target and export are exact, but the selected-signature facts
  // cannot close whether the value is callable. Schema v1 has no unknown
  // export-kind form, so generation must refuse rather than publish a false
  // negative callback claim.
  "export-kind-unresolved",
  // The generated summary contains function-only behavior for an export whose
  // resolved runtime kind is value. This is an internal proposal conflict,
  // distinct from simply lacking the callability fact above.
  "export-kind-conflict",
  "type-facts-failure",
  "checker-crash",
  "timeout",
  "install-failure",
  "integrity-failure",
  "unclassified"
];

// The inapplicable suffix is optional and deliberately does NOT make a row
// partial: a recorded disposition says no consumer reaches a certifiable module
// at that entrypoint, which is not an omitted claim. Only the "refused and
// omitted" suffix decides `partial-success`.
//
// Both counts are read from this pattern's own capture groups, never rescanned
// out of the matched text, and every free segment is `\S+` rather than `.+`.
// The package name and version in the "for <name>@<version>" slot come
// verbatim from the analyzed package's manifest -- attacker-controlled text --
// and a greedy `.+` there let a version string like `1.0.0-; 9 artifact
// case(s) refused and omitted` land inside the match, where a rescan read it
// as a real suffix and relabelled a complete contract `partial-success`. `\S+`
// cannot span the space in "artifact case(s)", the suffixes are anchored in
// fixed order after the path, and the counts come from the groups, so a
// manifest string can no longer inject either count.
const SUCCESS_PATTERN =
  /^(?:generated unaccepted stable contract proposal for \S+ at \S+(?:; (?<refusedCases>\d+) artifact case\(s\) refused and omitted)?(?:; (?<inapplicableCases>\d+) artifact case\(s\) recorded inapplicable)?; proof verification must issue its receipt|generated \S+ contract with \d+ entrypoints at \S+(?:; (?<legacyRefusedCases>\d+) entrypoint\(s\) refused and omitted)?; review plan \S+ \(\d+ checklist items\))$/m;

const INTEGRITY_PATTERN = /EINTEGRITY|integrity checksum failed|integrity mismatch|sha512 integrity/i;

// Marker order is deliberate, NOT the order the mapping is documented in:
// each entry is checked from most specific text to least specific, because a
// less specific marker (a bare Rust enum name like `ReactiveDispatchUnresolved`)
// can appear embedded inside a message that a more specific phrase already
// claims (for example the "required behavior: <exec>" tail of an
// unresolved-parameter-behavior line can itself name that enum variant). The
// more specific, harder-to-fake phrase must win so two different real
// diagnostics never collapse into the wrong class.
const MARKERS = [
  { class: "checker-crash", pattern: /SIGSEGV|SIGABRT|panicked at/i },
  // The full-refusal exit with an empty refusal census: every artifact case was
  // recorded inapplicable. Kept high because the shape is exact (the literal
  // zero count) and cannot match a message that carries any refusal, while the
  // named first-inapplicable reason below it is free text that a later marker
  // could otherwise claim.
  {
    class: "all-cases-inapplicable",
    pattern: /no certifiable artifact case; 0 case\(s\) refused/i
  },
  // A full-generation refusal can include attribution records for earlier
  // open callback claims before Rust emits the terminal proposal conflict.
  // `UnknownCallbackExecution` inside those records is evidence about how the
  // proposal was reached, not the terminal reason it was rejected. Keep this
  // exact conflict ahead of the broader parameter marker so diagnostic line
  // order cannot relabel the geolocation refusal.
  {
    class: "export-kind-conflict",
    pattern: /package contract value export .* cannot have function effects/i
  },
  { class: "unresolved-parameter-behavior", pattern: /unresolved parameter behavior|UnknownCallbackExecution/i },
  {
    class: "dependency-contract-obligation",
    pattern:
      /cannot statically expand external export-all|dependency contract for|accepted dependency .* exact .* binding/i
  },
  // The two publish-defect attributions, ranked here on purpose.
  //
  // BELOW `dependency-contract-obligation`: one real refusal states the
  // consumer-side obligation and then names the missing file that produced it
  // in the same message ("dependency contract for <dep> has no entrypoint
  // matching \".\"; resolved target <package-root>/dist/index.js is not a
  // file" — pinned in scripts/package-contract-v2-phase21-ledger.test.mjs).
  // The obligation is the terminal, actionable reason there and the absent
  // byte is only its evidence. phase20's classifyArtifactApplicability and
  // phase21's classifyPhase21Terminal both resolve that exact collision the
  // same way (the semantic terminal wins), so these three stay in agreement.
  //
  // ABOVE every marker below them, and `unsupported-package-shape` in
  // particular: its `package export target does not exist` alternative is this
  // same family stated without a resolved path, and the exact-path form must
  // win when one message carries both. Same argument against the broader
  // "<pkg> has no supported ESM runtime entrypoints" / "has no runtime ESM
  // exports" verdicts, which are whole-entrypoint statements rather than a
  // named absent file.
  //
  // The two patterns are disjoint fixed phrases, so their order relative to
  // each other is only for reading: entrypoint-target resolution comes first
  // because closure walking cannot start until it succeeds. Every free segment
  // is `\S+`, not `.+`, for the reason SUCCESS_PATTERN documents — the paths
  // are package-controlled text and must not be allowed to span a space into
  // the fixed wording on either side.
  {
    class: "unavailable-published-target",
    pattern: /resolved target \S+ is not a file/i
  },
  {
    class: "missing-closure-module",
    pattern: /local closure module \S+ from \S+ was not found/i
  },
  { class: "cjs-only-entrypoint", pattern: /has only a CJS runtime target/i },
  // Adjacent to `no-exported-surface` because it is the same family of
  // "the target resolved, the surface did not": here the name IS exported on
  // both axes but no exact runtime+declaration pair binds it, the legacy
  // dual-root shape where `main` and `types` describe different module graphs.
  // Ordered before the two broader export-surface markers so their looser
  // wording cannot claim it.
  {
    class: "missing-export-binding",
    pattern: /no exact runtime\/declaration binding for export/i
  },
  // Ordered before `no-esm-runtime-target`: classification takes the most
  // specific marker first, and "has no runtime ESM exports" is a strictly
  // narrower statement than "has no supported ESM runtime entrypoints".
  {
    class: "no-exported-surface",
    pattern: /has no runtime ESM exports/i
  },
  {
    class: "no-esm-runtime-target",
    pattern: /has no supported ESM runtime entrypoints|no semantic summary was produced/i
  },
  {
    class: "unsupported-package-shape",
    pattern:
      /must declare name(?:, version, and exports| and version)|package export target does not exist|export pattern|is not part of the TypeScript project|AST facts error: unsupported source path|Unknown file extension/i
  },
  {
    class: "incompatible-conditional-summaries",
    // The second alternative is the environment-dependent export KIND
    // refusal: schema v1 treats an export's kind as environment-
    // independent, so a `function` under one condition and a `value`
    // under another cannot be represented. Same family as the semantics
    // mismatch above -- two conditional branches that cannot be
    // reconciled -- so it groups here rather than earning a new class.
    pattern:
      /incompatible semantics across conditional targets|environment-dependent export kind/i
  },
  { class: "conditional-export-incompatible", pattern: /overlapping conditional-export branches/i },
  { class: "package-contract-environment-dependent", pattern: /PackageContractEnvironmentDependent/ },
  { class: "package-contract-export-missing", pattern: /PackageContractExportMissing/ },
  {
    class: "export-kind-unresolved",
    pattern: /runtime kind no closed type answers/i
  },
  { class: "type-facts-failure", pattern: /native Solid compiler facts error|type facts|typefacts protocol/i },
  { class: "reactive-source-uncaptured", pattern: /ReactiveSourceUncaptured/ },
  {
    class: "reactive-dispatch-unresolved",
    pattern: /ReactiveDispatchUnresolved|ReactiveCallbackUnresolved|StructuredReturnUnresolved/
  }
];

function dependencyFromPath(path) {
  const match = /node_modules\/((?:@[^/]+\/)?[^/]+)/.exec(path);
  return match ? match[1] : null;
}

function fileFromPath(path, dependency) {
  const marker = `node_modules/${dependency}/`;
  const index = path.indexOf(marker);
  if (index === -1) return null;
  return path.slice(index + marker.length);
}

function attachPathDetail(detail, path) {
  const dependency = dependencyFromPath(path);
  if (dependency) {
    detail.dependency = dependency;
    const file = fileFromPath(path, dependency);
    if (file) detail.file = file;
  } else {
    detail.file = path;
  }
}

function extractObligationDetail(text) {
  const detail = {};
  const match = /(?:unresolved obligation|unresolved effect) at (\/[^\s:]+):(\d+): (\w+)(?:\s*(\{[^}]*\}))?/.exec(
    text
  );
  if (!match) return detail;
  const [, path, offset, defect, fields] = match;
  detail.offset = Number(offset);
  detail.defect = defect;
  attachPathDetail(detail, path);
  if (fields) {
    const callee = /callee:\s*"([^"]*)"/.exec(fields);
    if (callee) detail.callee = callee[1];
    const member = /member:\s*Some\("([^"]*)"\)/.exec(fields);
    if (member) detail.member = member[1];
    const source = /source:\s*"([^"]*)"/.exec(fields);
    if (source) detail.source = source[1];
    // The consumer-side contract obligations name the dependency whose
    // contract is missing or environment-dependent. Recording it as `module`
    // is what lets the report count these as shared dependency blockers
    // instead of hundreds of unrelated one-off failures.
    const module = /module:\s*"([^"]*)"/.exec(fields);
    if (module) detail.module = module[1];
    const exported = /export:\s*"([^"]*)"/.exec(fields);
    if (exported) detail.exportedFunction = exported[1];
    const reexported = /reexported:\s*(true|false)/.exec(fields);
    if (reexported) detail.reexported = reexported[1] === "true";
  }
  return detail;
}

function extractParameterDetail(text) {
  const detail = {};
  const match =
    /unresolved parameter behavior in (\S+) parameter (\d+) \(([^)]*)\) at (\/[^\s:]+):(\d+): (.*?); required behavior: (.*?);/.exec(
      text
    );
  if (!match) return detail;
  const [, fn, parameter, parameterType, path, offset, , requiredExecution] = match;
  detail.exportedFunction = fn;
  detail.parameter = Number(parameter);
  detail.parameterType = parameterType;
  detail.offset = Number(offset);
  detail.requiredExecution = requiredExecution;
  attachPathDetail(detail, path);
  return detail;
}

function extractDependencyContractDetail(text) {
  const detail = {};
  const exportAll = /cannot statically expand external export-all "([^"]*)" from (\/[^\s;]+)/.exec(text);
  if (exportAll) {
    detail.module = exportAll[1];
    attachPathDetail(detail, exportAll[2]);
    return detail;
  }
  const acceptedBinding = /accepted dependency (\S+) has no exact (?:runtime|declaration) binding/.exec(text);
  if (acceptedBinding) {
    detail.module = acceptedBinding[1];
    return detail;
  }
  const depContract = /dependency contract for (\S+) has no entrypoint matching "([^"]*)"/.exec(text);
  if (depContract) {
    detail.dependency = depContract[1];
    detail.module = depContract[2];
  }
  return detail;
}

function extractTypeFactsDetail(text) {
  const detail = {};
  const match = /native Solid compiler facts error: (\/[^\s:]+):/.exec(text);
  if (match) attachPathDetail(detail, match[1]);
  return detail;
}

function extractCjsDetail(text) {
  const detail = {};
  const match = /^(.*?) has only a CJS runtime target; CJS contract generation is unsupported/m.exec(text.trim());
  if (match) detail.entrypoint = match[1];
  return detail;
}

function extractNoEsmDetail(text) {
  const detail = {};
  const exportsMatch = /entry file (\S+) exports "([^"]*)", but no semantic summary was produced/.exec(text);
  if (exportsMatch) {
    detail.file = exportsMatch[1];
    detail.exportedFunction = exportsMatch[2];
    return detail;
  }
  const entryFileMatch = /entry file (\S+) has no runtime ESM exports/.exec(text);
  if (entryFileMatch) {
    detail.file = entryFileMatch[1];
    return detail;
  }
  const pkgMatch = /^(\S+) has (?:no supported ESM runtime entrypoints|no runtime ESM exports)/m.exec(text.trim());
  if (pkgMatch) detail.entrypoint = pkgMatch[1];
  return detail;
}

function extractUnsupportedShapeDetail(text) {
  const detail = {};
  const mustDeclare = /^(\S+) must declare name(?:, version, and exports| and version) for package contract generation/m.exec(
    text.trim()
  );
  if (mustDeclare) {
    detail.file = mustDeclare[1];
    return detail;
  }
  const targetMissing = /package export target does not exist: (\S+)/.exec(text);
  if (targetMissing) {
    detail.module = targetMissing[1];
    return detail;
  }
  const notInProject = /entry file (\S+) is not part of the TypeScript project/.exec(text);
  if (notInProject) detail.file = notInProject[1];
  return detail;
}

function extractSuccessDetail(text, groups) {
  const detail = {};
  const match = /contract with (\d+) entrypoints at .*\((\d+) checklist items\)/.exec(text);
  if (match) {
    detail.entrypointCount = Number(match[1]);
    detail.checklistItems = Number(match[2]);
  }
  // Recorded, not omitted: read for the row's census, never for its outcome.
  // Taken from the success pattern's own group so no manifest string can
  // supply it.
  if (groups?.inapplicableCases !== undefined) {
    detail.inapplicableCases = Number(groups.inapplicableCases);
  }
  return detail;
}

function extractDetail(className, text) {
  switch (className) {
    case "reactive-dispatch-unresolved":
    case "reactive-source-uncaptured":
    case "package-contract-environment-dependent":
    case "package-contract-export-missing":
      return extractObligationDetail(text);
    case "unresolved-parameter-behavior":
      return extractParameterDetail(text);
    case "dependency-contract-obligation":
      return extractDependencyContractDetail(text);
    case "type-facts-failure":
      return extractTypeFactsDetail(text);
    case "cjs-only-entrypoint":
      return extractCjsDetail(text);
    case "no-esm-runtime-target":
    case "no-exported-surface":
      return extractNoEsmDetail(text);
    case "unsupported-package-shape":
      return extractUnsupportedShapeDetail(text);
    default:
      return {};
  }
}

function classifyInstallLikePhase({ status, stdout, stderr, raw }) {
  if (status === 0) {
    return { class: "success", signature: normalizeSignature(stdout || "install ok"), detail: {}, raw };
  }
  const text = `${stderr}\n${stdout}`;
  if (INTEGRITY_PATTERN.test(text)) {
    return { class: "integrity-failure", signature: normalizeSignature(stderr || stdout), detail: {}, raw };
  }
  return {
    class: "install-failure",
    signature: normalizeSignature(stderr || stdout || `bun install exited with status ${status}`),
    detail: {},
    raw
  };
}

function classifyGeneratePhase({ status, stdout, stderr, raw }) {
  if (status === 0) {
    const successMatch = SUCCESS_PATTERN.exec(stdout);
    if (successMatch) {
      // Historical schema-v1 benchmark logs carry the old generator's own note
      // for a partial contract, counted in entrypoints; the stable-v1
      // generator records finer-grained artifact-case refusals. Either way the
      // count is read from the statement rather than inferred by comparing
      // declared and generated entrypoint counts: a wildcard subpath is one
      // declared pattern expanding to many generated entrypoints, and `sameAs`
      // aliases collapse, so those two counts legitimately disagree on a
      // complete contract.
      const groups = successMatch.groups ?? {};
      const detail = extractSuccessDetail(successMatch[0], groups);
      const refusedCases = groups.refusedCases ?? groups.legacyRefusedCases;
      if (refusedCases !== undefined) {
        return {
          class: "partial-success",
          signature: normalizeSignature(successMatch[0]),
          detail: {
            ...detail,
            refusedCases: Number(refusedCases),
            refusalUnit: groups.refusedCases === undefined ? "entrypoint" : "artifact-case"
          },
          raw
        };
      }
      return {
        class: "success",
        signature: normalizeSignature(successMatch[0]),
        detail,
        raw
      };
    }
    // A zero exit without the documented success shape is never assumed to be
    // a success — the message format may simply have drifted, and silently
    // treating that as green would hide the drift instead of surfacing it.
    return { class: "unclassified", signature: normalizeSignature(`${stdout}\n${stderr}`), detail: {}, raw };
  }

  const text = `${stderr}\n${stdout}`;
  for (const marker of MARKERS) {
    if (marker.pattern.test(text)) {
      return {
        class: marker.class,
        signature: normalizeSignature(stderr || stdout),
        detail: extractDetail(marker.class, text),
        raw
      };
    }
  }

  // No recognized message: a null/negative status means the process was
  // killed by a signal, and a very high status is Rust's abort/panic exit
  // range. Only fall back to checker-crash here — never to a semantic class —
  // because we have no message to justify a more specific claim.
  if (status === null || (typeof status === "number" && (status < 0 || status >= 101))) {
    return { class: "checker-crash", signature: normalizeSignature(stderr || stdout || "checker crash"), detail: {}, raw };
  }

  return {
    class: "unclassified",
    signature: normalizeSignature(stderr || stdout || `exit status ${status}`),
    detail: {},
    raw
  };
}

export function classifyResult({ status, stdout = "", stderr = "", timedOut = false, phase = "generate" }) {
  const raw = { stdout, stderr };

  if (timedOut) {
    return { class: "timeout", signature: normalizeSignature(`timeout during ${phase}`), detail: {}, raw };
  }

  if (phase === "install" || phase === "verify") {
    return classifyInstallLikePhase({ status, stdout, stderr, raw });
  }

  return classifyGeneratePhase({ status, stdout, stderr, raw });
}

// Real logs carry either the full "solid-checker: solid-checker-rust: "
// prefix or just "solid-checker: " on its own (the conditional-export
// shape uses the shorter form) — match both.
// Either prefix may appear alone. Real multi-line stderr can put a warning
// on the first line and the actual error on a later one that starts with
// `solid-checker-rust:` without the outer `solid-checker:` prefix, so
// requiring the outer one left the prefix in place and split what should
// have been a single failure group.
const PREFIX_PATTERN = /^(?:solid-checker:\s*)?(?:solid-checker-rust:\s*)?/;
// Removes " at <path>:<offset>" as one unit first, so the common
// "<message> at <path>:<offset>: <rest>" shape collapses to
// "<message>: <rest>" without leaving a stray colon behind. The path
// character class stops at whitespace/colon/quote/paren/comma/semicolon so it
// never eats past the offset or into surrounding punctuation.
const AT_PATH_OFFSET_PATTERN = / at \/[^\s:"');,]+:\d+/g;
// Any other absolute path (no "at " prefix, e.g. the native-compiler-error
// shape, or a path following "from "). The lookbehind requires the leading
// "/" to be preceded by whitespace, a quote, an opening paren, or the start
// of the string — never by a word character. Without that guard this pattern
// used to match the "/name" tail of an unrelated "@scope/name" token (a
// scoped package or module is not a filesystem path), silently truncating it
// to "@scope" and leaving different packages with the same failure unable to
// group. See DEFECT 1 / DEFECT 2 in the fix that added this comment.
const ABS_PATH_PATTERN = /(?<=^|[\s"(])\/[^\s:"');,]+/g;
const DOUBLE_COLON_PATTERN = /:\s*:/g;
const OFFSET_RANGE_PATTERN = /@\d+\.\.\d+/g;
const OFFSET_COLON_PATTERN = /:\d+(?:\.\.\d+)?/g;
const VERSION_PATTERN = /\d+\.\d+\.\d+(?:-[0-9A-Za-z][0-9A-Za-z.-]*)?/g;
const OBJECT_LITERAL_PATTERN = /\{([^}]*)\}/g;

// The unresolved-parameter-behavior shape ends with a human-facing
// instruction and a generated schema-v1 JSON stub — neither is part of what
// identifies the failure, both are unbounded in size (the stub embeds the
// whole contract-in-progress, which was previously mangled by the
// brace-collapsing pass below since it does not parse nested braces), and
// keeping the stub in a group key means the signature can never be under a
// few hundred characters or safe to render in a Markdown table. We cut at
// "; required behavior:" rather than only at the stub marker: the required
// mode ("choose exactly one audited mode: inline, tracked, or deferred" in
// the one real sample seen) is itself a per-callback detail, and grouping by
// prose-shape-only (ignoring which mode a given callback needs) is what lets
// two failures on different required-execution modes still collapse into one
// row instead of fragmenting the report by an incidental detail. Both
// markers are cut for safety in case either appears without the other.
const REQUIRED_BEHAVIOR_TAIL_PATTERN = /;\s*required behavior:.*/s;
const STUB_EVIDENCE_TAIL_PATTERN = /edit this schema-v1 stub and review its evidence:.*/s;
// The documented shape itself is "... unresolved parameter behavior in <fn>
// parameter <i> (<type>) at <path>:<offset>: <message>; required behavior:
// <exec>; edit this schema-v1 stub...". Even after the two cuts above, the
// free-form "<message>" segment (which restates the resolved callee and a
// long fixed sentence about why it cannot be certified) is still per-callback
// prose that adds nothing beyond what the header already says, and a real
// sample of it measured 250+ characters on its own — too long to read in a
// report table and too easy to fragment across small wording differences. So
// for this one documented shape we cut straight after the header's closing
// paren and drop the rest outright, keeping only "in <fn> parameter <n>
// (<type>)" as the group key. This is more aggressive than the generic cuts
// above and runs first; it is a no-op (and the generic cuts above remain the
// fallback) for the sibling `UnknownCallbackExecution` shape, whose layout is
// not documented here.
const PARAMETER_BEHAVIOR_HEADER_ONLY_PATTERN = /(unresolved parameter behavior in \S+ parameter \d+ \([^)]*\)).*/s;

// "<pkg> <entrypoint>:<name> has different semantics across overlapping
// conditional-export branches <a> and <b>; ..." (and the sibling
// "has incompatible semantics across conditional targets: <a> versus <b>").
// The leading package/entrypoint/export tokens are exactly the identifiers
// that vary per package, so they are erased as one unit rather than left for
// the generic path/quote rules below to (mis)handle piecemeal.
const CONDITIONAL_HEADER_PATTERN = /^\S+ \S+:\S+ (has (?:different|incompatible) semantics across)/;
// Package/module names quoted in prose (export-all's `"<module>"`, a
// conditional branch's `"<condition>"`) and bracketed condition lists
// (`[]`, `["solid"]`) are per-package identifiers, not part of the shape.
const QUOTED_LITERAL_PATTERN = /"[^"]*"/g;
const BRACKETED_LIST_PATTERN = /\[[^\]]*\]/g;
// "resolved <Callee.method>" names the specific symbol a parameter/callback
// resolved to, which is again per-package/per-callback, not per-shape. The
// leading \b is required: without it this also matches the "resolved" that
// is a substring of "unresolved" (and of the enum name
// "ReactiveDispatchUnresolved"), corrupting both.
const RESOLVED_CALLEE_PATTERN = /\bresolved \S+/g;
// "in <fn> parameter <n> (<type>)" and the bare restatement later in the same
// message ("parameter <n> (<type>) is passed to ...") — the exported
// function name, parameter index, and parameter type all vary per
// package/callback, and the message repeats the parameter mention once
// without the "in <fn>" prefix, so this must replace every occurrence (`g`),
// not just the first.
const PARAMETER_MENTION_PATTERN = /(in \S+ )?parameter \d+ \([^)]*\)/g;

// Reduces a message to a shape-stable signature so the same underlying
// failure — hit against different packages, paths, byte offsets, tmpdirs,
// versions, export/callback names, and quoted or bracketed per-package
// identifiers — groups under one entry in the report instead of one entry
// per package. Everything erased above is exactly the part that varies
// run-to-run without changing what actually went wrong; the fixed English
// wording and the Rust enum's field NAMES (never their values) are kept
// because that is what distinguishes one real failure shape from another.
export function normalizeSignature(text) {
  if (typeof text !== "string") return "";
  let signature = text.trim();
  signature = signature.replace(PREFIX_PATTERN, "");
  signature = signature.replace(PARAMETER_BEHAVIOR_HEADER_ONLY_PATTERN, "$1");
  signature = signature.replace(REQUIRED_BEHAVIOR_TAIL_PATTERN, "");
  signature = signature.replace(STUB_EVIDENCE_TAIL_PATTERN, "");
  signature = signature.replace(CONDITIONAL_HEADER_PATTERN, "<pkg> <entrypoint>:<export> $1");
  signature = signature.replace(PARAMETER_MENTION_PATTERN, (match, fnPrefix) =>
    fnPrefix ? "in <fn> parameter <n> (<type>)" : "parameter <n> (<type>)"
  );
  signature = signature.replace(AT_PATH_OFFSET_PATTERN, "");
  signature = signature.replace(ABS_PATH_PATTERN, "");
  signature = signature.replace(DOUBLE_COLON_PATTERN, ":");
  signature = signature.replace(OFFSET_RANGE_PATTERN, "");
  signature = signature.replace(OFFSET_COLON_PATTERN, "");
  signature = signature.replace(VERSION_PATTERN, "");
  signature = signature.replace(OBJECT_LITERAL_PATTERN, (_, inner) => {
    const keys = inner
      .split(",")
      .map(part => part.trim().split(":")[0].trim())
      .filter(Boolean);
    return `{ ${keys.join(", ")} }`;
  });
  signature = signature.replace(BRACKETED_LIST_PATTERN, "[<conditions>]");
  signature = signature.replace(QUOTED_LITERAL_PATTERN, '"<value>"');
  signature = signature.replace(RESOLVED_CALLEE_PATTERN, "resolved <callee>");
  signature = signature.replace(/\s+/g, " ").trim();
  return signature;
}
