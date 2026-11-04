import { createSignal } from "solid-js";

const api = {
  async save(name: string): Promise<void> {
    if (name === "") throw new Error("name required");
  },
};
declare function ready(): boolean;

// Clean: the handler is the function's only entry. `catch` and `finally` run either on the handler's stack (the try head threw
// before it suspended) or past the await with no owner: legal on both paths.
export function Submit() {
  const [name, setName] = createSignal("");
  const [error, setError] = createSignal("");
  async function submit() {
    try {
      await api.save(name().trim());
    } catch {
      setError("failed");
    } finally {
      setName("");
    }
  }
  return <button onClick={() => void submit()}>{error()}</button>;
}

// Clean: an await in a branch; the write runs on the handler's stack or past the await.
export function Branch() {
  const [count, setCount] = createSignal(0);
  async function bump() {
    if (ready()) await api.save("x");
    setCount(1);
  }
  return <button onClick={() => void bump()}>{count()}</button>;
}

// Uncertifiable, not clean: the component body calls the function, so the path that does not suspend writes under the
// component's owner (Solid throws) while the other is legal. Neither is proven.
export function BodyCatch() {
  const [error, setError] = createSignal("");
  async function submit() {
    try {
      await api.save(error().trim());
    } catch {
      setError("failed");
    }
  }
  void submit();
  return <p>{error()}</p>;
}

// Violation (control): the write runs before any await, on the body's stack.
export function BodyBefore() {
  const [count, setCount] = createSignal(0);
  async function bump() {
    setCount(1);
    await api.save("x");
  }
  void bump();
  return <p>{count()}</p>;
}
