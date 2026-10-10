import { createTrackedEffect } from "solid-js";
import { startLiteral, startProperty, startLength, startMixed } from "reactive-package";

// Positive: exact arguments prove a mandatory cleanup at module scope.
startLiteral(true);
// A proven guarded registration beside an optional unguarded one: the call
// still registers for certain (ADR 0252).
startMixed({ resize: () => {} });
// Only the optional registration remains: possible, not proven.
startMixed({});
startProperty({ resize: () => {} });
startLength(["Control", "K"]);

// Negative: each guard is false; no registration is claimed.
startLiteral(false);
startProperty({});
startLength([]);

// Nonliteral arguments remain possible even with narrow declared types.
declare const enabled: boolean;
declare const handlers: { resize?: () => void };
declare const keys: string[];
startLiteral(enabled);
startProperty(handlers);
startLength(keys);

// A spread never establishes an exact argument slot, even a tuple spread.
declare const literalArgs: [boolean];
declare const propertyArgs: [{ resize?: () => void }];
declare const lengthArgs: [string[]];
startLiteral(...literalArgs);
startProperty(...propertyArgs);
startLength(...lengthArgs);

// Both owner consumers use the same guard decision in a leaf owner.
export function Leaf() {
  createTrackedEffect(() => {
    startLiteral(true);
    startProperty({ resize: () => {} });
    startLength(["Control", "K"]);
    startLiteral(false);
    startProperty({});
    startLength([]);
    startLiteral(enabled);
    startProperty(handlers);
    startLength(keys);
    startLiteral(...literalArgs);
    startProperty(...propertyArgs);
    startLength(...lengthArgs);
  });
  return <div />;
}
