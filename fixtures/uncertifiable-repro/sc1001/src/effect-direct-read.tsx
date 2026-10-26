// error-menu-web ProjectPage.tsx:57,96: an effect's apply calls a helper that reads a signal after an await.
import { createEffect, createSignal } from "solid-js";

async function listJobs(projectId: string): Promise<string[]> {
  const response = await fetch(`/jobs/${projectId}`);
  return response.json() as Promise<string[]>;
}

export function ProjectPage(props: { projectId: string }) {
  const [known, setKnown] = createSignal(0);
  const loadJobs = async (projectId: string) => {
    const jobs = await listJobs(projectId);
    if (jobs.length > known()) console.log("new jobs");
  };
  createEffect(
    () => props.projectId,
    (projectId) => {
      void loadJobs(projectId);
    },
  );
  return <button onClick={() => setKnown(known() + 1)}>{known()}</button>;
}
