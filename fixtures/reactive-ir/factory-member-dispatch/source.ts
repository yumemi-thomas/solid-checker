// Factories, and helpers that invoke a member of their parameter.

export type Tokens = { waitForToken(): number; settle(next: number): void };

// Every return is a fresh object literal holding the method.
export function createTokens(): Tokens {
  let token = 0;
  return {
    waitForToken(): number {
      return token;
    },
    settle(next: number): void {
      token = next;
    },
  };
}

// A concise arrow body that is the literal itself.
export const createArrowTokens = (): Tokens => ({
  waitForToken() {
    return 1;
  },
  settle() {},
});

// A getter: the value is what it returns when read.
export function createGetterTokens(): Tokens {
  return {
    get waitForToken() {
      return () => 2;
    },
    settle() {},
  };
}

// One path returns something that is not a literal written in the return.
export function createSharedTokens(shared: boolean, existing: Tokens): Tokens {
  if (shared) return existing;
  return {
    waitForToken() {
      return 3;
    },
    settle() {},
  };
}

// The literal holds the method only one level down.
export function createNestedTokens(): { inner: Tokens } & Partial<Tokens> {
  return {
    inner: {
      waitForToken() {
        return 4;
      },
      settle() {},
    },
  };
}

// The call's argument holds a same-named property; the result does not.
export function wrapTokens(source: Tokens): Tokens {
  return source;
}

// A method of an object: its receiver selects it, not this rule.
export const registry = {
  create(): Tokens {
    return {
      waitForToken() {
        return 5;
      },
      settle() {},
    };
  },
};

export function wait(tokens: Tokens): number {
  return tokens.waitForToken();
}

export type Renamable = { rename(): number };

export function createRenamable(): Renamable {
  return {
    rename() {
      return 6;
    },
  };
}

export function rename(value: Renamable): number {
  return value.rename();
}

// The program assigns a `rename` member somewhere.
const patched = createRenamable();
patched.rename = () => 7;
