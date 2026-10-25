// Normalisation, diffing and reporting for the Type Facts differential harness
// (scripts/typefacts-differential.mjs). Pure functions over the JSON that
// `typefacts-transcript decode` prints, so every rule here is testable on
// synthetic frames without a producer. The wire decoding itself stays in Rust
// (rust/crates/typefacts/src/transcript.rs): nothing in this file reads CBOR.
//
// A *cell* is one comparable fact: (family, column, key) -> value. A family is
// a response field or fact table (`table.entities`, `modules`, `sources`, ...),
// a column is one field of a row (or `(row)` for the row's presence), and the
// key identifies the row (`path:start-end` for entities, the symbol id, the
// file path). Both producers' cells are normalised, then matched by key.

import { readFileSync } from "node:fs";

/** What the Rust producer answers for a fact family that has not landed yet. */
export const UNAVAILABLE_MARKER = /not available in the tsrs backend yet \(migration slice S\d+\)/;

/** Errors that follow from an earlier unavailable answer in the same session. */
const CASCADE_CODES = new Set(["state-mismatch", "generation-mismatch"]);

const NON_DURABLE_ID = /symbol:\d+:\d+/g;
/** Every symbol id, hashed or counter (`canonicalIds`). */
const ANY_ID = /symbol:(?:h:[0-9a-f]+|\d+:\d+)/g;
const LIB_PATH = /^bundled:\/libs\/|\/lib\.[A-Za-z0-9.]+\.d\.ts$/;

export const CATEGORIES = [
  "equal",
  "allowed",
  "value-differs",
  "column-missing-in-rust",
  "column-extra-in-rust",
  "row-missing-in-rust",
  "row-extra-in-rust",
  "rust-unavailable",
  "rust-error",
  "go-error",
  "decode-error",
  "transport"
];

/** Categories that are neither a match nor an expected absence. */
export const FAILING = new Set(
  CATEGORIES.filter((c) => !["equal", "allowed", "rust-unavailable"].includes(c))
);

// --- normalisation ---------------------------------------------------------

/** Applies `fn` to every string (values and object keys excluded) in a JSON tree. */
export function mapStrings(value, fn) {
  if (typeof value === "string") return fn(value);
  if (Array.isArray(value)) return value.map((item) => mapStrings(item, fn));
  if (value && typeof value === "object") {
    const out = {};
    for (const [key, item] of Object.entries(value)) out[key] = mapStrings(item, fn);
    return out;
  }
  return value;
}

/** `forEachString` with each string's JSON pointer below `at`. */
function forEachStringAt(value, at, fn) {
  if (typeof value === "string") fn(value, at);
  else if (Array.isArray(value)) value.forEach((item, index) => forEachStringAt(item, `${at}/${index}`, fn));
  else if (value && typeof value === "object") {
    for (const [key, item] of Object.entries(value)) forEachStringAt(item, `${at}/${key}`, fn);
  }
}

function forEachString(value, fn) {
  if (typeof value === "string") fn(value);
  else if (Array.isArray(value)) for (const item of value) forEachString(item, fn);
  else if (value && typeof value === "object") for (const item of Object.values(value)) forEachString(item, fn);
}

/** `bundled:///libs/x` and `bundled:/libs/x` name the same default-library file. */
export const normalizeBundled = (text) => text.replaceAll("bundled:///", "bundled:/");

export const isLibPath = (path) => typeof path === "string" && LIB_PATH.test(path);

const locationKey = (location) =>
  location ? `${location.path}:${location.startByte}-${location.endByte}` : "";

/** Symbol rows of a response, wherever they sit. */
function symbolRows(response) {
  return [
    ...(response.table?.symbols ?? []),
    ...(response.symbolEvidence ?? []),
    ...(response.referenceEvidence ?? [])
  ];
}

const declarationsOf = (row) => row.declarations ?? [];

const isLibOnlySymbol = (row) =>
  declarationsOf(row).length > 0 &&
  declarationsOf(row).every((declaration) => isLibPath(declaration.location?.path));

