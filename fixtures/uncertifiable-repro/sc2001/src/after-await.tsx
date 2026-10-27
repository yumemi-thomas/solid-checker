// donegeon-client OnboardingRoute.tsx:80: a submit handler writes after the
// awaited request settles, in `catch` and `finally`.
import { createSignal } from "solid-js";

const api = {
  async completeOnboarding(name: string): Promise<void> {
    if (name === "") throw new Error("name required");
  },
};

export function Onboarding() {
  const [name, setName] = createSignal("");
  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal("");

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    setSaving(true);
    try {
      await api.completeOnboarding(name().trim());
    } catch (err) {
      setError(err instanceof Error ? err.message : "Onboarding failed");
    } finally {
      setSaving(false);
    }
  }

  return (
    <form onSubmit={(event) => void submit(event)}>
      <input value={name()} onInput={(event) => setName(event.currentTarget.value)} />
      <button disabled={saving()}>Save</button>
      <p>{error()}</p>
    </form>
  );
}
