// inferay AppHeader/index.tsx:81: a command's `run` calls a project function whose
// parameter has a default (shared/lib/dom.tsx:349, `openSettingsModal(section = "agents")`).
type Section = "agents" | "mcp" | "appearance";

function dispatchWindowEvent<T>(name: string, detail: T): void {
  window.dispatchEvent(new CustomEvent<T>(name, { detail }));
}

export function openSettingsModal(section: Section = "agents"): void {
  dispatchWindowEvent("open-settings-modal", { section });
}

export function AppHeader() {
  const commands = [{ id: "settings", label: "Open settings", run: () => openSettingsModal() }];
  return (
    <ul>
      {commands.map((command) => (
        <li onClick={() => command.run()}>{command.label}</li>
      ))}
    </ul>
  );
}