/**
 * A well-known-symbol property name embeds the checker's unique-symbol type
 * counter (`__@hasInstance@73` in one producer, `...@71` in the other): it
 * follows type creation order, which differs between the pinned TypeScript
 * snapshots. Compare the name without it. Project symbols are never renamed.
 */
export function libSymbolName(name) {
  const match = /^(.*@.*)@\d+$/.exec(name ?? "");
  return match ? match[1] : name;
}

const libLabel = (row) => `symbol:~lib:${libSymbolName(declarationsOf(row)[0].name)}`;

/**
 * Normalises one decoded transcript (`{handshake, exchanges}`).
 *
 * 1. `bundled:///` -> `bundled:/` everywhere.
 * 2. Default-library symbols, declarations and modules are dropped: which lib
 *    files exist and where their declarations sit follows the TypeScript
 *    version of each producer. An id that named a dropped symbol becomes
 *    `symbol:~lib:<name>` so a reference to it still compares.
 * 3. Non-durable ids (`symbol:<gen>:<n>`) are renamed from their content
 *    (first declaration, else the first entity that used them), so the
 *    producers' visit order cannot show through. One mapping per transcript.
 * 4. State tokens are opaque text: only their presence is kept. Source-arena
 *    paths and timings are dropped.
 *
 * A durable id hashes declaration offsets (`symbol:h:`), and the two producers'
 * library text differs, so the same library symbol has a different durable id in
 * each. `evidence` is the producer's own answer to "declare this id" (the
 * replay's evidence pass: symbol rows, same shape as `symbolEvidence`) and
 * joins the rows of the transcript itself as proof that an id is library-only.
 * An id with no proof in either place is left as it is: a durable id that
 * differs without proof is a difference, not noise.
 */
