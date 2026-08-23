// Minimal npm-compatible semver range matching.
//
// The benchmark decides which published Solid release a package accepts from
// the package's own declared ranges. Getting the *prerelease* rule wrong would
// silently pair a `^2.0.0-beta.17` package with a Solid 1.x release, or drop
// every beta-only package from the Solid 2 corpus, so the rule is implemented
// here rather than approximated by string prefixes. No dependency is available
// to scripts/, which run on plain node with node: builtins only.

const VERSION = /^v?(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+([0-9A-Za-z.-]+))?$/;

export function parseVersion(value) {
  if (typeof value !== "string") return null;
  const match = VERSION.exec(value.trim());
  if (!match) return null;
  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
    prerelease: match[4] ? match[4].split(".").map(part => (/^\d+$/.test(part) ? Number(part) : part)) : [],
    build: match[5] ?? ""
  };
}

function comparePrereleaseIdentifiers(left, right) {
  const leftNumeric = typeof left === "number";
  const rightNumeric = typeof right === "number";
  if (leftNumeric && rightNumeric) return left === right ? 0 : left < right ? -1 : 1;
  if (leftNumeric) return -1;
  if (rightNumeric) return 1;
  return left === right ? 0 : left < right ? -1 : 1;
}

function comparePrerelease(left, right) {
  if (left.length === 0 && right.length === 0) return 0;
  if (left.length === 0) return 1;
  if (right.length === 0) return -1;
  for (let index = 0; index < Math.max(left.length, right.length); index++) {
    if (index >= left.length) return -1;
    if (index >= right.length) return 1;
    const order = comparePrereleaseIdentifiers(left[index], right[index]);
    if (order !== 0) return order;
  }
  return 0;
}

export function compareVersions(left, right) {
  const a = typeof left === "string" ? parseVersion(left) : left;
  const b = typeof right === "string" ? parseVersion(right) : right;
  if (!a || !b) throw new Error(`cannot compare versions ${left} and ${right}`);
  for (const field of ["major", "minor", "patch"]) {
    if (a[field] !== b[field]) return a[field] < b[field] ? -1 : 1;
  }
  return comparePrerelease(a.prerelease, b.prerelease);
}

export function sortVersions(versions) {
  return [...versions].sort(compareVersions);
}

const WILDCARD = new Set(["x", "X", "*", "", undefined]);

function isWildcard(part) {
  return WILDCARD.has(part);
}

// Splits `1.2.3-beta.1` shaped text that may be partial (`1`, `1.x`, `1.2`).
function parsePartial(text) {
  const match = /^v?(\d+|[xX*])(?:\.(\d+|[xX*]))?(?:\.(\d+|[xX*]))?(?:-([0-9A-Za-z.-]+))?(?:\+([0-9A-Za-z.-]+))?$/.exec(
    text.trim()
  );
  if (!match) return null;
  return {
    major: match[1],
    minor: match[2],
    patch: match[3],
    prerelease: match[4] ? match[4].split(".").map(part => (/^\d+$/.test(part) ? Number(part) : part)) : []
  };
}

function comparator(operator, version) {
  return { operator, version };
}

function lowerBound(partial) {
  return {
    major: isWildcard(partial.major) ? 0 : Number(partial.major),
    minor: isWildcard(partial.minor) ? 0 : Number(partial.minor),
    patch: isWildcard(partial.patch) ? 0 : Number(partial.patch),
    prerelease: partial.prerelease,
    build: ""
  };
}

function nextMajor(version) {
  return { major: version.major + 1, minor: 0, patch: 0, prerelease: [], build: "" };
}

function nextMinor(version) {
  return { major: version.major, minor: version.minor + 1, patch: 0, prerelease: [], build: "" };
}

function nextPatch(version) {
  return { major: version.major, minor: version.minor, patch: version.patch + 1, prerelease: [], build: "" };
}

function expandCaret(partial) {
  const floor = lowerBound(partial);
  if (isWildcard(partial.major)) return [];
  if (isWildcard(partial.minor)) return [comparator(">=", floor), comparator("<", nextMajor(floor))];
  if (isWildcard(partial.patch)) {
    const ceiling = floor.major === 0 ? nextMinor(floor) : nextMajor(floor);
    return [comparator(">=", floor), comparator("<", ceiling)];
  }
  let ceiling;
  if (floor.major !== 0) ceiling = nextMajor(floor);
  else if (floor.minor !== 0) ceiling = nextMinor(floor);
  else ceiling = nextPatch(floor);
  return [comparator(">=", floor), comparator("<", ceiling)];
}

function expandTilde(partial) {
  const floor = lowerBound(partial);
  if (isWildcard(partial.major)) return [];
  if (isWildcard(partial.minor)) return [comparator(">=", floor), comparator("<", nextMajor(floor))];
  return [comparator(">=", floor), comparator("<", nextMinor(floor))];
}

