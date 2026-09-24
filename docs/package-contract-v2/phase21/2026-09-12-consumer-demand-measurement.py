#!/usr/bin/env python3
"""Consumer demand for package contracts, measured from real Solid projects.

Read-only inventory in the style of `2026-09-12-creates-refusal-inventory.py`.
It runs the checker over every consumer project under the given roots,
collects the SC9005 findings that name a package export with no contract
summary, aggregates them by (module, export), and crosses the result with the
pinned ecosystem report to say, per demanded export, which claim domains are
closed and what blocks the rest. It never certifies anything.

    python3 docs/package-contract-v2/phase21/2026-09-12-consumer-demand-measurement.py \
        --checker rust/target/debug/solid-checker-rust \
        --typefacts bin/solid-typefacts \
        --report benchmarks/ecosystem/report.json \
        --out /tmp/demand \
        <consumer-root> [<consumer-root> ...]

Every `tsconfig.json` under a root (depth <= 4, node_modules and template
trees excluded) is one project. Projects must already have their
dependencies installed; the checker resolves imports through node_modules and
raises the contract obligation only for a package whose manifest uses Solid.

Every import site counts (fixed 2026-09-24, ways-to-improve § 3.1 c). Until
then only findings matching the no-summary sentence became demand rows, which
missed two populations: every open-claims finding (an accepted contract that
leaves domains unknown), and -- since the checker began collapsing repeated
import obligations (ac507c05, c566bab3; 2026-09-15/16) -- every package
imported at more than one site, whose acceptance-gate finding is one sentence
("this project has no accepted reactivity contract for M; ... N exports used
across K import sites: ...") with the other sites in `relatedLocations`. The
2026-09-17 Solid 2 sweeps ran after both collapses. Now each finding's primary
and related locations are sites, and a collapsed site's export is read from
the source bytes the location names: a namespace member's property, or one of
the value bindings of the import declaration the checker anchors every named
binding to. A declaration whose value bindings cannot be matched one-to-one to
its repeated sites (a binding used only in types raises nothing) is counted
under `?` and reported, not guessed. Argument-site findings (an obligation at
one call argument, not at an import) are counted separately and are not
demand.

    python3 docs/package-contract-v2/phase21/2026-09-12-consumer-demand-measurement.py --self-test
"""
import argparse
import collections
import json
import os
import re
import subprocess
import sys

# The per-export sentences of SC9005 (`projection.rs`, `PackageContractExportMissing`).
# Each names one (module, export); a collapsed claim group keeps the sentence
# and carries its other sites in `relatedLocations`.
PER_EXPORT = [
    re.compile(r"the reactivity contract for (\S+) has no entrypoint/export summary for (?:imported|re-exported) export (\S+);"),
    re.compile(r"the reactivity contract for (\S+) leaves .+? unknown for (?:imported|re-exported) export (\S+);"),
    re.compile(r"the discovered reactivity contract for (\S+) was authorized only by obsolete proof policy 1; its claims cannot be used for (?:imported|re-exported) export (\S+)$"),
]
# The same obligation at one call argument: not an import, so not demand.
ARGUMENT = re.compile(r"the reactivity contract for (\S+) states .+? for (?:imported|re-exported) export (\S+), but this call site")
# The acceptance gate collapsed over a package: no export in the sentence.
COLLAPSED = re.compile(r"this project has no accepted reactivity contract for (\S+);")
# The no-summary sentence is also what the old collector matched, alone.
DEMAND = PER_EXPORT[0]
# Which gate an SC9005 import-site finding stopped at. The message alone cannot
# say: the no-summary text above is emitted both when a contract is accepted and
# leaves the export out, and when no contract is accepted at all. The
# analysis_context separates them, and the distinction is the whole answer to
# "which closures change a consumer-side finding" -- a closure moves a finding
# only at `unknown-contract-claims`. Measured 2026-09-14: 2,585 of 2,585 stop at
# the acceptance gate, so no closure in any domain moves any of them.
ACCEPTANCE_GATE = "no receipt-accepted contract matches this exact import"
LEG = re.compile(r"derivation: ([a-z-]+)")


def projects_under(root):
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in ("node_modules", "template", "tests", ".git")]
        if dirpath[len(root):].count(os.sep) > 4:
            dirnames[:] = []
            continue
        if "tsconfig.json" in filenames:
            yield dirpath


def analyze(checker, typefacts, project, out):
    ident = os.path.relpath(project).replace(os.sep, "__")
    target = os.path.join(out, ident + ".json")
    if not os.path.exists(target):
        env = dict(os.environ, SOLID_TYPEFACTS_BIN=typefacts)
        with open(target, "w") as stdout, open(target + ".err", "w") as stderr:
            subprocess.run(
                [checker, "--format", "json", "--project", os.path.join(project, "tsconfig.json")],
                stdout=stdout, stderr=stderr, env=env, check=False,
            )
    try:
        return ident, json.load(open(target))
    except Exception:
        return ident, None


IMPORT_DECLARATION = re.compile(r"^\s*import\s+(?!type\b)(.*?)\s+from\s+['\"]", re.S)