export function normalizeTranscript(transcript, evidence = [], { canonicalIds = false } = {}) {
  // `canonicalIds` renames hashed ids too, by the same content signature as a
  // counter. For comparing two id schemes (ADR 0366): the spelling of an id is
  // no longer a fact, the partition of symbols into ids is, and a partition
  // that differs shows as a value difference in every cell that names it.
  const idPattern = canonicalIds ? ANY_ID : NON_DURABLE_ID;
  const exchanges = transcript.exchanges.map((exchange) => ({
    ...exchange,
    response: exchange.response ? mapStrings(exchange.response, normalizeBundled) : exchange.response
  }));

  // (2) library symbol ids -> stable names
  const libIds = new Map();
  const proofs = (rows) => {
    for (const row of rows) {
      if (isLibOnlySymbol(row)) libIds.set(row.id, libLabel(row));
    }
  };
  for (const { response } of exchanges) {
    if (response) proofs(symbolRows(response));
  }
  // Evidence rows only prove; they are never added to the transcript. Their
  // declaration paths take the same `bundled:///` spelling as the rest.
  proofs(mapStrings(evidence, normalizeBundled));

  // (3) content signatures for the remaining ids, best tier first:
  //   decl@  a symbol row's first declaration (the row may sit in the
  //          transcript or in the producer's evidence, its own answer to
  //          "declare this id", which is never compared);
  //   site@  an object that carries the id as `symbol` beside its own
  //          `location` (or `declaration.location`) and `name`, such as a
  //          resolved call's declaration or parameter: a call signature has
  //          no declaration row but has a site;
  //   use@   the first entity whose `symbol` it is;
  //   other@ the id itself (no content found: a difference stays visible).
  // Within a tier the smallest signature wins.
  const TIERS = ["decl@", "site@", "use@", "other@"];
  const tierOf = (signature) => TIERS.findIndex((prefix) => signature.startsWith(prefix));
  const signatures = new Map(); // id -> signature
  const declarationLists = new Map(); // id -> every project declaration, sorted
  const noteSignature = (id, signature) => {
    const known = signatures.get(id);
    if (
      known === undefined ||
      tierOf(signature) < tierOf(known) ||
      (tierOf(signature) === tierOf(known) && signature < known)
    ) {
      signatures.set(id, signature);
    }
  };
  const declare = (row) => {
    if (typeof row?.id !== "string" || libIds.has(row.id)) return;
    const decls = declarationsOf(row)
      .filter((d) => !isLibPath(d.location?.path))
      .map((d) => `decl@${locationKey(d.location)}:${d.name}`)
      .sort();
    if (decls.length === 0) return;
    noteSignature(row.id, decls[0]);
    const list = decls.join("|");
    if (!declarationLists.has(row.id) || list < declarationLists.get(row.id)) declarationLists.set(row.id, list);
  };
  const sites = (value) => {
    if (Array.isArray(value)) {
      for (const item of value) sites(item);
    } else if (value && typeof value === "object") {
      const id = typeof value.symbol === "string" ? value.symbol : null;
      const site = value.location ?? value.declaration?.location;
      const name = value.name ?? value.declaration?.name;
      if (id && site && (value.symbol.match(idPattern) ?? [])[0] === id) {
        noteSignature(id, `site@${locationKey(site)}:${name ?? ""}`);
      }
      for (const item of Object.values(value)) sites(item);
    }
  };
  // Where each id is used: entity location, column and the place inside it
  // (a resolved call's `target` against its `declaration.symbol`). Two ids
  // with the same declarations (a shorthand property and its binding, a
  // method and its signature) are told apart by their uses, not by their
  // spelling.
  const uses = new Map(); // id -> Set of use coordinates
  for (const { response } of exchanges) {
    if (response) for (const row of symbolRows(response)) declare(row);
  }
  for (const row of mapStrings(evidence, normalizeBundled)) declare(row);
  for (const { response } of exchanges) {
    if (!response) continue;
    for (const entity of response.table?.entities ?? []) {
      const where = locationKey(entity.location);
      for (const [column, value] of Object.entries(entity)) {
        if (column === "location") continue;
        forEachStringAt(value, `${where}:${column}`, (text, pointer) => {
          for (const id of text.match(idPattern) ?? []) {
            if (!uses.has(id)) uses.set(id, new Set());
            uses.get(id).add(pointer);
          }
        });
        if (column !== "symbol") sites(value);
      }
      if (typeof entity.symbol === "string") {
        for (const id of entity.symbol.match(idPattern) ?? []) noteSignature(id, `use@${where}`);
      }
    }
    forEachString(response, (text) => {
      for (const id of text.match(idPattern) ?? []) noteSignature(id, `other@${id}`);
    });
  }
  const seen = new Map();
  const renames = new Map();
  // Ids sharing a signature (one first declaration, several ids) are numbered
  // by their whole declaration list, then by their uses, and only then by
  // spelling: hashed before counter, then by id. Ids equal in all of these
  // are interchangeable in every cell that names them by use.
  const rank = (id) => (canonicalIds && id.startsWith("symbol:h:") ? 0 : 1);
  const usesOf = (id) => [...(uses.get(id) ?? [])].sort().join("|");
  const tieKey = new Map(
    [...signatures.keys()].map((id) => [id, `${declarationLists.get(id) ?? ""}\u0000${usesOf(id)}`])
  );
  const compareText = (x, y) => (x < y ? -1 : x > y ? 1 : 0);
  const ordered = [...signatures.entries()].sort(
    (a, b) =>
      compareText(a[1], b[1]) ||
      compareText(tieKey.get(a[0]), tieKey.get(b[0])) ||
      rank(a[0]) - rank(b[0]) ||
      compareText(a[0], b[0])
  );
  for (const [id, signature] of ordered) {
    const count = seen.get(signature) ?? 0;
    seen.set(signature, count + 1);
    renames.set(id, `symbol:~${signature}${count > 0 ? `#${count}` : ""}`);
  }
  const renameIds = (text) => {
    let out = text;
    // Whole-string library ids first (durable hashes included).
    if (libIds.has(out)) return libIds.get(out);
    if (out.includes("symbol:")) {
      out = out.replace(idPattern, (id) => renames.get(id) ?? id);
    }
    return out;
  };

  const normalized = exchanges.map((exchange) => {
    const response = exchange.response;
    if (!response) return exchange;
    const next = { ...response };
    delete next.sourceArena;
    if (next.stateToken !== undefined) next.stateToken = "present";
    if (next.modules) next.modules = next.modules.filter((m) => !isLibPath(m.path));
    const cleanSymbols = (rows) =>
      rows
        .filter((row) => !isLibOnlySymbol(row))
        .map((row) => ({
          ...row,
          declarations: declarationsOf(row).filter((d) => !isLibPath(d.location?.path))
        }))
        .map((row) => (row.declarations.length === 0 ? withoutKey(row, "declarations") : row));
    if (next.symbolEvidence) next.symbolEvidence = cleanSymbols(next.symbolEvidence);
    if (next.referenceEvidence) next.referenceEvidence = cleanSymbols(next.referenceEvidence);
    if (next.table) {
      next.table = { ...next.table, symbols: cleanSymbols(next.table.symbols ?? []) };
    }
    return { ...exchange, response: mapStrings(next, renameIds) };
  });
  return { ...transcript, exchanges: normalized };
}

function withoutKey(object, key) {
  const { [key]: _removed, ...rest } = object;
  return rest;
}

// --- cells -----------------------------------------------------------------

const ROW = "(row)";

/** A stable text form for comparing values: object keys sorted, arrays in order. */
export function canonical(value) {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value && typeof value === "object") {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`)
      .join(",")}}`;
  }
  return JSON.stringify(value) ?? "null";
}

