// ai-memory-ui lib/api.ts:70-81: the helper the leaf callback in
// leaf-project-function.tsx reaches, a fetch read in a `try`.
export type CurrentUser = { name: string };

export async function fetchCurrentUser(): Promise<CurrentUser | null> {
  try {
    const response = await fetch("/oauth2/userinfo", { headers: { accept: "application/json" } });
    if (!response.ok) return null;
    return (await response.json()) as CurrentUser;
  } catch {
    return null;
  }
}
