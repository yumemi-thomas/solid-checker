function semanticValue(value) {
  if (Array.isArray(value)) return value.map(semanticValue);
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.entries(value)
      .filter(([key]) => key !== "evidence" && key !== "variants")
      .map(([key, child]) => [key, semanticValue(child)])
  );
}

function mergeEvidence(left, right) {
  // Evidence for a narrower condition cannot promote the broader summary it
  // collapses into. Preserve the representative's trust level and only union
  // observations when both sides are already independently probed.
  if (!left || !right || JSON.stringify(left) === JSON.stringify(right)) return left;
  if (left.kind === "probed" && right.kind === "probed") {
    return {
      kind: "probed",
      modes: [...new Set([...(left.modes ?? []), ...(right.modes ?? [])])].sort(),
      calls: Math.max(left.calls ?? 1, right.calls ?? 1)
    };
  }
  return left;
}

function mergeEquivalentEvidence(left, right) {
  if (Array.isArray(left) && Array.isArray(right)) {
    return left.map((value, index) => mergeEquivalentEvidence(value, right[index]));
  }
  if (left && right && typeof left === "object" && typeof right === "object") {
    const merged = { ...left };
    for (const [key, value] of Object.entries(right)) {
      if (key === "evidence") merged.evidence = mergeEvidence(merged.evidence, value);
      else if (key !== "variants" && key in merged) {
        merged[key] = mergeEquivalentEvidence(merged[key], value);
      } else if (key !== "variants") merged[key] = value;
    }
    return merged;
  }
  return left;
}

function conditionsContain(container, contained) {
  return contained.every(condition => container.includes(condition));
}

function removeRedundantVariants(variants) {
  const kept = [];
  for (const variant of [...variants].sort(
    (left, right) =>
      left.conditions.length - right.conditions.length ||
      JSON.stringify(left.conditions).localeCompare(JSON.stringify(right.conditions))
  )) {
    const covering = kept.find(
      candidate =>
        JSON.stringify(semanticValue(candidate.summary)) ===
          JSON.stringify(semanticValue(variant.summary)) &&
        conditionsContain(variant.conditions, candidate.conditions)
    );
    if (covering) {
      covering.summary = mergeEquivalentEvidence(covering.summary, variant.summary);
    } else {
      kept.push(variant);
    }
  }
  return kept;
}

function collapseEvidenceOnlyVariants(summary, entrypointConditions = []) {
  const variants = removeRedundantVariants(
    (summary.variants ?? []).map(variant => ({
      ...variant,
      summary: collapseEvidenceOnlyVariants(variant.summary)
    }))
  );
  if (!variants.length) return summary;
  // A variant set encodes one of two different facts. Usually it is branches
  // that differ only in evidence, and collapsing them loses nothing. But it
  // can also record *where the export exists at all*: when the set does not
  // cover every condition its entrypoint resolves under, the uncovered
  // environments are ones this export was never observed in. Collapsing then
  // republishes one branch's summary as unconditional and hands a consumer in
  // an uncovered environment a complete claim about an export that is absent
  // there, so the gating has to survive.
  const covered = new Set(variants.flatMap(variant => variant.conditions ?? []));
  if (entrypointConditions.some(condition => !covered.has(condition))) {
    return { ...summary, variants };
  }
  const base = { ...summary };
  delete base.variants;
  const semantic = JSON.stringify(semanticValue(base));
  if (variants.every(variant => JSON.stringify(semanticValue(variant.summary)) === semantic)) {
    return variants.reduce(
      (merged, variant) => mergeEquivalentEvidence(merged, variant.summary),
      base
    );
  }
  return { ...summary, variants };
}

function canonicalSummary(summary) {
  return JSON.stringify(summary);
}

function plainSummary(summary) {
  return (
    !summary.evidence &&
    summary.reactiveReads?.status !== "unknown" &&
    !summary.reactiveReads?.length &&
    summary.returns?.status !== "unknown" &&
    !summary.returns &&
    summary.callbacks?.status !== "unknown" &&
    !summary.callbacks?.length &&
    summary.ownerRequirements?.status !== "unknown" &&
    !summary.ownerRequirements?.length &&
    !summary.variants?.length &&
    summary.asyncBehavior?.status !== "unknown" &&
    !summary.asyncBehavior
  );
}