def value_bindings(declaration):
    """The imported names of an import declaration's value bindings, in order.

    `import d, { a, b as c, type T } from "m"` -> ["default", "a", "b"]. A
    namespace binding is not an export: the checker raises its obligations at
    each member access instead. `None` when the text is not a declaration this
    reads (a string-named specifier, say), so the caller counts the site as
    unattributed rather than guessing.
    """
    match = IMPORT_DECLARATION.match(declaration)
    if not match:
        return None
    clause = match.group(1).strip()
    names = []
    braces = re.search(r"\{(.*)\}", clause, re.S)
    head = clause[: braces.start()] if braces else clause
    for part in [piece.strip() for piece in head.split(",") if piece.strip()]:
        if part.startswith("*"):
            continue
        if not re.fullmatch(r"[A-Za-z_$][\w$]*", part):
            return None
        names.append("default")
    if braces:
        for part in [piece.strip() for piece in braces.group(1).split(",") if piece.strip()]:
            if re.match(r"type\s", part):
                continue
            imported = part.split(" as ")[0].strip()
            if not re.fullmatch(r"[A-Za-z_$][\w$]*", imported):
                return None
            names.append(imported)
    return names


def source_bytes(location, cache):
    path = location.get("path")
    if path not in cache:
        try:
            cache[path] = open(path, "rb").read()
        except OSError:
            cache[path] = None
    return cache[path]


def collapsed_sites(module, locations, cache):
    """(module, export) per site of a finding collapsed over a package.

    Returns (attributed, unattributed): a Counter of exports and a count of
    sites whose export the bytes do not settle one-to-one.
    """
    attributed = collections.Counter()
    unattributed = 0
    declarations = collections.Counter()
    for location in locations:
        data = source_bytes(location, cache)
        start, end = location.get("startByte"), location.get("endByte")
        if data is None or start is None or end is None:
            unattributed += 1
            continue
        text = data[start:end].decode("utf-8", "replace")
        if re.fullmatch(r"[A-Za-z_$][\w$]*", text) and data[start - 1 : start] == b".":
            attributed[text] += 1  # a namespace member's property
        else:
            declarations[(location.get("path"), start, end, text)] += 1
    for (_, _, _, text), repeats in declarations.items():
        names = value_bindings(text)
        if names is not None and len(names) == repeats:
            attributed.update(names)
        else:
            unattributed += repeats
    return attributed, unattributed


def import_sites(finding, cache):
    """((module, export) -> sites, unattributed sites, argument sites) for one SC9005 finding."""
    message = finding.get("message", "")
    locations = [finding.get("primaryLocation") or {}] + list(finding.get("relatedLocations") or [])
    if ARGUMENT.search(message):
        return collections.Counter(), collections.Counter(), len(locations)
    for pattern in PER_EXPORT:
        match = pattern.search(message)
        if match:
            return collections.Counter({(match.group(1), match.group(2)): len(locations)}), collections.Counter(), 0
    match = COLLAPSED.search(message)
    if match:
        module = match.group(1)
        attributed, unattributed = collapsed_sites(module, locations, cache)
        sites = collections.Counter({(module, export): count for export, count in attributed.items()})
        return sites, collections.Counter({module: unattributed} if unattributed else {}), 0
    # A sentence this collector does not know is a site it cannot name; say so.
    return collections.Counter(), collections.Counter({"<unrecognized SC9005 message>": len(locations)}), 0


def self_test():
    import tempfile

    with tempfile.TemporaryDirectory() as scratch:
        path = os.path.join(scratch, "App.tsx")
        source = (
            'import { a, b as c, type T } from "m";\n'
            'import * as ns from "m";\n'
            'import d, { e } from "m";\n'
            "ns.f();\n"
        )
        open(path, "w").write(source)
        data = source.encode()

        def span(text, occurrence=0):
            start = -1
            for _ in range(occurrence + 1):
                start = data.index(text.encode(), start + 1)
            return {"path": path, "startByte": start, "endByte": start + len(text.encode())}

        first = span('import { a, b as c, type T } from "m";')
        third = span('import d, { e } from "m";')
        call = span("ns.f")
        member = {"path": path, "startByte": call["startByte"] + 3, "endByte": call["startByte"] + 4}
        collapsed = {
            "message": "this project has no accepted reactivity contract for m; solid-checker ...",
            "primaryLocation": first,
            "relatedLocations": [first, third, third, member],
        }
        sites, unattributed, arguments = import_sites(collapsed, {})
        assert sites == collections.Counter({("m", "a"): 1, ("m", "b"): 1, ("m", "default"): 1, ("m", "e"): 1, ("m", "f"): 1}), sites
        assert not unattributed and arguments == 0
        # One repeat for two value bindings: which one raised it is not in the bytes.
        partial = dict(collapsed, relatedLocations=[])
        sites, unattributed, _ = import_sites(partial, {})
        assert not sites and unattributed == collections.Counter({"m": 1}), (sites, unattributed)
        claims = {
            "message": "the reactivity contract for m leaves reads,returns unknown for imported export a; code ...",
            "primaryLocation": first,
            "relatedLocations": [first, first],
        }
        assert import_sites(claims, {})[0] == collections.Counter({("m", "a"): 3})
        argument = {
            "message": "the reactivity contract for m states callbacks for imported export a, but this call site gives ...",
            "primaryLocation": member,
        }
        assert import_sites(argument, {})[2] == 1
    print("self-test passed")
    return 0