function expandPlain(operator, partial) {
  const floor = lowerBound(partial);
  const partialMinor = isWildcard(partial.minor);
  const partialPatch = isWildcard(partial.patch);
  if (isWildcard(partial.major)) {
    // `*` / `x` / empty: any version, but still not a prerelease (npm rule).
    if (operator === "<" || operator === "<=") return [comparator("<", { major: Infinity, minor: 0, patch: 0, prerelease: [], build: "" })];
    return [];
  }
  if (!partialMinor && !partialPatch) {
    if (operator === "=" || operator === "") return [comparator("=", floor)];
    return [comparator(operator, floor)];
  }
  const ceiling = partialMinor ? nextMajor(floor) : nextMinor(floor);
  switch (operator) {
    case ">":
      return [comparator(">=", ceiling)];
    case ">=":
      return [comparator(">=", floor)];
    case "<":
      return [comparator("<", floor)];
    case "<=":
      return [comparator("<", ceiling)];
    default:
      return [comparator(">=", floor), comparator("<", ceiling)];
  }
}

function expandComparator(text) {
  const match = /^(\^|~>|~|>=|<=|>|<|=|v)?\s*(.+)$/.exec(text.trim());
  if (!match) return null;
  const operator = match[1] === "~>" ? "~" : match[1] === "v" ? "" : (match[1] ?? "");
  const partial = parsePartial(match[2]);
  if (!partial) return null;
  if (operator === "^") return expandCaret(partial);
  if (operator === "~") return expandTilde(partial);
  return expandPlain(operator, partial);
}

function expandHyphen(left, right) {
  const low = parsePartial(left);
  const high = parsePartial(right);
  if (!low || !high) return null;
  const floor = comparator(">=", lowerBound(low));
  if (isWildcard(high.major)) return [floor];
  const ceilingBase = lowerBound(high);
  if (isWildcard(high.minor)) return [floor, comparator("<", nextMajor(ceilingBase))];
  if (isWildcard(high.patch)) return [floor, comparator("<", nextMinor(ceilingBase))];
  return [floor, comparator("<=", ceilingBase)];
}

/**
 * Parses an npm range into a disjunction of comparator sets. Returns null when
 * the range text is not understood — callers must treat that as "unknown", not
 * as "no match", so an unparsed range never silently drops a package.
 */
export function parseRange(range) {
  if (typeof range !== "string") return null;
  const text = range.trim();
  if (text === "" || text === "*" || text === "x" || text === "X" || text === "latest") return [[]];
  const alternatives = [];
  for (const alternative of text.split("||")) {
    // npm allows whitespace between a comparison operator and its version
    // (`">= 1.3.0"` is a real published range, e.g. @solid-primitives/stream's
    // solid-js dependency). Splitting on whitespace first would make the bare
    // operator its own token and fail the whole range as unparsed, which is
    // fail-closed but wrong: the package would silently leave the corpus.
    // Rejoining the operator with its operand here keeps the tokenizer simple
    // while matching what npm's own parser accepts. `-` is deliberately absent
    // from the operator set so hyphen ranges still tokenize as three tokens.
    const tokens = alternative
      .trim()
      .replace(/([<>]=?|=|\^|~>?)\s+/g, "$1")
      .split(/\s+/)
      .filter(Boolean);
    if (tokens.length === 0) {
      alternatives.push([]);
      continue;
    }
    const set = [];
    let failed = false;
    for (let index = 0; index < tokens.length; index++) {
      if (tokens[index + 1] === "-" && tokens[index + 2] !== undefined) {
        const expanded = expandHyphen(tokens[index], tokens[index + 2]);
        if (!expanded) {
          failed = true;
          break;
        }
        set.push(...expanded);
        index += 2;
        continue;
      }
      const expanded = expandComparator(tokens[index]);
      if (!expanded) {
        failed = true;
        break;
      }
      set.push(...expanded);
    }
    if (failed) return null;
    alternatives.push(set);
  }
  return alternatives;
}

function testComparator(version, entry) {
  const order = compareVersions(version, entry.version);
  switch (entry.operator) {
    case ">":
      return order > 0;
    case ">=":
      return order >= 0;
    case "<":
      return order < 0;
    case "<=":
      return order <= 0;
    default:
      return order === 0;
  }
}

function sameTuple(left, right) {
  return left.major === right.major && left.minor === right.minor && left.patch === right.patch;
}

/**
 * npm's prerelease rule: `1.0.0-beta` satisfies a comparator set only when some
 * comparator in that same set carries a prerelease on the same [major, minor,
 * patch] tuple. This is why `>=1.0.0` does not accept `2.0.0-rc.0` while
 * `^2.0.0-beta.17` does.
 */
export function satisfies(version, range) {
  const parsedVersion = typeof version === "string" ? parseVersion(version) : version;
  if (!parsedVersion) return false;
  const parsedRange = Array.isArray(range) ? range : parseRange(range);
  if (!parsedRange) return false;
  for (const set of parsedRange) {
    if (!set.every(entry => testComparator(parsedVersion, entry))) continue;
    if (parsedVersion.prerelease.length === 0) return true;
    if (set.some(entry => entry.version.prerelease.length > 0 && sameTuple(entry.version, parsedVersion))) return true;
  }
  return false;
}

export function maxSatisfying(versions, range) {
  const parsed = parseRange(range);
  if (!parsed) return null;
  const matching = sortVersions(versions.filter(version => satisfies(version, parsed)));
  return matching.length ? matching[matching.length - 1] : null;
}

export function minSatisfying(versions, range) {
  const parsed = parseRange(range);
  if (!parsed) return null;
  const matching = sortVersions(versions.filter(version => satisfies(version, parsed)));
  return matching.length ? matching[0] : null;
}
