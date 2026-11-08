import { Errored, createTrackedEffect, flush } from "solid-js";

// ADR 0908: an open program (no private manifest). rc.21's `flush()` inside a tracked effect throws a plain Error, no diagnostic.

const run = () => {
  flush();
  return 1;
};

// Uncertifiable (`computation-error-unobserved`): a component written inside an exported one is no export; its one mount is under a
// boundary whose fallback takes the error, so the tracked effect's throw is the program's own handling (rc.21 Chrome logs nothing).
export function NestedHandled() {
  const Kid = () => {
    createTrackedEffect(() => {
      run();
    });
    return <i></i>;
  };
  return (
    <div>
      <Errored fallback={(e) => <i>{String(e())}</i>}>
        <Kid />
      </Errored>
    </div>
  );
}

// Violation: the same nested component mounted under no boundary; the throw escalates and halts.
export function NestedUnhandled() {
  const Kid = () => {
    createTrackedEffect(() => {
      run();
    });
    return <i></i>;
  };
  return (
    <div>
      <Kid />
    </div>
  );
}

// Violation (kept): a component exported itself can be mounted outside the analyzed code, under no boundary.
export function ExportedKid() {
  createTrackedEffect(() => {
    run();
  });
  return <i></i>;
}

export function MountsExported() {
  return (
    <Errored fallback={(e) => <i>{String(e())}</i>}>
      <ExportedKid />
    </Errored>
  );
}
