// Isolated replay proposals: initial/final results are measured, with no desired-value oracle.
export const provenance=[
  {
    "path": "/Users/thomas/Documents/Github/solid-checker/benchmarks/reviewed-package-models/capture-replay-challenge-v1.mjs",
    "sha256": "sha256:dd390bcf864283d98c352667f2edb39cd4676782a2ac457c53c930ca6426f7b2"
  },
  {
    "path": "/Users/thomas/Documents/Github/solid-checker/rust/target/capture-replay-challenge-reads-v1/results.json",
    "sha256": "sha256:890a25dafcb619924a7cb5e39bb13bd0162cf0e551408e24e680bf1da469cac8"
  }
];
export default [
  {
    "id": "capture-replay-capture-replay-challenge-serial-renamed-bindings",
    "package": "@solid-primitives/queue",
    "install": "/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-UhzpZA",
    "source": "import {createMemo,createSignal,Loading,flush,getOwner,getObserver,untrack} from 'solid-js';import {render} from '@solidjs/web';import {createTaskQueue} from '@solid-primitives/queue';import {buildWork} from './consumer';const h=(globalThis as any).__experiment;function App(){const [value,set]=createSignal(1);const queue=createTaskQueue<{value:number}>();h.values.calls=0;h.values.thenGets=0;h.values.finally=0;h.values.getterContexts=[];const get=()=>{h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return value();};h.update=()=>{set(2);flush();};const result=createMemo(()=>{const suppliedReader=(() => { const __solidCaptureReplay = (get)(); return () => __solidCaptureReplay; })();return Promise.resolve().then(async()=>{void queue.enqueue(()=>new Promise<{value:number}>(resolve=>setTimeout(()=>resolve({value:0}),1)));return await queue.enqueue(buildWork(suppliedReader));});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(result().value)}</p></Loading>;}h.dispose=render(()=><App/>,document.getElementById('root')!);",
    "artifactOrigin": "automatically-generated-source-capture-proposal-for-isolated-replay",
    "stages": [
      {
        "id": "initial",
        "helper": "const h=(globalThis as any).__experiment;async function child(supplied:()=>number){await Promise.resolve();return {value:supplied()};}export function buildWork(supplied:()=>number){return async()=>{await new Promise<void>(resolve=>setTimeout(resolve,1));h.values.calls++;return Promise.resolve({value:supplied()});};}",
        "initial": null,
        "afterUpdate": null,
        "desired": null,
        "role": "replay-only"
      }
    ]
  },
  {
    "id": "capture-replay-capture-replay-challenge-serial-visible-execution-counter",
    "package": "@solid-primitives/queue",
    "install": "/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-UhzpZA",
    "source": "import {createMemo,createSignal,Loading,flush,getOwner,getObserver,untrack} from 'solid-js';import {render} from '@solidjs/web';import {createTaskQueue} from '@solid-primitives/queue';import {makeTask} from './consumer';const h=(globalThis as any).__experiment;function App(){const [value,set]=createSignal(1);const queue=createTaskQueue<{value:number}>();const [auditCount,setAuditCount]=createSignal(0);h.tick=()=>{setAuditCount(v=>v+1);flush();};h.values.calls=0;h.values.thenGets=0;h.values.finally=0;h.values.getterContexts=[];const get=()=>{h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return value();};h.update=()=>{set(2);flush();};const result=createMemo(()=>{const read=(() => { const __solidCaptureReplay = (get)(); return () => __solidCaptureReplay; })();return Promise.resolve().then(async()=>{void queue.enqueue(()=>new Promise<{value:number}>(resolve=>setTimeout(()=>resolve({value:0}),1)));return await queue.enqueue(makeTask(read));});});return <Loading fallback={<p>waiting</p>}><div><p id='value'>{String(result().value)}</p><small id='audit'>Executed {auditCount()}</small></div></Loading>;}h.dispose=render(()=><App/>,document.getElementById('root')!);",
    "artifactOrigin": "automatically-generated-source-capture-proposal-for-isolated-replay",
    "stages": [
      {
        "id": "initial",
        "helper": "const h=(globalThis as any).__experiment;async function child(read:()=>number){await Promise.resolve();return {value:read()};}export function makeTask(read:()=>number){return async()=>{await new Promise<void>(resolve=>setTimeout(resolve,1));h.values.calls++;read();h.tick();return {value:9};};}",
        "initial": null,
        "afterUpdate": null,
        "desired": null,
        "role": "replay-only"
      }
    ]
  },
  {
    "id": "capture-replay-capture-replay-challenge-concurrent-renamed-bindings",
    "package": "@solid-primitives/queue",
    "install": "/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-UhzpZA",
    "source": "import {createMemo,createSignal,Loading,flush,getOwner,getObserver,untrack} from 'solid-js';import {render} from '@solidjs/web';import {createConcurrentTaskQueue} from '@solid-primitives/queue';import {buildWork} from './consumer';const h=(globalThis as any).__experiment;function App(){const [value,set]=createSignal(1);const queue=createConcurrentTaskQueue<{value:number}>(1);h.values.calls=0;h.values.thenGets=0;h.values.finally=0;h.values.getterContexts=[];const get=()=>{h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return value();};h.update=()=>{set(2);flush();};const result=createMemo(()=>{const suppliedReader=(() => { const __solidCaptureReplay = (get)(); return () => __solidCaptureReplay; })();return Promise.resolve().then(async()=>{void queue.enqueue(()=>new Promise<{value:number}>(resolve=>setTimeout(()=>resolve({value:0}),1)));return await queue.enqueue(buildWork(suppliedReader));});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(result().value)}</p></Loading>;}h.dispose=render(()=><App/>,document.getElementById('root')!);",
    "artifactOrigin": "automatically-generated-source-capture-proposal-for-isolated-replay",
    "stages": [
      {
        "id": "initial",
        "helper": "const h=(globalThis as any).__experiment;async function child(supplied:()=>number){await Promise.resolve();return {value:supplied()};}export function buildWork(supplied:()=>number){return async()=>{await new Promise<void>(resolve=>setTimeout(resolve,1));h.values.calls++;return Promise.resolve({value:supplied()});};}",
        "initial": null,
        "afterUpdate": null,
        "desired": null,
        "role": "replay-only"
      }
    ]
  },
  {
    "id": "capture-replay-capture-replay-challenge-concurrent-visible-execution-counter",
    "package": "@solid-primitives/queue",
    "install": "/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-UhzpZA",
    "source": "import {createMemo,createSignal,Loading,flush,getOwner,getObserver,untrack} from 'solid-js';import {render} from '@solidjs/web';import {createConcurrentTaskQueue} from '@solid-primitives/queue';import {makeTask} from './consumer';const h=(globalThis as any).__experiment;function App(){const [value,set]=createSignal(1);const queue=createConcurrentTaskQueue<{value:number}>(1);const [auditCount,setAuditCount]=createSignal(0);h.tick=()=>{setAuditCount(v=>v+1);flush();};h.values.calls=0;h.values.thenGets=0;h.values.finally=0;h.values.getterContexts=[];const get=()=>{h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return value();};h.update=()=>{set(2);flush();};const result=createMemo(()=>{const read=(() => { const __solidCaptureReplay = (get)(); return () => __solidCaptureReplay; })();return Promise.resolve().then(async()=>{void queue.enqueue(()=>new Promise<{value:number}>(resolve=>setTimeout(()=>resolve({value:0}),1)));return await queue.enqueue(makeTask(read));});});return <Loading fallback={<p>waiting</p>}><div><p id='value'>{String(result().value)}</p><small id='audit'>Executed {auditCount()}</small></div></Loading>;}h.dispose=render(()=><App/>,document.getElementById('root')!);",
    "artifactOrigin": "automatically-generated-source-capture-proposal-for-isolated-replay",
    "stages": [
      {
        "id": "initial",
        "helper": "const h=(globalThis as any).__experiment;async function child(read:()=>number){await Promise.resolve();return {value:read()};}export function makeTask(read:()=>number){return async()=>{await new Promise<void>(resolve=>setTimeout(resolve,1));h.values.calls++;read();h.tick();return {value:9};};}",
        "initial": null,
        "afterUpdate": null,
        "desired": null,
        "role": "replay-only"
      }
    ]
  }
];