function addRow(cells, family, key, row, where) {
  cells.push({ family, column: ROW, key, value: true, where });
  for (const [column, value] of Object.entries(row)) {
    cells.push({ family, column, key, value, where });
  }
}

function addSet(cells, family, values) {
  for (const value of values) cells.push({ family, column: ROW, key: String(value), value: true });
}

/** Flattens one normalised response into cells. */
export function cellsOf(response) {
  const cells = [];
  cells.push({ family: "response", column: "ok", key: "", value: response.ok });
  cells.push({ family: "response", column: "generation", key: "", value: response.generation });
  if (response.error) {
    cells.push({ family: "response", column: "error", key: "", value: response.error });
  }
  if (response.stateToken !== undefined) {
    cells.push({ family: "response", column: "stateToken", key: "", value: response.stateToken });
  }
  if (response.referenceChangesExact) {
    cells.push({ family: "response", column: "referenceChangesExact", key: "", value: true });
  }
  addSet(cells, "affected", response.affected ?? []);
  addSet(cells, "unknownImportPaths", response.unknownImportPaths ?? []);
  addSet(cells, "unmappedRoots", response.unmappedRoots ?? []);
  addSet(cells, "changedReferenceSymbols", response.changedReferenceSymbols ?? []);
  for (const source of response.sources ?? []) {
    addRow(cells, "sources", source.path, withoutKey(source, "path"), { path: source.path });
  }
  for (const module of response.modules ?? []) {
    addRow(cells, "modules", module.path, withoutKey(module, "path"), { path: module.path });
  }
  withKeys(cells, "moduleImports", response.moduleImports ?? [], (row) => locationKey(row.specifier), (row) => row.specifier);
  for (const family of ["symbolEvidence", "referenceEvidence"]) {
    for (const row of response[family] ?? []) {
      addRow(cells, family, row.id, withoutKey(row, "id"), {});
    }
  }
  const table = response.table;
  if (table) {
    for (const source of table.sources ?? []) {
      addRow(cells, "table.sources", source.path, withoutKey(source, "path"), { path: source.path });
    }
    for (const file of table.files ?? []) {
      addRow(cells, "table.files", file.path, withoutKey(file, "path"), { path: file.path });
    }
    withKeys(cells, "table.entities", table.entities ?? [], (row) => locationKey(row.location), (row) => row.location, "location");
    for (const symbol of table.symbols ?? []) {
      addRow(cells, "table.symbols", symbol.id, withoutKey(symbol, "id"), {});
    }
  }
  return cells;
}

