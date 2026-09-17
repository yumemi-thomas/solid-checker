#!/usr/bin/env python3
"""Read retained report/proposals; never run certification or infer missing shapes."""

import argparse
import collections
import glob
import hashlib
import json
from pathlib import Path


def read_json(path):
    return json.loads(Path(path).read_text())


def digest(path):
    return "sha256:" + hashlib.sha256(Path(path).read_bytes()).hexdigest()


def matching_summaries(contract, candidate):
    artifact = candidate["artifact"]
    cases = contract["entrypoints"][artifact["entrypoint"]]["cases"]
    result = []
    for case in cases:
        runtime = case["artifact"]
        declarations = case["declarations"]
        if (
            runtime["path"] == artifact["runtimePath"]
            and "sha256:" + runtime["sha256"] == artifact["runtimeDigest"]
            and "sha256:" + runtime["closureSha256"] == artifact["closureDigest"]
            and declarations["path"] == artifact["declarationsPath"]
            and "sha256:" + declarations["sha256"] == artifact["declarationsDigest"]
        ):
            result.append(contract["summaries"][case["exports"][candidate["subject"]["export"]]])
    if not result or any(item != result[0] for item in result):
        raise ValueError(f"missing or conflicting exact artifact: {candidate['claimId']}")
    return result[0]


def classify(summary):
    call = summary.get("call", {})
    if "returns" not in call:
        return "unknown", None
    operations = {item["id"]: item for item in call.get("operations", [])}
    returns = [operations[item] for item in call["returns"]]
    if not returns:
        return "empty", []
    output = returns[0].get("output", {})
    if (
        len(returns) == 1
        and returns[0].get("kind") == "return"
        and output.get("kind") == "parameter"
        and isinstance(output.get("index"), int)
        and output.get("index") >= 0
        and output.get("path") == []
    ):
        return "whole-parameter-identity", returns
    return "unsupported-shape", returns


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument("--proposal-glob", action="append", default=[])
    args = parser.parse_args()
    report = read_json(args.report)
    rows = []
    for probe in report["results"]:
        attempt = probe.get("certificationAttempt") or {}
        for detail in attempt.get("withheldClosureDetails", []):
            if detail["domain"] == "returns" and detail["reason"] == "no recipe in corpus":
                node = detail.get("node")
                rows.append({
                    "claimId": detail["semanticClaimId"],
                    "artifactCase": detail["artifactCase"],
                    "export": detail["export"],
                    "identity": (node["package"], node["version"], detail["export"]) if node else None,
                    "attribution": "report-node" if node else "unattributed",
                    "probeId": probe["probeId"],
                    "lane": attempt.get("lane"),
                })
    ids = {row["claimId"] for row in rows}
    evidence = {}
    paths = sorted({path for pattern in args.proposal_glob for path in glob.glob(pattern)})
    for proposal_path in paths:
        proposal = read_json(proposal_path)
        candidates = [item for item in proposal.get("closureCandidates", []) if item["claimId"] in ids]
        if not candidates:
            continue
        contract_path = proposal_path.removesuffix(".proposal.json")
        if contract_path == proposal_path:
            raise ValueError("proposal filenames must end with .proposal.json")
        contract = read_json(contract_path)
        for candidate in candidates:
            claim_id = candidate["claimId"]
            matching_rows = [row for row in rows if row["claimId"] == claim_id]
            subject = candidate["subject"]
            if not all(
                row["artifactCase"] == subject["artifactCase"]
                and row["export"] == subject["export"]
                for row in matching_rows
            ):
                raise ValueError(f"claim subject mismatch: {claim_id}")
            summary = matching_summaries(contract, candidate)
            package = contract["package"]
            identity = (package["name"], package["version"], subject["export"])
            for row in matching_rows:
                if row["identity"] is not None and row["identity"] != identity:
                    raise ValueError(f"contract package disagrees with attributed claim: {claim_id}")
                if row["identity"] is None:
                    row["identity"] = identity
                    row["attribution"] = "exact-retained-contract"
            shape, operations = classify(summary)
            observation = {"shape": shape, "operations": operations}
            previous = evidence.get(claim_id)
            if previous and previous["observation"] != observation:
                raise ValueError(f"conflicting exact claim observations: {claim_id}")
            if previous:
                continue
            evidence[claim_id] = {
                "observation": observation,
                "proposal": {"path": proposal_path, "sha256": digest(proposal_path)},
                "contract": {"path": contract_path, "sha256": digest(contract_path)},
                "artifact": candidate["artifact"],
                "package": {"name": package["name"], "version": package["version"]},
            }

    def shape_for(claim_id):
        return evidence.get(claim_id, {}).get("observation", {}).get("shape", "unknown")

    identities = collections.defaultdict(list)
    for row in rows:
        if row["identity"] is not None:
            identities[row["identity"]].append(row)
    unattributed = [row for row in rows if row["identity"] is None]
    output = {
        "report": {
            "path": str(args.report), "sha256": digest(args.report),
            "startedAt": report["startedAt"], "finishedAt": report["finishedAt"],
            "probes": len(report["results"]), "checker": report["checker"],
        },
        "proposalFilesRead": len(paths),
        "missingRecipeReturns": {
            "closureRows": len(rows), "distinctClaimIds": len(ids),
            "distinctAttributedPackageVersionExports": len(identities),
            "unattributedClosureRows": len(unattributed),
            "unattributedClaimIds": len({row["claimId"] for row in unattributed}),
            "rowsByAttribution": dict(collections.Counter(row["attribution"] for row in rows)),
            "distinctRootProbes": len({row["probeId"] for row in rows}),
            "rowsByShape": dict(collections.Counter(shape_for(row["claimId"]) for row in rows)),
            "claimsByShape": dict(collections.Counter(shape_for(claim_id) for claim_id in ids)),
        },
        "exports": [{
            "package": identity[0], "version": identity[1], "export": identity[2],
            "closureRows": len(group),
            "distinctClaimIds": len({row["claimId"] for row in group}),
            "rowsByShape": dict(collections.Counter(shape_for(row["claimId"]) for row in group)),
        } for identity, group in sorted(identities.items(), key=lambda item: (-len(item[1]), item[0]))],
        "unattributedRows": unattributed,
        "claims": [{
            "claimId": claim_id, "shape": shape_for(claim_id),
            "closureRows": sum(row["claimId"] == claim_id for row in rows),
            "evidence": evidence.get(claim_id),
        } for claim_id in sorted(ids)],
        "limitations": [
            "Counts use results once; the report repeats results inside family summaries.",
            "A dependency detail belongs to its node package, not the root probe package.",
            "Missing node identities require exact retained contract attribution; root probe identity is never substituted.",
            "Exact retained claim/artifact matching classifies proposed shape only.",
            "No sample eligibility, implementation census, veto, or consumer success is established.",
            "Unknown includes missing retained proposals; no export-name inference is used.",
            "The report stores executable paths, not executable content digests.",
        ],
    }
    print(json.dumps(output, indent=2))


if __name__ == "__main__":
    main()
