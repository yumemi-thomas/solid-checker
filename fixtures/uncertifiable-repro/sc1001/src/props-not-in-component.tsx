// probus-hk AlertWatcher.tsx:38,83-84: a component that renders nothing (returns null, so no JSX in it),
// rendered by a tag from a loop, reads props in its body.
import { For } from "solid-js";

type Alert = { id: string; lead: number };

function alertContext(alert: Alert, lang: string) {
  return `${alert.id}:${lang}`;
}

function Runner(props: { alert: Alert; lang: string }) {
  const title = alertContext(props.alert, props.lang);
  console.log(title);
  return null;
}

export function Watcher(props: { alerts: Alert[]; lang: string }) {
  return <For each={props.alerts}>{(alert) => <Runner alert={alert} lang={props.lang} />}</For>;
}
