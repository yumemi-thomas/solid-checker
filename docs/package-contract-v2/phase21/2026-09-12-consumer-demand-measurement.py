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
"""
import argparse
import collections
import json
import os
import re
import subprocess
import sys

DEMAND = re.compile(
    r"the reactivity contract for (\S+) has no entrypoint/export summary for imported export (\S+);"
)
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
                    gates["acceptance gate"] += 1
                elif context.startswith("unknown-contract-claims:"):
                    gates["open claims: " + context.split(":", 1)[1]] += 1
                elif context.startswith("unbound-contract-claims:"):
                    gates["unbound claims"] += 1
                elif context.startswith("obsolete-policy1"):
                    gates["obsolete policy 1"] += 1
                else:
                    gates["callback execution (not an import site)"] += 1
                match = DEMAND.search(finding.get("message", ""))
                if match:
                    key = (match.group(1), match.group(2))
                    demand[key] += 1
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
    print(f"distinct (module, export) demanded: {len(demand)}; call sites: {sum(demand.values())}")
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
        {f"{m}|{e}": {"sites": n, "projects": sorted(projects[(m, e)])} for (m, e), n in demand.items()},
        open(os.path.join(args.out, "demand.json"), "w"), indent=1,
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
