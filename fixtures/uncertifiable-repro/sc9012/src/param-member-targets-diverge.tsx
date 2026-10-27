// ai-memory-ui scope-wiki.tsx:30, scope-overview.tsx:31-39 with lib/query.ts, lib/api.ts:194: a `queryFn` handed to a
// project query hook calls a project function with an object of prop reads, which passes `key.workspace` on.
import { createMemo } from "solid-js";

function useQuery(options: () => { queryFn: () => Promise<string[]>; queryKey: readonly string[] }) {
  const resolved = createMemo(options);
  return { isPending: resolved().queryKey.length === 0 };
}

function segment(value: string): string {
  return encodeURIComponent(value);
}

async function listPages(key: { workspace: string; project: string }): Promise<string[]> {
  const response = await fetch(`/workspaces/${segment(key.workspace)}/projects/${segment(key.project)}/pages`);
  return (await response.json()) as string[];
}

export function Wiki(props: { project: string; workspace: string }) {
  const pages$ = useQuery(() => ({
    queryFn: () => listPages({ project: props.project, workspace: props.workspace }),
    queryKey: ["pages", props.workspace, props.project],
  }));
  return <p>{pages$.isPending ? "loading" : "loaded"}</p>;
}

// Referenced as an object member (a route table): the `escaped-component:object-member-reference` variant.
export function Overview(props: { project: string; workspace: string }) {
  const pages$ = useQuery(() => ({
    queryFn: () => listPages({ project: props.project, workspace: props.workspace }),
    queryKey: ["overview", props.workspace, props.project],
  }));
  return <p>{pages$.isPending ? "loading" : "loaded"}</p>;
}

export const route = { component: Overview };
