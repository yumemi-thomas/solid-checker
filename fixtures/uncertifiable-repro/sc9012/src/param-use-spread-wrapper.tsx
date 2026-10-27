// ai-memory-ui screens/audit.tsx:249 (lib/admin-api.ts:213): a component hands a scope object to a project
// function that spreads it into a literal it passes to another project function.
type Scope = { workspace: string; project: string };

function json(body: object): RequestInit {
  return { method: "POST", body: JSON.stringify(body) };
}

function adminLint(scope: Scope, options: { dry_run?: boolean } = {}): Promise<Response> {
  return fetch("/lint", json({ ...scope, ...options }));
}

export function Audit() {
  const scope: Scope = { workspace: "main", project: "docs" };
  const report = adminLint(scope, { dry_run: true });
  return <button onClick={() => void report}>Lint</button>;
}
