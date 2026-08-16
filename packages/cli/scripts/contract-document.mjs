function canonicalSummary(summary) {
  return JSON.stringify(summary);
}

function plainSummary(summary) {
  return (
    !summary.evidence &&
    !summary.reactiveReads?.length &&
    !summary.returns &&
    !summary.callbacks?.length &&
    !summary.asyncBehavior
  );
}

export function normalizeContract(contract) {
  const unique = new Map();
  for (const entrypoint of Object.values(contract.entrypoints)) {
    for (const summary of Object.values(entrypoint.exports)) {
      unique.set(canonicalSummary(summary), summary);
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