/** Rows keyed by `keyOf`, duplicates disambiguated by order of appearance. */
function withKeys(cells, family, rows, keyOf, whereOf, dropColumn) {
  const seen = new Map();
  for (const row of rows) {
    const base = keyOf(row);
    const count = seen.get(base) ?? 0;
    seen.set(base, count + 1);
    const key = count === 0 ? base : `${base}#${count}`;
    addRow(cells, family, key, dropColumn ? withoutKey(row, dropColumn) : row, whereOf(row));
  }
}

const cellId = (cell) => `${cell.family}\u0000${cell.key}\u0000${cell.column}`;

// --- comparison ------------------------------------------------------------

/**
 * Compares the cells of one exchange.
 *
 * @param {object[]} goCells
 * @param {object[]|null} rustCells null when Rust gave no usable answer
 * @param {{category: string}|null} absence why `rustCells` is null
 */
export function compareCells(goCells, rustCells, absence) {
  const results = [];
  if (rustCells === null) {
    for (const cell of goCells) {
      results.push({ ...describe(cell), category: absence.category, go: cell.value });
    }
    return results;
  }
  const rust = new Map(rustCells.map((cell) => [cellId(cell), cell]));
  const goRows = new Set(goCells.filter((c) => c.column === ROW).map((c) => `${c.family}\u0000${c.key}`));
  const rustRows = new Set(rustCells.filter((c) => c.column === ROW).map((c) => `${c.family}\u0000${c.key}`));
  for (const cell of goCells) {
    const match = rust.get(cellId(cell));
    const rowId = `${cell.family}\u0000${cell.key}`;
    if (match) {
      rust.delete(cellId(cell));
      const same = canonical(match.value) === canonical(cell.value);
      results.push({
        ...describe(cell),
        category: same ? "equal" : "value-differs",
        ...(same ? {} : { go: cell.value, rust: match.value })
      });
    } else if (!rustRows.has(rowId)) {
      // The whole row is absent: one result for the row, none for its columns.
      if (cell.column === ROW) {
        results.push({ ...describe(cell), category: "row-missing-in-rust", go: cell.value });
      }
    } else {
      results.push({ ...describe(cell), category: "column-missing-in-rust", go: cell.value });
    }
  }
  for (const cell of rust.values()) {
    const rowId = `${cell.family}\u0000${cell.key}`;
    if (!goRows.has(rowId)) {
      if (cell.column === ROW) {
        results.push({ ...describe(cell), category: "row-extra-in-rust", rust: cell.value });
      }
    } else {
      results.push({ ...describe(cell), category: "column-extra-in-rust", rust: cell.value });
    }
  }
  return results;
}

function describe(cell) {
  return { family: cell.family, column: cell.column, key: cell.key, where: cell.where };
}

/** The category for an exchange whose Rust answer is an error or missing. */
export function absenceFor(goResponse, rustResponse, sawUnavailable, replayNote = "") {
  if (!rustResponse) {
    return { category: "rust-error", reason: `no response${replayNote ? ` (${replayNote})` : ""}` };
  }
  if (rustResponse.ok) return null;
  const error = rustResponse.error ?? {};
  if (UNAVAILABLE_MARKER.test(error.message ?? "")) return { category: "rust-unavailable", reason: error.message };
  if (sawUnavailable && CASCADE_CODES.has(error.code)) {
    return { category: "rust-unavailable", reason: `follows an unavailable answer: ${error.code}` };
  }
  return { category: "rust-error", reason: `${error.code}: ${error.message}` };
}

/**
 * Compares two normalised transcripts exchange by exchange.
 * Returns one result per cell, each carrying its session coordinates.
 */
