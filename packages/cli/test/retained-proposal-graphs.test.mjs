import { test } from "vitest";
import assert from "node:assert/strict";
import { selectRecoveryPreparation, retainedProposalGraphCases, certifyRetainedProposalSelection } from "../scripts/retained-proposal-graphs.mjs";

const coordinate = index => ({ entrypoint: `./case-${index}`, conditions: ["import"] });
const recovery = (retained, frontier) => {
  const cases = Array.from({ length: retained + frontier }, (_, index) => coordinate(index));
  return { cases, retainedCases: cases.slice(0, retained) };
};

test("large recovery spends the graph budget on missing cases and retains every generated case", () => {
  const input = recovery(118, 4);
  const selected = selectRecoveryPreparation(input, []);
  assert.deepEqual(selected.retainedProposalCases, input.retainedCases);
  assert.deepEqual(selected.graphCases, input.cases.slice(118));
  assert.deepEqual([...selected.retainedProposalCases, ...selected.graphCases], input.cases);
  const small = recovery(20, 4);
  assert.deepEqual(selectRecoveryPreparation(small, []), { graphCases: small.cases, retainedProposalCases: [] });
  assert.deepEqual(selectRecoveryPreparation(small, [], { retainGeneratedCases: true }), {
    graphCases: small.cases.slice(20), retainedProposalCases: small.retainedCases
  });
  const frontier = [coordinate(0)];
  assert.deepEqual(selectRecoveryPreparation(null, frontier), { graphCases: frontier, retainedProposalCases: [] });
});

test("large recovery keeps explicit graph and publication resource bounds", () => {
  assert.throws(() => selectRecoveryPreparation(recovery(118, 33), []), /32-case graph budget/);
  assert.throws(() => selectRecoveryPreparation(recovery(1024, 1), []), /1024-case publication budget/);
  assert.throws(() => selectRecoveryPreparation(recovery(40, 0), []), /missing frontier/);
  const wrong = recovery(118, 4);
  wrong.retainedCases = [coordinate(999)];
  assert.throws(() => selectRecoveryPreparation(wrong, []), /exact retained-case prefix/);
});

function rootsInput() {
  return {
    coordinates: [coordinate(0), coordinate(1)],
    plannings: [0, 1].map(index => ({
      schemaVersion: 1, proposal: "/scratch/original-proposal.json",
      resolution: { importer: "/project/importer.mjs", requestedEntrypoint: `./case-${index}` },
      exportConditions: ["import"], archive: "/scratch/root.tgz", registryMetadata: "/scratch/metadata.json"
    })),
    sourceDependenciesByInput: [[{ archive: "/scratch/source-a.tgz" }], [{ archive: "/scratch/source-b.tgz" }]],
    lockfile: "/project/bun.lock", lockLocator: "root"
  };
}

test("retained roots preserve proposal, importer, source and lock inputs without semantic child receipts", () => {
  const input = rootsInput();
  const cases = retainedProposalGraphCases(input);
  for (const [index, item] of cases.entries()) {
    assert.equal(item.root.planning, input.plannings[index]);
    assert.equal(item.root.sourceDependencies, input.sourceDependenciesByInput[index]);
    assert.equal(item.artifactCase, input.coordinates[index]);
    assert.deepEqual(item.nodes, [item.root]);
    assert.equal(item.root.node.bunLockPath, input.lockfile);
    assert.equal(item.root.node.lockLocator, input.lockLocator);
  }
  for (const mutate of [
    x => { x.plannings[0].resolution.importer = "/project/other.mjs"; },
    x => { x.plannings[0].archive = "/scratch/other.tgz"; },
    x => { x.sourceDependenciesByInput[0] = []; },
    x => { x.lockLocator = "nested/root"; }
  ]) {
    const changed = rootsInput();
    mutate(changed);
    assert.notEqual(retainedProposalGraphCases(changed)[0].root.node.key, cases[0].root.node.key);
  }
});

test("retained roots refuse incomplete, duplicate and conflicting coordinates", () => {
  for (const mutate of [
    x => { x.sourceDependenciesByInput.pop(); },
    x => { x.lockLocator = ""; },
    x => { x.coordinates[0].entrypoint = "./other"; },
    x => { x.coordinates[0].conditions = ["solid"]; },
    x => { x.coordinates[1] = x.coordinates[0]; x.plannings[1] = x.plannings[0]; },
    x => { x.plannings[0].sourceDependencies = [{ archive: "/other.tgz" }]; }
  ]) {
    const input = rootsInput();
    mutate(input);
    assert.throws(() => retainedProposalGraphCases(input));
  }
});

const proofRefusal = reason => Object.assign(new Error(reason), { owner: "certifier", stage: "witness-acquisition" });
const isProofRefusal = error => error.owner === "certifier" && error.stage === "witness-acquisition";

