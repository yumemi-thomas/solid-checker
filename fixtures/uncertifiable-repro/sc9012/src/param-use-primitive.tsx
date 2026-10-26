// error-menu-web ProjectReporting.tsx:23: an effect's apply passes its
// string parameter to a project function that wraps it in an object
// (api/projects.ts:314-317, `path: { project_id: projectId }`).
import { createEffect, createSignal } from "solid-js";

async function api(route: string, init: { path: Record<string, string> }): Promise<{ status: number }> {
  return { status: route.length + Object.keys(init.path).length };
}

const readReporting = async (projectId: string) => {
  const response = await api("/projects/{project_id}/reporting", { path: { project_id: projectId } });
  return response.status === 200;
};

export const ProjectReporting = (properties: { projectId: string }) => {
  const [enabled, setEnabled] = createSignal(false);
  createEffect(
    () => properties.projectId,
    (projectId) => {
      void readReporting(projectId).then((ok) => setEnabled(ok));
    },
  );
  return <p>{enabled() ? "on" : "off"}</p>;
};
