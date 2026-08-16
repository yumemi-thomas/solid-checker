const namespace = { existing: () => 1 };

Object.defineProperty(namespace, "getter", {
  enumerable: true,
  get() {
    return () => 2;
  }
});

namespace.late = () => 3;

export const runtimeNamespace = namespace;
export const stable = () => 4;
