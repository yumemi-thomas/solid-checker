import { createHash } from "node:crypto";

export const RECOVERY_GRAPH_CASE_BUDGET = 32;
export const RECOVERY_CASE_SET_BUDGET = 1024;

export function recoveryGraphBudgetRefusal(caseCount) {
  if (caseCount <= RECOVERY_GRAPH_CASE_BUDGET) return null;
  return `entrypoint recovery prepares ${caseCount} artifact cases, above the ${RECOVERY_GRAPH_CASE_BUDGET}-case graph budget`;
}

// Small sets retain the measured dependency-graph strategy. Large sets keep
// their generated proposals and spend the graph budget on the missing cases.
// Both kinds of root still require one fresh native case-set transaction.
export function selectRecoveryPreparation(recovery, dependencyCases, { retainGeneratedCases = false } = {}) {
  if (!recovery) return { graphCases: dependencyCases, retainedProposalCases: [] };
  const { cases, retainedCases } = recovery;
  if (cases.length > RECOVERY_CASE_SET_BUDGET) {
    throw new Error(`entrypoint recovery exceeds the ${RECOVERY_CASE_SET_BUDGET}-case publication budget`);
  }
  if (JSON.stringify(cases.slice(0, retainedCases.length)) !== JSON.stringify(retainedCases)) {
    throw new Error("entrypoint recovery requires the exact retained-case prefix");
  }
  if (cases.length <= RECOVERY_GRAPH_CASE_BUDGET && !retainGeneratedCases) {
    return { graphCases: cases, retainedProposalCases: [] };
  }
  const graphCases = cases.slice(retainedCases.length);
  const refusal = recoveryGraphBudgetRefusal(graphCases.length);
  if (refusal) throw new Error(refusal);
  if (!retainedCases.length || !graphCases.length) {
    throw new Error("large entrypoint recovery requires retained proposals and a missing frontier");
  }
  return { graphCases, retainedProposalCases: retainedCases };
}

// These are acquisition requests, never accepted evidence. The native graph
// planner authenticates every archive and lock selection, derives all demands,
// and refuses a proposal with any unmet semantic dependency. Compiler sources
// belong to their original input, exactly as in ordinary proposal certification.
export function retainedProposalGraphCases({ plannings, sourceDependenciesByInput, coordinates, lockfile, lockLocator }) {
  if (!plannings.length || plannings.length !== coordinates.length ||
      plannings.length !== sourceDependenciesByInput.length || !lockfile || !lockLocator) {
    throw new Error("retained proposal roots require complete positional inputs and an exact lock selection");
  }
  const seen = new Set();
  return plannings.map((planning, index) => {
    const coordinate = coordinates[index];
    const conditions = [...new Set([...coordinate.conditions, "import"])].sort();
    if (planning.resolution.requestedEntrypoint !== coordinate.entrypoint ||
        JSON.stringify([...planning.exportConditions].sort()) !== JSON.stringify(conditions)) {
      throw new Error("retained proposal root disagrees with its entrypoint or conditions");
    }
    const selection = JSON.stringify([coordinate.entrypoint, conditions]);
    if (seen.has(selection)) throw new Error("retained proposal roots repeat a case selection");
    seen.add(selection);
    const sourceDependencies = sourceDependenciesByInput[index];
    if (!Array.isArray(sourceDependencies) || planning.sourceDependencies?.length) {
      throw new Error("retained proposal root requires one explicit compiler-source set");
    }
    const input = { planning, sourceDependencies, lockfile, lockLocator };
    const key = `retained-proposal:${createHash("sha256").update(JSON.stringify(input)).digest("hex")}`;
    const root = { index, planning, sourceDependencies,
      node: { key, bunLockPath: lockfile, lockLocator } };
    return { root, nodes: [root], artifactCase: coordinate };
  });
}