export function diffTranscripts(go, rust, meta = {}) {
  const results = [];
  const at = (exchange, extra) => ({
    ...extra,
    project: meta.project,
    session: meta.session,
    index: exchange?.index ?? -1,
    operation: exchange?.operation ?? "handshake"
  });
  const handshakeSame = canonical(go.handshake) === canonical(rust.handshake);
  results.push(
    at(null, {
      family: "handshake",
      column: ROW,
      key: "",
      category: handshakeSame ? "equal" : "value-differs",
      ...(handshakeSame ? {} : { go: go.handshake, rust: rust.handshake })
    })
  );
  let sawUnavailable = false;
  for (const goExchange of go.exchanges) {
    const goResponse = goExchange.response;
    if (!goResponse) continue; // the Go producer never answered (e.g. cancel)
    const rustExchange = rust.exchanges.find((e) => e.index === goExchange.index);
    const rustResponse = rustExchange?.response ?? null;
    if (rustResponse?.decodeError || goResponse.decodeError) {
      results.push(
        at(goExchange, {
          family: "response",
          column: "decode",
          key: "",
          category: "decode-error",
          go: goResponse.decodeError,
          rust: rustResponse?.decodeError
        })
      );
      continue;
    }
    if (!goResponse.ok) {
      // Go refused: Rust must refuse the same way.
      const same = rustResponse && !rustResponse.ok && canonical(rustResponse.error) === canonical(goResponse.error);
      results.push(
        at(goExchange, {
          family: "response",
          column: "error",
          key: "",
          category: same ? "equal" : rustResponse?.ok ? "go-error" : "value-differs",
          go: goResponse.error,
          rust: rustResponse?.error ?? rustResponse?.ok
        })
      );
      continue;
    }
    const absence = absenceFor(goResponse, rustResponse, sawUnavailable, meta.replayNote);
    if (absence?.category === "rust-unavailable") sawUnavailable = true;
    const compared = compareCells(cellsOf(goResponse), absence ? null : cellsOf(rustResponse), absence);
    for (const result of compared) {
      results.push(at(goExchange, { ...result, ...(absence ? { reason: absence.reason } : {}) }));
    }
  }
  return results;
}

// --- allow-list ------------------------------------------------------------

const ALLOW_FIELDS = new Set(["id", "family", "column", "category", "project", "key", "go", "rust", "justification"]);

/**
 * Parses and validates an allow-list document. Every entry is a snapshot
 * decision (AGENTS.md invariant 6), so a justification is mandatory, an
 * unknown field is refused, and an entry that names nothing matches nothing.
 */
export function parseAllowList(text) {
  const document = JSON.parse(text);
  if (document.version !== 1 || !Array.isArray(document.entries)) {
    throw new Error("allow-list: expected {version: 1, entries: [...]}");
  }
  const ids = new Set();
  for (const entry of document.entries) {
    for (const field of Object.keys(entry)) {
      if (!ALLOW_FIELDS.has(field)) throw new Error(`allow-list entry ${entry.id}: unknown field ${field}`);
    }
    if (!entry.id || ids.has(entry.id)) throw new Error(`allow-list: missing or duplicate id ${entry.id}`);
    ids.add(entry.id);
    if (typeof entry.justification !== "string" || entry.justification.trim().length < 20) {
      throw new Error(`allow-list entry ${entry.id}: justification is required (at least 20 characters)`);
    }
    if (!entry.family || !entry.column || !entry.category) {
      throw new Error(`allow-list entry ${entry.id}: family, column and category are required`);
    }
    if (!FAILING.has(entry.category)) {
      throw new Error(`allow-list entry ${entry.id}: category ${entry.category} cannot be allowed`);
    }
  }
  return document.entries;
}

export const loadAllowList = (path) => parseAllowList(readFileSync(path, "utf8"));

/** `*` matches any run of characters; everything else is literal. */
export function globMatches(pattern, text) {
  const escaped = pattern.split("*").map((part) => part.replace(/[.+?^${}()|[\]\\]/g, "\\$&"));
  return new RegExp(`^${escaped.join(".*")}$`).test(text ?? "");
}

