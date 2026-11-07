const view = document.defaultView;
if (view) view.getComputedStyle(document.body);
const { top } = document.body.getBoundingClientRect();
console.log(top);
type Node = { parent?: Node };
const node: Node = {};
console.log(node.parent);
export {};
