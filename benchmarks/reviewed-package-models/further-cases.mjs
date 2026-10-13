const pagination = `import { createPagination } from "@solid-primitives/pagination";`;
const timer = `import { createTimer, createTimeoutLoop } from "@solid-primitives/timer";`;
export default [
  { id: "further-pagination-second-member", package: "@solid-primitives/pagination", export: "createPagination", rule: "strict-read-untracked",
    misuse: `${pagination} export default function App() { const [, page] = createPagination({ pages: 10 }); const value = page(); return <p>{value}</p>; }`,
    correct: `${pagination} export default function App() { const [, page] = createPagination({ pages: 10 }); return <p>{page()}</p>; }` },
  { id: "further-pagination-two-members", package: "@solid-primitives/pagination", export: "createPagination", rule: "strict-read-untracked",
    misuse: `${pagination} export default function App() { const [props, page, setPage] = createPagination({ pages: 10 }); const value = page(); return <button onClick={() => setPage(2)}>{value}:{props().length}</button>; }`,
    correct: `${pagination} export default function App() { const [props, page, setPage] = createPagination({ pages: 10 }); return <button onClick={() => setPage(2)}>{page()}:{props().length}</button>; }` },
  { id: "further-pagination-wrapper", package: "@solid-primitives/pagination", export: "createPagination", rule: "strict-read-untracked",
    misuse: `${pagination} function helper() { return createPagination({ pages: 10 }); }
export default function App() { const [, page] = helper(); const value = page(); return <p>{value}</p>; }`,
    correct: `${pagination} function helper() { return createPagination({ pages: 10 }); }
export default function App() { const [, page] = helper(); return <p>{page()}</p>; }` },
  { id: "further-pagination-default-binding", package: "@solid-primitives/pagination", export: "createPagination", rule: "strict-read-untracked", expectWarning: false, expectUnsupported: true,
    misuse: `${pagination} export default function App() { const [, page = () => 0] = createPagination({ pages: 10 }); const value = page(); return <p>{value}</p>; }` },
  { id: "further-timer-function-delay", package: "@solid-primitives/timer", export: "createTimer", rule: "missing-owner",
    misuse: `${timer} createTimer(() => {}, () => 1000, setInterval); export default function App() { return <p>timer</p>; }`,
    correct: `${timer} export default function App() { createTimer(() => {}, () => 1000, setInterval); return <p>timer</p>; }` },
  { id: "further-timer-unknown-delay", package: "@solid-primitives/timer", export: "createTimer", rule: "missing-owner", expectWarning: false, expectUnsupported: true,
    misuse: `${timer} const delay = Math.random() < 0.5 ? 1000 : () => 1000; createTimer(() => {}, delay, setInterval); export default function App() { return <p>timer</p>; }` },
  { id: "further-timeout-function-delay", package: "@solid-primitives/timer", export: "createTimeoutLoop", rule: "missing-owner",
    misuse: `${timer} createTimeoutLoop(() => {}, () => 1000); export default function App() { return <p>timer</p>; }`,
    correct: `${timer} export default function App() { createTimeoutLoop(() => {}, () => 1000); return <p>timer</p>; }` },
  { id: "further-media-two-wrappers", package: "@solid-primitives/media", export: "createMediaQuery", rule: "strict-read-untracked",
    misuse: `import { createMediaQuery } from "@solid-primitives/media"; function inner() { return createMediaQuery("x"); } function outer() { return inner(); }
export default function App() { const read = outer(); const value = read(); return <p>{String(value)}</p>; }`,
    correct: `import { createMediaQuery } from "@solid-primitives/media"; function inner() { return createMediaQuery("x"); } function outer() { return inner(); }
export default function App() { const read = outer(); return <p>{String(read())}</p>; }` },
];