export function entryMatches(entry, result) {
  if (entry.family !== result.family || entry.column !== result.column) return false;
  if (entry.category !== result.category) return false;
  if (entry.project !== undefined && !globMatches(entry.project, result.project)) return false;
  if (entry.key !== undefined && !globMatches(entry.key, result.key)) return false;
  if (entry.go !== undefined && canonical(entry.go) !== canonical(result.go)) return false;
  if (entry.rust !== undefined && canonical(entry.rust) !== canonical(result.rust)) return false;
  return true;
}

/** Marks allowed results `category: "allowed"` and returns per-entry use counts. */
export function applyAllowList(results, entries) {
  const used = new Map(entries.map((entry) => [entry.id, 0]));
  for (const result of results) {
    if (!FAILING.has(result.category)) continue;
    const entry = entries.find((candidate) => entryMatches(candidate, result));
    if (entry) {
      used.set(entry.id, used.get(entry.id) + 1);
      result.allowedBy = entry.id;
      result.wasCategory = result.category;
      result.category = "allowed";
    }
  }
  return used;
}

// --- report ----------------------------------------------------------------

const clip = (value, limit = Number(process.env.TYPEFACTS_DIFF_CLIP) || 400) => {
  if (value === undefined) return undefined;
  const text = typeof value === "string" ? value : canonical(value);
  return text.length > limit ? `${text.slice(0, limit)}...` : text;
};

/**
 * Aggregates results into the machine-readable report.
 *
 * `families[family][column][category]` counts cells; `examples` keeps the first
 * `limit` results per (category, family, column) with their coordinates.
 */
export function buildReport(results, { limit = 3, sessions = 0, failedSessions = [], allowUsed = new Map(), unavailableOperations = {} } = {}) {
  const families = {};
  const totals = {};
  const examples = [];
  const kept = new Map();
  for (const result of results) {
    const column = ((families[result.family] ??= {})[result.column] ??= {});
    column[result.category] = (column[result.category] ?? 0) + 1;
    totals[result.category] = (totals[result.category] ?? 0) + 1;
    if (result.category === "equal") continue;
    const id = `${result.category}\u0000${result.family}\u0000${result.column}`;
    const count = kept.get(id) ?? 0;
    if (count >= limit) continue;
    kept.set(id, count + 1);
    examples.push({
      category: result.category,
      family: result.family,
      column: result.column,
      project: result.project,
      session: result.session,
      exchange: result.index,
      operation: result.operation,
      key: result.key,
      path: result.where?.path,
      span: result.where ? [result.where.startByte, result.where.endByte] : undefined,
      go: clip(result.go),
      rust: clip(result.rust),
      reason: result.reason,
      allowedBy: result.allowedBy
    });
  }
  const failing = Object.entries(totals)
    .filter(([category]) => FAILING.has(category))
    .reduce((sum, [, count]) => sum + count, 0);
  return {
    version: 1,
    sessions,
    failedSessions,
    totals,
    failing,
    unavailable: totals["rust-unavailable"] ?? 0,
    unavailableOperations,
    families,
    allowList: {
      used: Object.fromEntries(allowUsed),
      unused: [...allowUsed].filter(([, count]) => count === 0).map(([id]) => id)
    },
    examples
  };
}