test("large selection preserves all retained cases, names the unproved frontier, and rechecks the final union", async () => {
  const census = recovery(118, 4);
  const cases = census.cases.map((_, index) => index);
  const attempts = [];
  await certifyRetainedProposalSelection({ cases, recovery: census, isProofRefusal,
    certify: async (selected, publish) => {
      attempts.push({ selected: [...selected], publish });
      if (selected.includes(120)) throw proofRefusal("unproved frontier case");
    }
  });
  assert.deepEqual(census.publishedCases, census.cases.filter((_, index) => index !== 120));
  assert.deepEqual(census.caseRefusals.map(x => x.entrypoint), ["./case-120"]);
  for (const attempt of attempts) assert.deepEqual(attempt.selected.slice(0, 118), cases.slice(0, 118));
  assert.deepEqual(attempts.at(-1), { selected: cases.filter(x => x !== 120), publish: true });
  assert.deepEqual(attempts.filter(x => x.publish).map(x => x.selected.length), [122, 121]);
});

test("a refused retained baseline never publishes a reduced retained set", async () => {
  const census = recovery(118, 4);
  const attempts = [];
  await assert.rejects(certifyRetainedProposalSelection({ cases: census.cases, recovery: census, isProofRefusal,
    certify: async (selected, publish) => {
      attempts.push({ length: selected.length, publish });
      throw proofRefusal("retained case unproved in this context");
    }
  }), /retained case unproved/);
  assert.deepEqual(attempts, [{ length: 122, publish: true }, { length: 118, publish: false }]);
  assert.equal(census.publishedCases, undefined);
});

test("trial successes cannot override a conflicting final union or an infrastructure failure", async () => {
  for (const infrastructure of [false, true]) {
    const census = recovery(33, 1);
    let attempts = 0;
    await assert.rejects(certifyRetainedProposalSelection({ cases: census.cases, recovery: census, isProofRefusal,
      certify: async (_selected, publish) => {
        attempts++;
        if (infrastructure) throw new Error("infrastructure failure");
        if (publish) throw proofRefusal("conflicting final union");
      }
    }), infrastructure ? /infrastructure failure/ : /conflicting final union/);
    assert.equal(census.publishedCases, undefined);
    assert.equal(attempts, infrastructure ? 1 : 4);
  }
});

const floorFor = census => ({ acceptedCases: census.retainedCases.slice(1),
  caseRefusals: [{ ...census.retainedCases[0], ...proofRefusal("unaccepted generated case"), reason: "unaccepted generated case" }],
  publication: { documentDigest: "sha256:private-native-publication" } });

test("a verified ordinary floor excludes an explicitly refused generated case and retains graph successes", async () => {
  const census = recovery(33, 2), cases = census.cases.map((_, index) => index), attempts = [];
  await certifyRetainedProposalSelection({ cases, recovery: census, isProofRefusal,
    establishFloor: async () => floorFor(census),
    certify: async (selected, publish) => {
      attempts.push({ selected: [...selected], publish });
      if (selected.includes(0)) throw proofRefusal("generated baseline is not fully certified");
      return "accepted union";
    }
  });
  assert.deepEqual(attempts.map(x => [x.selected.length, x.publish]), [[35, true], [33, false], [34, true]]);
  assert.deepEqual(census.publishedCases, census.cases.slice(1));
  assert.deepEqual(census.caseRefusals.map(x => x.entrypoint), ["./case-0"]);
  assert.deepEqual(census.cases, Array.from({ length: 35 }, (_, index) => coordinate(index)));
  assert.equal(census.verifiedRetainedFloor.acceptedCases.length, 32);
});

test("verified floor failures in graph context cannot drop accepted retained cases", async () => {
  const census = recovery(33, 2), attempts = [];
  await assert.rejects(certifyRetainedProposalSelection({ cases: census.cases, recovery: census, isProofRefusal,
    establishFloor: async () => floorFor(census),
    certify: async (selected, publish) => { attempts.push({ selected, publish }); throw proofRefusal("accepted retained case cannot certify here"); }
  }), /accepted retained case/);
  assert.equal(census.publishedCases, undefined);
  assert.deepEqual(attempts.at(-1), { selected: census.retainedCases.slice(1), publish: false });
  assert.equal(attempts.length, 4);
});

test("verified floors require an exact positive/refused partition and never infer omitted cases", async () => {
  for (const mutate of [
    floor => { floor.caseRefusals = []; },
    floor => { floor.acceptedCases.pop(); },
    floor => { floor.acceptedCases.push(floor.acceptedCases[0]); },
    floor => { floor.acceptedCases[0] = coordinate(100); },
    floor => { floor.caseRefusals[0].owner = "probe-gate"; },
    floor => { floor.caseRefusals[0].reason = ""; },
    floor => { delete floor.publication; },
    floor => { floor.acceptedCases = []; }
  ]) {
    const census = recovery(33, 2), floor = floorFor(census); mutate(floor);
    let calls = 0;
    await assert.rejects(certifyRetainedProposalSelection({ cases: census.cases, recovery: census, isProofRefusal,
      establishFloor: async () => floor,
      certify: async () => { calls++; throw proofRefusal("baseline"); }
    }), /verified retained floor/);
    assert.equal(calls, 2);
    assert.equal(census.publishedCases, undefined);
  }
});
