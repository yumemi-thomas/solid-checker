// A corpus manifest names each app's repository and commit (`apps`), so the
// corpus can be rebuilt exactly; a sweep over a checkout nobody can rebuild is
// not comparable with any other.

const FIELDS = ["repository", "commit"];

/** Throws, naming the app, unless every app a project row uses has both. */
export function assertManifestProvenance(manifest, manifestPath = "manifest") {
  const problems = [];
  for (const app of new Set((manifest.projects ?? []).map((row) => row.app))) {
    const entry = manifest.apps?.[app];
    for (const field of FIELDS) {
      if (typeof entry?.[field] !== "string" || entry[field] === "") {
        problems.push(`app ${app} has no ${field}`);
      }
    }
    if (typeof entry?.commit === "string" && entry.commit !== "" && !/^[0-9a-f]{40}$/.test(entry.commit)) {
      problems.push(`app ${app} commit is not a full commit hash: ${entry.commit}`);
    }
  }
  if (problems.length > 0) {
    throw new Error(
      `${manifestPath}: refusing a corpus that cannot be rebuilt at its commits: ${problems.join("; ")}`,
    );
  }
}