// Retained proposal roots are mandatory unless fresh ordinary certification
// establishes their exact accepted/refused partition. Every accepted root is
// then mandatory in graph context; a failure cannot turn into lost coverage.
// Private successes select whole cases only. Publication rechecks their union.
export async function certifyRetainedProposalSelection({ cases, recovery, certify, isProofRefusal, establishFloor = null }) {
  const retainedCount = recovery.retainedCases.length;
  if (!retainedCount || retainedCount >= cases.length ||
      cases.length > RECOVERY_CASE_SET_BUDGET ||
      cases.length - retainedCount > RECOVERY_GRAPH_CASE_BUDGET ||
      cases.length !== recovery.cases.length ||
      JSON.stringify(recovery.cases.slice(0, retainedCount)) !== JSON.stringify(recovery.retainedCases)) {
    throw new Error("retained proposal selection requires an exact bounded case census");
  }
  try {
    const result = await certify(cases, true);
    recovery.publishedCases = [...recovery.cases];
    return result;
  } catch (error) {
    if (!isProofRefusal(error)) throw error;
    recovery.combinedRefusal = error.reason ?? error.message;
  }
  recovery.caseRefusals = [];
  let selected = cases.slice(0, retainedCount);
  let accepted = recovery.cases.slice(0, retainedCount);
  try {
    await certify(selected, false);
  } catch (error) {
    if (!isProofRefusal(error) || !establishFloor) throw error;
    recovery.retainedBaselineRefusal = error.reason ?? error.message;
    const floor = await establishFloor();
    const key = item => JSON.stringify([item.entrypoint, [...item.conditions].sort()]);
    const expected = new Map(recovery.retainedCases.map((item, index) => [key(item), index]));
    if (expected.size !== retainedCount || !floor?.acceptedCases?.length || !Array.isArray(floor.caseRefusals) || !floor.publication) throw new Error("verified retained floor requires an exact positive and refused census");
    const seen = new Set(), indexes = [];
    for (const item of floor.acceptedCases) {
      const identity = key(item);
      if (!expected.has(identity) || seen.has(identity)) throw new Error("verified retained floor has an unknown or duplicate accepted case");
      seen.add(identity);
      indexes.push(expected.get(identity));
    }
    for (const item of floor.caseRefusals) {
      const identity = key(item);
      if (!expected.has(identity) || seen.has(identity) || item.owner !== "certifier" || item.stage !== "witness-acquisition" || typeof item.reason !== "string" || !item.reason) throw new Error("verified retained floor has an invalid explicit refusal");
      seen.add(identity);
    }
    if (seen.size !== retainedCount) throw new Error("verified retained floor cannot infer a refusal from an absent case");
    indexes.sort((a, b) => a - b);
    selected = indexes.map(index => cases[index]);
    accepted = indexes.map(index => recovery.cases[index]);
    recovery.verifiedRetainedFloor = floor;
    recovery.caseRefusals = [...floor.caseRefusals];
    // A newly established ordinary floor is still mandatory in graph context.
    // Try its complete union before paying for individual frontier trials.
    try {
      const result = await certify([...selected, ...cases.slice(retainedCount)], true);
      recovery.publishedCases = [...accepted, ...recovery.cases.slice(retainedCount)];
      return result;
    } catch (failure) {
      if (!isProofRefusal(failure)) throw failure;
      recovery.verifiedFloorUnionRefusal = failure.reason ?? failure.message;
    }
    await certify(selected, false);
  }
  for (let index = retainedCount; index < cases.length; index++) {
    try {
      await certify([...selected, cases[index]], false);
      selected.push(cases[index]);
      accepted.push(recovery.cases[index]);
    } catch (error) {
      if (!isProofRefusal(error)) throw error;
      recovery.caseRefusals.push({ ...recovery.cases[index], stage: error.stage,
        owner: error.owner, demandId: error.demandId ?? null,
        family: error.family ?? null, reason: error.reason ?? error.message });
    }
  }
  const result = await certify(selected, true);
  recovery.publishedCases = accepted;
  return result;
}
