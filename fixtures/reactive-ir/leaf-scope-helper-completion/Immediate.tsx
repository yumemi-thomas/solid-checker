import { createTrackedEffect, onCleanup } from "solid-js";

declare const flag: boolean;
declare const unknown: () => void;

export function CleanImmediate() {
  createTrackedEffect(() => {
    (() => { const value = 1; void value; })();
    (function () { const value = 2; void value; })();
    (function* () { onCleanup(() => {}); })();
    ((() => {}) as () => void)();
    void (async () => {
      await Promise.resolve();
      unknown();
    })();
  });
  return <div />;
}

export function ForbiddenImmediate() {
  createTrackedEffect(() => {
    (() => { onCleanup(() => {}); })();
  });
  return <div />;
}

export function AsyncPrefix() {
  createTrackedEffect(() => {
    void (async () => {
      onCleanup(() => {});
      await Promise.resolve();
    })();
  });
  return <div />;
}

export function ConditionalImmediate() {
  createTrackedEffect(() => {
    ((enabled: boolean) => { if (enabled) onCleanup(() => {}); })(flag);
  });
  return <div />;
}

export function KnownArgument() {
  createTrackedEffect(() => {
    ((enabled: boolean) => { if (enabled) onCleanup(() => {}); })(false);
  });
  return <div />;
}

export function KnownTrueArgument() {
  createTrackedEffect(() => {
    ((enabled: boolean) => { if (enabled) onCleanup(() => {}); })(true);
  });
  return <div />;
}

export function SpreadArgument() {
  createTrackedEffect(() => {
    ((enabled: boolean) => { if (enabled) onCleanup(() => {}); })(...([true] as const));
  });
  return <div />;
}

function registerWhenEnabled(enabled: boolean) {
  if (enabled) onCleanup(() => {});
}

function makeRegistration() {
  return registerWhenEnabled;
}

export function StableTransportFalse() {
  const register = makeRegistration();
  createTrackedEffect(() => {
    register(false);
  });
  return <div />;
}

export function StableTransportTrue() {
  const register = makeRegistration();
  createTrackedEffect(() => {
    register(true);
  });
  return <div />;
}

export function StableTransportSpread() {
  const register = makeRegistration();
  createTrackedEffect(() => {
    register(...([true] as const));
  });
  return <div />;
}

function makeMemberRegistration() {
  return {
    register(enabled: boolean) {
      if (enabled) onCleanup(() => {});
    },
  };
}

export function StableMemberReceiver() {
  const target = makeMemberRegistration();
  createTrackedEffect(() => {
    target.register(false);
  });
  return <div />;
}

export function UnknownImmediate() {
  createTrackedEffect(() => {
    (() => { unknown(); })();
  });
  return <div />;
}

export function DefaultImmediate() {
  createTrackedEffect(() => {
    ((_value = unknown()) => {})();
  });
  return <div />;
}

export function ConditionalSuspension() {
  createTrackedEffect(() => {
    void (async () => {
      if (flag) await Promise.resolve();
      onCleanup(() => {});
    })();
  });
  return <div />;
}

export function OpaqueSelection() {
  createTrackedEffect(() => {
    (flag ? () => {} : unknown)();
  });
  return <div />;
}

export function WrappedImmediate() {
  createTrackedEffect(() => {
    ((() => { onCleanup(() => {}); }) as () => void)();
  });
  return <div />;
}