def package_of(module):
    parts = module.split("/")
    return "/".join(parts[:2]) if module.startswith("@") else parts[0]


def reason_class(reason):
    if reason.startswith("no recipe"):
        return "noRecipe"
    if reason.startswith("census refused"):
        leg = LEG.search(reason)
        return "census:" + (leg.group(1) if leg else reason[len("census refused: "):][:60])
    return reason[:40]


def main():
    if sys.argv[1:] == ["--self-test"]:
        return self_test()
    parser = argparse.ArgumentParser()
    parser.add_argument("--checker", required=True)
    parser.add_argument("--typefacts", required=True)
    parser.add_argument("--report", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("roots", nargs="+")
    args = parser.parse_args()
    os.makedirs(args.out, exist_ok=True)

    demand = collections.Counter()
    projects = collections.defaultdict(set)
    status = collections.Counter()
    gates = collections.Counter()
    demand_gates = collections.defaultdict(collections.Counter)
    unattributed = collections.Counter()
    argument_sites = 0
    sources = {}
    analyzed = 0
    for root in args.roots:
        for project in sorted(projects_under(root)):
            ident, result = analyze(args.checker, args.typefacts, project, args.out)
            if result is None:
                status["unparsable"] += 1
                continue
            analyzed += 1
            status[result.get("status")] += 1
            for finding in result.get("findings", []):
                if finding.get("id") != "SC9005":
                    continue
                context = finding.get("analysisContext", "")
                if context == ACCEPTANCE_GATE:
                    gate = "acceptance gate"
                elif context.startswith("unknown-contract-claims:"):
                    gate = "open claims: " + context.split(":", 1)[1]
                elif context.startswith("unbound-contract-claims:"):
                    gate = "unbound claims"
                elif context.startswith("obsolete-policy1"):
                    gate = "obsolete policy 1"
                else:
                    gate = "callback execution (not an import site)"
                gates[gate] += 1
                sites, unnamed, arguments = import_sites(finding, sources)
                argument_sites += arguments
                unattributed.update(unnamed)
                for key, count in sites.items():
                    demand[key] += count
                    demand_gates[key][gate] += count
                    projects[key].add(ident)

    report = json.load(open(args.report))
    closed = collections.defaultdict(set)
    withheld = collections.defaultdict(lambda: collections.defaultdict(collections.Counter))
    for row in report["results"]:
        attempt = row.get("certificationAttempt") or {}
        for entry in (attempt.get("certifiedClosures") or {}).get("closed") or []:
            node = entry.get("node") or {}
            closed[(node.get("package") or row["package"], entry["export"])].update(entry["closed"])
        for entry in attempt.get("withheldClosureDetails") or []:
            node = entry.get("node") or {}
            key = (node.get("package") or row["package"], entry["export"])
            withheld[key][entry["domain"]][reason_class(entry["reason"])] += 1
    in_corpus = {key[0] for key in list(closed) + list(withheld)}

    print(f"projects analyzed: {analyzed}; status: {dict(status)}")
    # Print this before the demand totals: a corpus whose findings all stop at
    # the acceptance gate has no closure-sensitive demand at all, whatever the
    # per-export table below says a closure would move after acceptance.
    print("SC9005 by gate:", gates.most_common())
    print(f"distinct (module, export) demanded: {len(demand)}; import sites: {sum(demand.values())}")
    print(f"import sites whose export the bytes do not settle: {sum(unattributed.values())}", unattributed.most_common(10))
    print(f"argument-site findings (not demand): {argument_sites}")
    by_package = collections.Counter()
    for (module, _), count in demand.items():
        by_package[package_of(module)] += count
    print("by package (call sites):", by_package.most_common(25))
    not_in_corpus = collections.Counter()
    print("\nsites projects package export | closed | open -> blockers")
    for (module, export), count in demand.most_common():
        package = package_of(module)
        if package not in in_corpus:
            not_in_corpus[package] += count
            continue
        key = (package, export)
        closed_domains = sorted(closed.get(key, set()))
        open_domains = {
            domain: dict(reasons.most_common(2))
            for domain, reasons in withheld.get(key, {}).items()
            if domain not in closed_domains
        }
        print(
            count, len(projects[(module, export)]), package, export, "|",
            ",".join(closed_domains) or "-", "|",
            json.dumps(open_domains) if open_domains else ("ALL CLOSED" if closed_domains else "no ledger entry"),
        )
    print("\ndemanded packages not in the ecosystem corpus (sites):", not_in_corpus.most_common(20))
    json.dump(
        {
            f"{m}|{e}": {"sites": n, "projects": sorted(projects[(m, e)]), "gates": dict(demand_gates[(m, e)])}
            for (m, e), n in demand.items()
        },
        open(os.path.join(args.out, "demand.json"), "w"), indent=1,
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
