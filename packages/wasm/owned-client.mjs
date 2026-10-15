// The owned checker's main-thread side (P5 S2): one worker, one check in
// flight, a FIFO of accepted requests, and settlement of every outstanding
// promise whenever the worker can no longer answer.
//
// `attach` is internal: `createOwnedChecker` is the public way in, and it
// only ever attaches the shipped worker.

export class OwnedCheckerError extends Error {
  constructor(message) {
    super(message);
    this.name = "OwnedCheckerError";
  }
}

/** A request's JSON text, fixed when the check is accepted. */
function freezeRequest(request) {
  if (request === null || typeof request !== "object" || Array.isArray(request)) {
    throw new TypeError("an owned check request must be a JSON object");
  }
  const text = JSON.stringify(request);
  if (typeof text !== "string") throw new TypeError("an owned check request must serialize to JSON");
  return text;
}

function answer(status, output, transport) {
  const value = JSON.parse(output);
  if (status === 0 && value && typeof value === "object" && "snapshot" in value && Array.isArray(value.notes)) {
    return { ok: true, snapshot: value.snapshot, notes: value.notes, json: output, transport };
  }
  if (status === 1 && value && typeof value.error === "string") {
    return { ok: false, error: value.error, json: output, transport };
  }
  throw new Error(`status ${status} with an unexpected answer`);
}

/**
 * Starts `worker` with the module bytes and validated mount, and resolves to
 * the checker once it is ready. `transfer` lists the buffers moved to it.
 */
export function attach(worker, { checker, producer, mount, transfer }) {
  return new Promise((resolveReady, rejectReady) => {
    let state = "initializing";
    let failure;
    let closing;
    let current;
    let nextId = 1;
    const queue = [];
    const settle = error => {
      if (current) current.reject(error);
      current = undefined;
      for (const pending of queue.splice(0)) pending.reject(error);
    };
    const poison = error => {
      if (state === "poisoned" || state === "closed") return;
      const starting = state === "initializing";
      state = "poisoned";
      failure = error;
      settle(error);
      worker.terminate().catch(() => {});
      if (starting) rejectReady(error);
    };
    const pump = () => {
      if (state !== "ready" || current) return;
      if (queue.length === 0) {
        worker.unref?.();
        return;
      }
      current = queue.shift();
      worker.ref?.();
      worker.postMessage({ type: "check", id: current.id, requestJson: current.requestJson });
    };
    const api = Object.freeze({
      check(request) {
        if (state === "closed") return Promise.reject(new OwnedCheckerError("the owned checker is closed"));
        if (state === "poisoned") return Promise.reject(failure);
        let requestJson;
        try {
          requestJson = freezeRequest(request);
        } catch (error) {
          return Promise.reject(error);
        }
        return new Promise((resolve, reject) => {
          queue.push({ id: nextId++, requestJson, resolve, reject });
          worker.ref?.();
          pump();
        });
      },
      close() {
        if (!closing) {
          const wasPoisoned = state === "poisoned";
          state = "closed";
          if (!wasPoisoned) settle(new OwnedCheckerError("the owned checker is closed"));
          closing = worker.terminate().then(() => undefined, () => undefined);
        }
        return closing;
      },
    });
    worker.on("message", message => {
      if (state === "initializing" && message?.type === "ready") {
        state = "ready";
        resolveReady(api);
        pump();
      } else if (state === "initializing" && message?.type === "init-failed") {
        poison(new OwnedCheckerError(`the owned checker failed to start: ${message.error}`));
      } else if (state === "ready" && message?.type === "result" && current && message.id === current.id) {
        let result;
        try {
          result = answer(message.status, message.output, message.transport);
        } catch (error) {
          // `current` is still the active check here, so the poison settles it
          // with the queued ones.
          poison(new OwnedCheckerError(`the owned checker answered malformed output: ${error.message}`));
          return;
        }
        const pending = current;
        current = undefined;
        pending.resolve(result);
        pump();
      } else if (message?.type === "poison") {
        poison(new OwnedCheckerError(`the owned checker is poisoned: ${message.reason}`));
      } else {
        poison(new OwnedCheckerError(`the owned checker sent an unexpected ${JSON.stringify(message?.type)} message`));
      }
    });
    worker.on("error", error => poison(new OwnedCheckerError(`the owned checker worker failed: ${error?.message ?? error}`)));
    worker.on("messageerror", error => poison(new OwnedCheckerError(`the owned checker sent an undecodable message: ${error?.message ?? error}`)));
    worker.on("exit", code => poison(new OwnedCheckerError(`the owned checker worker exited with code ${code}`)));
    try {
      worker.postMessage({ type: "init", checker, producer, mount }, transfer);
    } catch (error) {
      poison(new OwnedCheckerError(`the owned checker could not be started: ${error.message}`));
    }
  });
}