/** A short human summary of a report. */
export function summarize(report) {
  const lines = [];
  const totalCells = Object.values(report.totals).reduce((a, b) => a + b, 0);
  lines.push(`typefacts differential: ${report.sessions} session(s), ${totalCells} cell(s)`);
  for (const category of CATEGORIES) {
    if (report.totals[category]) lines.push(`  ${category.padEnd(24)} ${report.totals[category]}`);
  }
  const rows = [];
  for (const [family, columns] of Object.entries(report.families)) {
    for (const [column, counts] of Object.entries(columns)) {
      if (column === "(row)" || Object.keys(counts).some((c) => c !== "equal")) {
        rows.push([`${family}.${column}`, counts]);
      }
    }
  }
  if (rows.length > 0) lines.push("per family.column (cells):");
  for (const [name, counts] of rows) {
    lines.push(
      `  ${name.padEnd(42)} ${CATEGORIES.filter((c) => counts[c])
        .map((c) => `${c}=${counts[c]}`)
        .join(" ")}`
    );
  }
  if (report.failedSessions.length > 0) {
    lines.push(`sessions that did not replay cleanly: ${report.failedSessions.length}`);
    for (const failure of report.failedSessions.slice(0, 5)) lines.push(`  ${failure.session}: ${failure.reason}`);
  }
  if (report.allowList.unused.length > 0) {
    lines.push(`unused allow-list entries: ${report.allowList.unused.join(", ")}`);
  }
  const shown = new Set();
  const failingFirst = [...report.examples].sort(
    (a, b) => Number(a.category === "rust-unavailable") - Number(b.category === "rust-unavailable")
  );
  const short = (text) => (text !== undefined && text.length > 110 ? `${text.slice(0, 110)}...` : text);
  for (const example of failingFirst) {
    const head = `${example.category} ${example.family}.${example.column}`;
    if (shown.has(head)) continue;
    shown.add(head);
    if (shown.size > 10) break;
    lines.push(
      `e.g. ${head}: ${example.project} ${example.operation}#${example.exchange} key=${short(example.key)}` +
        (example.go !== undefined ? ` go=${short(example.go)}` : "") +
        (example.rust !== undefined ? ` rust=${short(example.rust)}` : "") +
        (example.reason ? ` (${short(example.reason)})` : "")
    );
  }
  return lines.join("\n");
}

// --- end-to-end (coverage with the Rust producer) --------------------------

/** One-line reason for a checker failure: its message, minus paths and numbers. */
export function failureReason(failure) {
  const lines = (failure?.stderr ?? "").split("\n").filter((line) => line.trim());
  // Notes precede the error that ended the run; prefer the line that says so.
  const text = lines.find((line) => /error|failed/i.test(line)) ?? lines[0] ?? "no output";
  return text
    .replace(/\/[^\s"']+/g, "<path>")
    .replace(/\d{3,}/g, "<n>")
    .slice(0, 200);
}

/** Aggregates the report scripts/coverage.mjs writes under SOLID_COVERAGE_REPORT. */
export function summarizeCoverage(report) {
  const reasons = {};
  const moved = [];
  for (const entry of report.moved) {
    if (entry.state === "checker-failed") {
      const reason = failureReason(entry.failure);
      reasons[reason] = (reasons[reason] ?? 0) + 1;
    } else if (entry.state === "moved") moved.push(entry);
  }
  return {
    version: 1,
    projects: report.projects,
    states: report.states,
    checkerFailureReasons: reasons,
    movedWithoutFailure: moved.map((entry) => ({
      project: entry.project,
      statusWas: entry.statusWas,
      statusNow: entry.statusNow,
      findingsWas: entry.findingsWas,
      findingsNow: entry.findingsNow
    }))
  };
}

export function summarizeCoverageText(summary) {
  const lines = [`end-to-end coverage with the Rust producer: ${summary.projects} snapshot(s)`];
  for (const [state, count] of Object.entries(summary.states)) lines.push(`  ${state.padEnd(16)} ${count}`);
  const reasons = Object.entries(summary.checkerFailureReasons).sort((a, b) => b[1] - a[1]);
  if (reasons.length > 0) lines.push("checker failures by reason:");
  for (const [reason, count] of reasons.slice(0, 8)) lines.push(`  ${String(count).padStart(5)}  ${reason}`);
  if (summary.movedWithoutFailure.length > 0) {
    lines.push(`snapshots that moved although the checker ran: ${summary.movedWithoutFailure.length}`);
    for (const entry of summary.movedWithoutFailure.slice(0, 10)) {
      lines.push(
        `  ${entry.project}: ${entry.statusWas} -> ${entry.statusNow}, ${entry.findingsWas} -> ${entry.findingsNow} finding(s)`
      );
    }
  }
  return lines.join("\n");
}
