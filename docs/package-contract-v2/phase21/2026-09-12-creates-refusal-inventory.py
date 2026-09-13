#!/usr/bin/env python3
"""Classify retained creates census refusals without running the analyzer."""

import argparse
import collections
import hashlib
import json
import re
from pathlib import Path


DEPTH_EXHAUSTION = re.compile(
    r"^census refused: creates census exceeds ([0-9]+) local-recursion hops at "
)
INVOKING_FORM = re.compile(
    r"^census refused: creates census refuses an uncensused invoking form: ([a-z-]+) "
)
REVIEWED_INVOKING_FORMS = {
    "property-access-unknown-accessor", "coercion", "iteration-protocol",
    "instanceof", "jsx-element", "await-then", "get-accessor",
}


def family(reason):
    if DEPTH_EXHAUSTION.match(reason):
        return "local-recursion-depth-exhaustion"
    invoking = INVOKING_FORM.match(reason)
    if invoking:
        name = invoking.group(1)
        return name if name in REVIEWED_INVOKING_FORMS else "unclassified-invoking-form"
    phrases = [
        ("caller-supplied code", "caller-supplied-callable"),
        ("the census does not trace values", "initializer-value-tracing"),
        ("its binding is written", "written-local-binding"),
        ("standard-library member", "standard-invoker-callback-resolution"),
        ("neither a default-library member", "outside-authenticated-runtime"),
        ("not in this artifact's own runtime source", "outside-authenticated-runtime"),
        ("no control-flow census", "missing-control-flow-transcript"),
        ("no function-like declaration", "missing-function-declaration"),
        ("an unresolved callee", "unresolved-callee"),
        ("the same name is declared again", "merged-local-binding"),
        ("runtime implementation transcript is incomplete or open", "open-implementation-transcript"),
    ]
    return next((name for phrase, name in phrases if phrase in reason), "unclassified")


def attributed_identity(detail):
    node = detail.get("node")
    if not isinstance(node, dict):
        return None
    if not all(isinstance(node.get(key), str) and node[key] for key in ("package", "version")):
        return None
    return (node["package"], node["version"], detail["export"])


def counts(rows):
    return {
        "rows": len(rows),
        "distinctClaimIds": len({row["claimId"] for row in rows}),
        "distinctAttributedExports": len({row["identity"] for row in rows if row["identity"]}),
        "unattributedRows": sum(row["identity"] is None for row in rows),
        "unattributedClaimIds": len({row["claimId"] for row in rows if row["identity"] is None}),
        "rootProbes": len({row["probeId"] for row in rows}),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    args = parser.parse_args()
    raw = args.report.read_bytes()
    report = json.loads(raw)
    rows = []
    for probe in report["results"]:
        attempt = probe.get("certificationAttempt") or {}
        for detail in attempt.get("withheldClosureDetails", []):
            reason = detail.get("reason", "")
            if detail.get("domain") != "creates" or not reason.startswith("census refused:"):
                continue
            claim_id = detail.get("semanticClaimId")
            if not isinstance(claim_id, str) or not claim_id:
                raise ValueError("a creates census-refusal detail lacks its semanticClaimId")
            rows.append({
                "claimId": claim_id,
                "identity": attributed_identity(detail),
                "export": detail["export"],
                "artifactCase": detail["artifactCase"],
                "probeId": probe["probeId"],
                "probeDurationMs": probe.get("durationMs"),
                "certificationDurationMs": attempt.get("durationMs"),
                "family": family(reason),
                "reason": reason,
            })
    grouped = collections.defaultdict(list)
    for row in rows:
        grouped[row["family"]].append(row)
    families = []
    for name, members in sorted(grouped.items(), key=lambda item: (-len(item[1]), item[0])):
        known = collections.defaultdict(list)
        for row in members:
            if row["identity"]:
                known[row["identity"]].append(row)
        examples = []
        for identity, occurrences in sorted(known.items(), key=lambda item: (-len(item[1]), item[0]))[:3]:
            shortest = min(occurrences, key=lambda row: (
                row["probeDurationMs"] if isinstance(row["probeDurationMs"], (int, float)) else float("inf"),
                row["probeId"],
            ))
            examples.append({
                "package": identity[0], "version": identity[1], "export": identity[2],
                "rows": len(occurrences),
                "distinctClaimIds": len({row["claimId"] for row in occurrences}),
                "shortestObservedProbe": shortest["probeId"],
                "probeDurationMs": shortest["probeDurationMs"],
                "certificationDurationMs": shortest["certificationDurationMs"],
                "exampleClaimId": shortest["claimId"],
                "exampleReason": shortest["reason"],
            })
        families.append({"family": name, **counts(members), "examples": examples})
    depth_rows = [row for row in rows if DEPTH_EXHAUSTION.match(row["reason"])]
    result = {
        "report": {
            "path": str(args.report),
            "sha256": hashlib.sha256(raw).hexdigest(),
            "startedAt": report.get("startedAt"), "finishedAt": report.get("finishedAt"),
            "probes": len(report["results"]),
        },
        "createsCensusRefusals": counts(rows),
        "families": families,
        "localRecursionDepthExhaustion": {
            **counts(depth_rows),
            "reportedHopLimits": dict(collections.Counter(
                DEPTH_EXHAUSTION.match(row["reason"]).group(1) for row in depth_rows
            )),
            "match": DEPTH_EXHAUSTION.pattern,
        },
        "limitations": [
            "Only root results are read; family summaries repeat them.",
            "Families classify refusal wording, not whether a claim is true or provable.",
            "Export counts require node package/version; missing attribution stays unknown.",
            "A claim can appear in multiple families across attempts; family claim counts need not sum to the total.",
            "The reported reason is the first observed blocker, not an exhaustive census of blockers.",
            "Shortest observed probe duration is historical wall time, not a predicted rerun duration.",
            "Generic depth mentions never count as local-recursion budget exhaustion.",
        ],
    }
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