export function normalizeContract(contract) {
  const unique = new Map();
  for (const entrypoint of Object.values(contract.entrypoints)) {
    for (const [name, summary] of Object.entries(entrypoint.exports)) {
      const collapsed = collapseEvidenceOnlyVariants(summary, entrypoint.conditions ?? []);
      entrypoint.exports[name] = collapsed;
      unique.set(canonicalSummary(collapsed), collapsed);
    }
  }

  const counters = new Map();
  const ids = new Map();
  const summaries = {};
  for (const [canonical, summary] of [...unique].sort(([left], [right]) =>
    left.localeCompare(right)
  )) {
    let id;
    if (plainSummary(summary)) {
      id = summary.kind;
    } else {
      const next = (counters.get(summary.kind) ?? 0) + 1;
      counters.set(summary.kind, next);
      id = `${summary.kind}-${next}`;
    }
    ids.set(canonical, id);
    summaries[id] = summary;
  }

  const entrypoints = {};
  const surfaces = new Map();
  for (const [name, entrypoint] of Object.entries(contract.entrypoints).sort(([left], [right]) =>
    left.localeCompare(right)
  )) {
    const exports = {};
    for (const [exportName, summary] of Object.entries(entrypoint.exports).sort(
      ([left], [right]) => left.localeCompare(right)
    )) {
      const id = ids.get(canonicalSummary(summary));
      (exports[id] ??= []).push(exportName);
    }
    const surface = JSON.stringify(exports);
    const sameAs = surfaces.get(surface);
    if (!sameAs) surfaces.set(surface, name);
    entrypoints[name] = {
      ...(sameAs ? { sameAs } : { exports }),
      ...(entrypoint.conditions?.length ? { conditions: [...entrypoint.conditions] } : {})
    };
  }

  return {
    schemaVersion: contract.schemaVersion,
    package: contract.package,
    compilerFactsProtocol: contract.compilerFactsProtocol,
    ...(contract.artifacts &&
    (contract.artifacts.declaration || contract.artifacts.implementation)
      ? { artifacts: contract.artifacts }
      : {}),
    summaries,
    entrypoints,
    evidence: contract.evidence
  };
}

export function expandContract(document) {
  if (!document.summaries) {
    throw new Error("contract document is not normalized");
  }
  const expanded = new Map();
  const visiting = new Set();
  const expandEntrypoint = name => {
    if (expanded.has(name)) return expanded.get(name);
    const entrypoint = document.entrypoints[name];
    if (!entrypoint) throw new Error(`entrypoint alias targets missing entrypoint ${name}`);
    if (visiting.has(name)) throw new Error(`entrypoint alias cycle includes ${name}`);
    visiting.add(name);
    let exports;
    if (entrypoint.sameAs) {
      if (entrypoint.exports) {
        throw new Error(`entrypoint ${name} cannot declare both exports and sameAs`);
      }
      exports = { ...expandEntrypoint(entrypoint.sameAs) };
    } else {
      exports = {};
      for (const [summaryId, names] of Object.entries(entrypoint.exports ?? {})) {
        const summary = document.summaries[summaryId];
        if (!summary) throw new Error(`entrypoint ${name} references ${summaryId}`);
        for (const exportName of names) {
          if (exportName in exports) {
            throw new Error(`entrypoint ${name} repeats export ${exportName}`);
          }
          exports[exportName] = summary;
        }
      }
    }
    visiting.delete(name);
    expanded.set(name, exports);
    return exports;
  };

  return {
    schemaVersion: document.schemaVersion,
    package: document.package,
    compilerFactsProtocol: document.compilerFactsProtocol,
    artifacts: document.artifacts ?? {},
    entrypoints: Object.fromEntries(
      Object.entries(document.entrypoints).map(([name, entrypoint]) => [
        name,
        {
          exports: expandEntrypoint(name),
          ...(entrypoint.conditions?.length ? { conditions: entrypoint.conditions } : {})
        }
      ])
    ),
    evidence: document.evidence
  };
}
