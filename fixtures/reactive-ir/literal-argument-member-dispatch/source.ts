// The parameter is structural, so which `replace` runs is chosen by each call
// site's argument, not by this declaration.
export interface Replaceable {
  replace(pattern: RegExp, value: string): string;
}

export function translate(key: Replaceable): string {
  return key.replace(/\{name\}/g, "you");
}

export interface ReplaceableWith {
  replace(pattern: RegExp, replacer: () => string): string;
}

// The replacer is this function's own callback: it runs synchronously inside
// `String.prototype.replace`, during the call.
export function fill(key: ReplaceableWith, read: () => string): string {
  return key.replace(/\{name\}/g, () => read());
}

export const label: Replaceable = {
  replace: (_pattern, value) => value,
};

// The member call resolves to `String.toUpperCase`: whatever string arrives,
// that built-in runs.
export function shout(message: string): string {
  return message.toUpperCase();
}

// `Date.getTime` is an object type's method; a subclass can override it.
export function stamp(when: Date): number {
  return when.getTime();
}

// An argumentless built-in call inside a chain: `trim` has no arguments, so
// its resolved declaration has to be demanded for the parameter-rooted call.
export function words(message: string): string[] {
  return message.trim().split(" ");
}
