// jandibat-web OwnerGate.tsx:200 (auth/owner-viewer.ts:36, relay-query.ts:24): an
// effect's apply starts a request, passing an AbortSignal through a helper to
// another helper that calls addEventListener on it.
import { createEffect } from "solid-js";

function readQuery(name: string, signal?: AbortSignal): Promise<string> {
  return new Promise((resolve, reject) => {
    const abort = () => reject(new DOMException("Aborted", "AbortError"));
    signal?.addEventListener("abort", abort, { once: true });
    resolve(name);
  });
}

async function readViewer(signal?: AbortSignal): Promise<string> {
  const response = await readQuery("viewer", signal);
  return response;
}

export function Gate(props: { epoch: number }) {
  const load = async () => {
    const controller = new AbortController();
    console.log(await readViewer(controller.signal));
  };
  createEffect(
    () => props.epoch,
    () => {
      void load();
    },
  );
  return <p>{props.epoch}</p>;
}
