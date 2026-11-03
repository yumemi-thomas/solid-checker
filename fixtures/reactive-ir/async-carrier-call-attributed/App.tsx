import { For, createMemo } from "solid-js";

// ADR 0564: ADR 0462/0490's call-attributed walk climbs out of a named nested function no
// named-callback role places (a flow child's lexical role is no proof for it) and follows the inline
// carriers it is handed to by name, anchoring the row at the carrier; ADR 0563's predicate decides
// whether the carrier runs it.

// Violation (SC5001 at the carrier): a wrapper's read in a closure a non-empty carrier runs in a flow
// component's child, which renders untracked.
export function FlowForCarrier() {
  const user = createMemo(async () => 1);
  const wrapUse = () => {
    user();
  };
  return (
    <div>
      <For each={[1]}>
        {() => {
          const hf = () => {
            wrapUse();
          };
          [1].forEach(hf);
          return <p />;
        }}
      </For>
    </div>
  );
}

// Violation (SC5001 at the carrier): `.call` always runs its receiver.
export function FlowForCall() {
  const user = createMemo(async () => 1);
  const wrapUse = () => {
    user();
  };
  return (
    <div>
      <For each={[1]}>
        {() => {
          const hf = () => {
            wrapUse();
          };
          hf.call(null);
          return <p />;
        }}
      </For>
    </div>
  );
}

// Violation (SC5001 at the carrier): a tagged template calls its tag in the component body.
export function BodyCarrierTagged() {
  const user = createMemo(async () => 1);
  const wrapUse = () => {
    user();
  };
  const hf = (_s: TemplateStringsArray) => {
    wrapUse();
  };
  hf`x`;
  return <p />;
}

// Uncertifiable, never a violation: an empty receiver may run the closure zero times.
export function FlowCarrierEmpty() {
  const user = createMemo(async () => 1);
  const wrapUse = () => {
    user();
  };
  return (
    <div>
      <For each={[1]}>
        {() => {
          const hf = () => {
            wrapUse();
          };
          ([] as number[]).forEach(hf);
          return <p />;
        }}
      </For>
    </div>
  );
}

// No SC5003 row: a closure handed to a timer, or never called, is not run by the flow child's
// rendering.
export function FlowCarrierTimer() {
  const user = createMemo(async () => 1);
  const wrapUse = () => {
    user();
  };
  return (
    <div>
      <For each={[1]}>
        {() => {
          const hf = () => {
            wrapUse();
          };
          setTimeout(hf);
          return <p />;
        }}
      </For>
    </div>
  );
}

export function FlowNeverCalled() {
  const user = createMemo(async () => 1);
  const wrapUse = () => {
    user();
  };
  return (
    <div>
      <For each={[1]}>
        {() => {
          const hf = () => {
            wrapUse();
          };
          void hf;
          return <p />;
        }}
      </For>
    </div>
  );
}

// Silent: the carrier runs in a memo compute, which suspends on the pending read.
export function FlowCarrierInMemo() {
  const user = createMemo(async () => 1);
  const wrapUse = () => {
    user();
  };
  return (
    <div>
      <For each={[1]}>
        {() => {
          const hf = () => {
            wrapUse();
          };
          const m = createMemo(() => {
            [1].forEach(hf);
            return 1;
          });
          return <p>{m()}</p>;
        }}
      </For>
    </div>
  );
}

// No violation: a project method named `forEach` and a local `Reflect` are not carriers.
export function FlowShadowedForEach() {
  const user = createMemo(async () => 1);
  const wrapUse = () => {
    user();
  };
  const box = {
    forEach(f: () => void) {
      setTimeout(f);
    },
  };
  return (
    <div>
      <For each={[1]}>
        {() => {
          const hf = () => {
            wrapUse();
          };
          box.forEach(hf);
          return <p />;
        }}
      </For>
    </div>
  );
}

export function FlowLocalReflect() {
  const user = createMemo(async () => 1);
  const wrapUse = () => {
    user();
  };
  const Reflect = { apply(_f: () => void, _t: null, _a: []) {} };
  return (
    <div>
      <For each={[1]}>
        {() => {
          const hf = () => {
            wrapUse();
          };
          Reflect.apply(hf, null, []);
          return <p />;
        }}
      </For>
    </div>
  );
}

// Uncertifiable, no SC5003: a function written as a member's value runs wherever the member is called.
export function FlowMemberAssigned() {
  const user = createMemo(async () => 1);
  const wrapUse = () => {
    user();
  };
  return (
    <div>
      <For each={[1]}>
        {() => {
          class Holder {
            m: () => void;
            constructor() {
              this.m = () => {
                wrapUse();
              };
            }
          }
          new Holder().m();
          return <p />;
        }}
      </For>
    </div>
  );
}
