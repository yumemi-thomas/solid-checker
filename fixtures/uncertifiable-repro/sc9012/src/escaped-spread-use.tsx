// kui feature-ksql KsqlRoute.tsx:163: the component is rendered with a spread (`<Reporting {...rest} />`), so
// its callers are unknown; the string prop it hands a project function from a load closure is not proven a primitive.

async function api(route: string, init: { path: Record<string, string> }): Promise<{ status: number }> {
  return { status: route.length + Object.keys(init.path).length };
}

const readReporting = async (projectId: string) => {
  const response = await api("/projects/{project_id}/reporting", { path: { project_id: projectId } });
  return response.status === 200;
};

function useQuery(options: { load: () => Promise<boolean> }) {
  const pending = options.load();
  return () => pending;
}

function Reporting(properties: { projectId: string }) {
  const enabled = useQuery({ load: () => readReporting(properties.projectId) });
  return <p>{String(enabled())}</p>;
}

export function Page(rest: { projectId: string }) {
  return <Reporting {...rest} />;
}
