// Output-only worker harness. It assigns sequence numbers and performs the
// bounded drain instructions Rust issued. Event meaning is deliberately left
// to the Rust evaluator.

export function createRuntimeProbeHarness(session) {
  const recorded = [];
  let microtasks = 0;
  let macrotasks = 0;
  return Object.freeze({
    emit(event) {
      if (!event || typeof event !== "object" || typeof event.marker !== "string") {
        throw new TypeError("runtime probe events require a marker");
      }
      recorded.push({ sequence: recorded.length, ...event });
    },
    async drain(controls = {}) {
      for (const step of session.drain) {
        if (step.kind === "flush") {
          if (typeof controls.flush !== "function") {
            throw new TypeError("runtime probe recipe did not provide the planned flush control");
          }
          await controls.flush();
        } else if (step.kind === "microtasks") {
          for (let turn = 0; turn < step.maxTurns; turn += 1) {
            await Promise.resolve();
            microtasks += 1;
          }
        } else if (step.kind === "macrotasks") {
          for (let turn = 0; turn < step.maxTurns; turn += 1) {
            await new Promise(resolve => setTimeout(resolve, 0));
            macrotasks += 1;
          }
        } else {
          throw new TypeError(`unknown runtime probe drain step ${step.kind}`);
        }
      }
    },
    events: () => structuredClone(recorded),
    drainedMicrotasks: () => microtasks,
    drainedMacrotasks: () => macrotasks
  });
}
