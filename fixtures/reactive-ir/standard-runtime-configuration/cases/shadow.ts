const DEV = { hooks: { onGraph() {} } };
const OBSERVE = { exclude() {} };
function enableExternalSource() {}
DEV.hooks.onGraph = () => {};
OBSERVE.exclude(); enableExternalSource();
export {};
