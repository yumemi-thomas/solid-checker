export const automaticCompositionCases = [
  { name: "connectivity", version: "1.0.0-next.2", targets: ["makeConnectivityListener", "makeNetworkInformation", "createConnectivitySignal"],
    body: 'makeConnectivityListener(() => {})(); makeNetworkInformation(() => {})(); createConnectivitySignal()();' },
  { name: "media", version: "4.0.0-next.2", targets: ["makeMediaQueryListener"], body: 'makeMediaQueryListener("(min-width: 1px)", () => {})();' },
  { name: "mouse", version: "4.0.0-next.3", targets: ["makeMousePositionListener", "makeMouseInsideListener"],
    body: 'makeMousePositionListener(element, () => {})(); makeMouseInsideListener(element, () => {})();' },
  { name: "orientation", version: "1.0.0-next.2", targets: ["makeOrientation"], body: 'makeOrientation(() => {})();' },
  { name: "page-utilities", version: "3.0.0-next.2", targets: ["createPageVisibility", "createPageLeaveBlocker"],
    body: 'createPageVisibility()(); createPageLeaveBlocker();', callbackBody: 'createPageVisibility()(); createPageLeaveBlocker(() => read() > 0);' },
  { name: "interaction", version: "1.0.0-next.4", targets: ["makeInteractOutside"], body: 'makeInteractOutside(element, { onInteractOutside: () => {} })();' },
  { name: "drag-drop", version: "0.1.0-next.0", targets: [], body: null }
];

// Import under ordinary Node globals. In particular, utils' typeof window
// environment test must run normally. The guard falsifies call-time behavior;
// it supplies no evidence about module initialization or unsampled arguments.
export function callTimeProbeSource(spec, module) {
  return `const {${spec.targets.join(",")}} = await import(${JSON.stringify(module)});
for (const key of ["window","document","navigator","screen"]) Object.defineProperty(globalThis,key,{configurable:true,get(){throw new Error("DOM accessed: "+key)}});
let calls=0; const element = new Proxy({}, {get(){throw new Error("element read");}});
${spec.body.replaceAll("() => {}", '() => { calls++; throw new Error("callback executed"); }')}
console.log(JSON.stringify({calls}));`;
}
