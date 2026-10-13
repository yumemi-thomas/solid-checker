import "./runtime-entry.ts";
import "./extension-conflict-entry.ts";
import "./extension-directory-entry.ts";
import { startClosed } from "reactive-package";
import { value, browserHelper } from "./shared";
import { remote } from "./server";
import { onServer } from "./server-import-only";
import { barrelOnly } from "./barrel";
console.log(value);
barrelOnly();
browserHelper();
remote(() => { startClosed(); /* RPC-only argument stays baseline */ });
function Router(props: {children: (value: {children: null}) => JSX.Element}) {
  return props.children({children: null});
}
function Shell(props: {content: () => JSX.Element}) { return props.content(); }
function consume(callback: () => void) { callback(); }
const lazy = (loader: () => Promise<unknown>) => loader();
consume(() => {
  startClosed(); /* argument callback */
  const uncalled = () => { startClosed(); /* nested uncalled */ };
  void uncalled;
});
function App() {
  return <Router>{(_props) => {
    startClosed(); /* Router child */
    return <Shell content={() => { startClosed(); /* JSX prop */ return <div />; }} />;
  }}</Router>;
}
App();
lazy(() => import("./loaded"));
const path: string = "./never-loaded";
void import(path);
export async function serverAction() {
  "use server";
  onServer();
  startClosed(); /* server function */
  consume(() => { startClosed(); /* callback in server function */ });
}
serverAction();
startClosed();
// Host-sensitive execution: markers are inside the region whose host is asked.
if (import.meta.env.SSR) { void "host SSR call"; startClosed(); }
if (!import.meta.env.SSR) { void "host client call"; startClosed(); }
import.meta.env.SSR ? (void "host ternary server", startClosed()) : (void "host ternary client", startClosed());
import.meta.env.SSR && (void "host and server", startClosed());
!import.meta.env.SSR && (void "host and client", startClosed());
import.meta.env.SSR || (void "host or client", startClosed());
if (import.meta.env.SSR) void import("./server-guarded");
if (import.meta.env.SSR) { void "host server JSX"; <div/>; }
if (import.meta.env.SSR) { void "host server member"; console.log; }
function hostEarlyGuard() {
  if (import.meta.env.SSR) return;
  void "host client tail"; startClosed();
}
function hostDead() { return; void "host dead tail"; startClosed(); }
function hostThrows() { throw new Error(); void "host thrown tail"; startClosed(); }
function hostDefaultSkipped(x: boolean = (void "host default skipped", startClosed(), true)) { return x; }
function hostDefaultOmitted(x: boolean = (void "host default omitted", startClosed(), true)) { return x; }
function hostDefaultUndefined(x: boolean = (void "host default void zero", startClosed(), true)) { return x; }
function hostDefaultUnknown(x: boolean = (void "host default bare undefined", startClosed(), true)) { return x; }
function hostDefaultPriorUnknown(x: boolean = (startClosed(), true), y: boolean = (void "host default prior unknown", startClosed(), true)) { return x && y; }
function hostDefaultPriorLiteral(x: boolean = true, y: boolean = (void "host default prior literal", startClosed(), true)) { return x && y; }
hostEarlyGuard(); hostDead();
hostDefaultSkipped(false); hostDefaultOmitted(); hostDefaultUndefined(void 0); hostDefaultUnknown(undefined);
hostDefaultPriorUnknown(); hostDefaultPriorLiteral();
// Keep the throw test last: it cannot prevent the positive twins above.
hostThrows();
