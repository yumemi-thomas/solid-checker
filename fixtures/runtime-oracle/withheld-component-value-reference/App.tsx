import { action, createSignal, For, type Component } from "solid-js";
import { createRouter, defineRoutes, memoryHistory } from "@solidjs/router";
import { Dynamic } from "@solidjs/web";

// Components that `createComponent` renders and that are also referenced as a
// value nobody calls plainly. ADR 0331 withholds their body findings (the
// reference may be a plain call); each one still runs as a component here, so
// the dev build warns. Shapes from the corpus adjudication (reviewed-findings).

// compass-ui Bridge.tsx:357-361, :559 and TopicView.tsx: an exact `Component`
// value in a route table; the router renders it by `createComponent` (next.41
// `dist/index.js:1823`).
const Board: Component = () => {
  const [tab] = createSignal("issues");
  const landing = tab();
  return <p>{landing}</p>;
};
const appRoutes = defineRoutes([{ path: "/", component: Board }]);
const Router = createRouter({ routes: appRoutes, history: memoryHistory("/") });
export function RouteTable() {
  return <Router>{(props) => <main>{props.children}</main>}</Router>;
}

// Control: `Dynamic` runs its component under `untrack` with no strict-read
// label (rc.17 `solid-js` `dist/solid.dev.js:1520`), so this body read is quiet.
const Tile: Component = () => {
  const [shade] = createSignal(1);
  const first = shade();
  return <p>{first}</p>;
};
const tiles = { tile: Tile };
export function DynamicControl() {
  return <Dynamic component={tiles.tile} />;
}

// ai-memory-ui icons.tsx:53: an icon in a nav map, called plainly in tracked
// JSX (no warning) and also rendered by a tag (the warning).
function Archive(props: { size?: number }) {
  const [stroke] = createSignal(2);
  console.log(stroke());
  return <svg width={props.size} />;
}
const nav = [{ icon: Archive, label: "backups" }];
export function IconMap() {
  return (
    <nav>
      <For each={nav}>{(item) => <span>{item.icon({ size: 14 })}</span>}</For>
      <button type="button"><Archive size={14} /></button>
    </nav>
  );
}

// jar-hell-web BadgesDialog.tsx:29: a static member on the component.
function Badges() {
  const [opened] = createSignal(false);
  console.log(opened());
  return <dialog id={Badges.id} />;
}
Badges.id = "badges-dialog";
export function StaticMember() {
  return <Badges />;
}

// kui SubjectList.tsx:78, GroupList.tsx:117: a Storybook meta holds the
// component; the route renders it with a reactive prop.
function SubjectList(props: { search: string }) {
  console.log(props.search);
  return <ul />;
}
const meta = { component: SubjectList, title: "SubjectList" };

// kui Drawer.tsx:74: a helper the body calls reads a signal; the component is
// exported (the package index re-exports it), held by its Storybook meta and
// rendered by a tag.
let sequence = 0;
function nextId(prefix: string): string {
  const [revision] = createSignal(0);
  console.log(revision());
  sequence += 1;
  return `${prefix}-${sequence}`;
}
export function Drawer(props: { title?: string }) {
  const titleId = nextId("kui-sheet-title");
  return <aside aria-labelledby={titleId}>{props.title}</aside>;
}
const drawerMeta = { title: "Surfaces/Drawer", component: Drawer };
export function StoryMeta() {
  const [typed] = createSignal("orders");
  return (
    <section title={meta.title} data-story={drawerMeta.title}>
      <SubjectList search={typed()} />
      <Drawer title="Produce" />
    </section>
  );
}

// every-deck-of-cards ExplorerPage.tsx:28, kui ConnectorList.tsx:64: a test
// route table holds the page; the app renders it by tag. Its own root: the dev
// build throws after reporting.
const save = action(function* () {});
function ExplorerPage() {
  void save();
  return <main>explorer</main>;
}
const testRoutes = [{ path: "/", component: ExplorerPage }];
export function ActionPage() {
  return <div data-routes={testRoutes.length}><ExplorerPage /></div>;
}
