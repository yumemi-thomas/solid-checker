// A tag elsewhere renders the exported component (kui shell/src/app.tsx renders SettingsPage).
import { Panel } from "./exported-component";

export function Page() {
  return <Panel session={{ brush: () => "round" }} />;
}
