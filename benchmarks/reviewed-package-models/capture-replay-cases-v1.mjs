// Automatically generated isolated replay proposals; original input provenance is audited.
export const provenance=[
  {
    "path": "/Users/thomas/Documents/Github/solid-checker/benchmarks/reviewed-package-models/async-body-return-cases-v1.mjs",
    "sha256": "sha256:ef40abebed0619904153924fbedca415b3170a7c3acaad559ae963eb976076fc"
  },
  {
    "path": "/Users/thomas/Documents/Github/solid-checker/rust/target/async-body-return-fresh-reads-v1/results.json",
    "sha256": "sha256:4bd09bdd15adb1e8a0f812768cb066fcc6389eefdb9a1d47134ff937c3e61181"
  }
];
export default [
  {
    "id": "capture-replay-async-body-return-serial-promise-object-deferred",
    "package": "@solid-primitives/queue",
    "install": "/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-UhzpZA",
    "source": "import {createMemo,createSignal,Loading,flush,getOwner,getObserver,untrack} from 'solid-js';import {render} from '@solidjs/web';import {createTaskQueue} from '@solid-primitives/queue';import {makeTask} from './consumer';const h=(globalThis as any).__experiment;function App(){const [value,set]=createSignal(1);const queue=createTaskQueue<{value:number}>();h.values.calls=0;h.values.thenGets=0;h.values.finally=0;h.values.getterContexts=[];const get=()=>{h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return value();};h.update=()=>{set(2);flush();};const result=createMemo(()=>{const read=(() => { const __solidCaptureReplay = (get)(); return () => __solidCaptureReplay; })();return Promise.resolve().then(async()=>{void queue.enqueue(()=>new Promise<{value:number}>(resolve=>setTimeout(()=>resolve({value:0}),1)));return await queue.enqueue(makeTask(read));});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(result().value)}</p></Loading>;}h.dispose=render(()=><App/>,document.getElementById('root')!);",
    "artifactOrigin": "automatically-generated-source-capture-proposal-for-isolated-replay",
    "stages": [
      {
        "id": "initial",
        "helper": "const h=(globalThis as any).__experiment;async function child(read:()=>number){await Promise.resolve();return {value:read()};}export function makeTask(read:()=>number){return async()=>{await new Promise<void>(resolve=>setTimeout(resolve,1));h.values.calls++;return Promise.resolve({value:read()});};}",
        "initial": "1",
        "afterUpdate": "2",
        "desired": "2",
        "role": "control"
      }
    ]
  },
  {
    "id": "capture-replay-async-body-return-serial-thenable-read-before-return-deferred",
    "package": "@solid-primitives/queue",
    "install": "/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-UhzpZA",
    "source": "import {createMemo,createSignal,Loading,flush,getOwner,getObserver,untrack} from 'solid-js';import {render} from '@solidjs/web';import {createTaskQueue} from '@solid-primitives/queue';import {makeTask} from './consumer';const h=(globalThis as any).__experiment;function App(){const [value,set]=createSignal(1);const queue=createTaskQueue<{value:number}>();h.values.calls=0;h.values.thenGets=0;h.values.finally=0;h.values.getterContexts=[];const get=()=>{h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return value();};h.update=()=>{set(2);flush();};const result=createMemo(()=>{const read=(() => { const __solidCaptureReplay = (get)(); return () => __solidCaptureReplay; })();return Promise.resolve().then(async()=>{void queue.enqueue(()=>new Promise<{value:number}>(resolve=>setTimeout(()=>resolve({value:0}),1)));return await queue.enqueue(makeTask(read));});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(result().value)}</p></Loading>;}h.dispose=render(()=><App/>,document.getElementById('root')!);",
    "artifactOrigin": "automatically-generated-source-capture-proposal-for-isolated-replay",
    "stages": [
      {
        "id": "initial",
        "helper": "const h=(globalThis as any).__experiment;async function child(read:()=>number){await Promise.resolve();return {value:read()};}export function makeTask(read:()=>number){return async()=>{await new Promise<void>(resolve=>setTimeout(resolve,1));h.values.calls++;const value=read();return {then(resolve:(value:{value:number})=>void){resolve({value});}};};}",
        "initial": "1",
        "afterUpdate": "2",
        "desired": "2",
        "role": "control"
      }
    ]
  },
  {
    "id": "capture-replay-async-body-return-serial-function-object-deferred",
    "package": "@solid-primitives/queue",
    "install": "/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-UhzpZA",
    "source": "import {createMemo,createSignal,Loading,flush,getOwner,getObserver,untrack} from 'solid-js';import {render} from '@solidjs/web';import {createTaskQueue} from '@solid-primitives/queue';import {makeTask} from './consumer';const h=(globalThis as any).__experiment;function App(){const [value,set]=createSignal(1);const queue=createTaskQueue<{value:number}>();h.values.calls=0;h.values.thenGets=0;h.values.finally=0;h.values.getterContexts=[];const get=()=>{h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return value();};h.update=()=>{set(2);flush();};const result=createMemo(()=>{const read=(() => { const __solidCaptureReplay = (get)(); return () => __solidCaptureReplay; })();return Promise.resolve().then(async()=>{void queue.enqueue(()=>new Promise<{value:number}>(resolve=>setTimeout(()=>resolve({value:0}),1)));return await queue.enqueue(makeTask(read));});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(result().value)}</p></Loading>;}h.dispose=render(()=><App/>,document.getElementById('root')!);",
    "artifactOrigin": "automatically-generated-source-capture-proposal-for-isolated-replay",
    "stages": [
      {
        "id": "initial",
        "helper": "const h=(globalThis as any).__experiment;async function child(read:()=>number){await Promise.resolve();return {value:read()};}export function makeTask(read:()=>number){return async()=>{await new Promise<void>(resolve=>setTimeout(resolve,1));h.values.calls++;return Object.assign(()=>0,{value:read()});};}",
        "initial": "1",
        "afterUpdate": "2",
        "desired": "2",
        "role": "control"
      }
    ]
  },
  {
    "id": "capture-replay-async-body-return-serial-awaited-child-object-deferred",
    "package": "@solid-primitives/queue",
    "install": "/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-UhzpZA",
    "source": "import {createMemo,createSignal,Loading,flush,getOwner,getObserver,untrack} from 'solid-js';import {render} from '@solidjs/web';import {createTaskQueue} from '@solid-primitives/queue';import {makeTask} from './consumer';const h=(globalThis as any).__experiment;function App(){const [value,set]=createSignal(1);const queue=createTaskQueue<{value:number}>();h.values.calls=0;h.values.thenGets=0;h.values.finally=0;h.values.getterContexts=[];const get=()=>{h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return value();};h.update=()=>{set(2);flush();};const result=createMemo(()=>{const read=(() => { const __solidCaptureReplay = (get)(); return () => __solidCaptureReplay; })();return Promise.resolve().then(async()=>{void queue.enqueue(()=>new Promise<{value:number}>(resolve=>setTimeout(()=>resolve({value:0}),1)));return await queue.enqueue(makeTask(read));});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(result().value)}</p></Loading>;}h.dispose=render(()=><App/>,document.getElementById('root')!);",
    "artifactOrigin": "automatically-generated-source-capture-proposal-for-isolated-replay",
    "stages": [
      {
        "id": "initial",
        "helper": "const h=(globalThis as any).__experiment;async function child(read:()=>number){await Promise.resolve();return {value:read()};}export function makeTask(read:()=>number){return async()=>{await new Promise<void>(resolve=>setTimeout(resolve,1));h.values.calls++;return await child(read);};}",
        "initial": "1",
        "afterUpdate": "2",
        "desired": "2",
        "role": "control"
      }
    ]
  },
  {
    "id": "capture-replay-async-body-return-serial-constant-object-deferred",
    "package": "@solid-primitives/queue",
    "install": "/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-UhzpZA",
    "source": "import {createMemo,createSignal,Loading,flush,getOwner,getObserver,untrack} from 'solid-js';import {render} from '@solidjs/web';import {createTaskQueue} from '@solid-primitives/queue';import {makeTask} from './consumer';const h=(globalThis as any).__experiment;function App(){const [value,set]=createSignal(1);const queue=createTaskQueue<{value:number}>();h.values.calls=0;h.values.thenGets=0;h.values.finally=0;h.values.getterContexts=[];const get=()=>{h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return value();};h.update=()=>{set(2);flush();};const result=createMemo(()=>{const read=(() => { const __solidCaptureReplay = (get)(); return () => __solidCaptureReplay; })();return Promise.resolve().then(async()=>{void queue.enqueue(()=>new Promise<{value:number}>(resolve=>setTimeout(()=>resolve({value:0}),1)));return await queue.enqueue(makeTask(read));});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(result().value)}</p></Loading>;}h.dispose=render(()=><App/>,document.getElementById('root')!);",
    "artifactOrigin": "automatically-generated-source-capture-proposal-for-isolated-replay",
    "stages": [
      {
        "id": "initial",
        "helper": "const h=(globalThis as any).__experiment;async function child(read:()=>number){await Promise.resolve();return {value:read()};}export function makeTask(read:()=>number){return async()=>{await new Promise<void>(resolve=>setTimeout(resolve,1));h.values.calls++;read();return {value:9};};}",
        "initial": "9",
        "afterUpdate": "9",
        "desired": "9",
        "role": "control"
      }
    ]
  },
  {
    "id": "capture-replay-async-body-return-serial-caught-rejected-promise-deferred",
    "package": "@solid-primitives/queue",
    "install": "/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-UhzpZA",
    "source": "import {createMemo,createSignal,Loading,flush,getOwner,getObserver,untrack} from 'solid-js';import {render} from '@solidjs/web';import {createTaskQueue} from '@solid-primitives/queue';import {makeTask} from './consumer';const h=(globalThis as any).__experiment;function App(){const [value,set]=createSignal(1);const queue=createTaskQueue<{value:number}>();h.values.calls=0;h.values.thenGets=0;h.values.finally=0;h.values.getterContexts=[];const get=()=>{h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return value();};h.update=()=>{set(2);flush();};const result=createMemo(()=>{const read=(() => { const __solidCaptureReplay = (get)(); return () => __solidCaptureReplay; })();return Promise.resolve().then(async()=>{void queue.enqueue(()=>new Promise<{value:number}>(resolve=>setTimeout(()=>resolve({value:0}),1)));try{return await queue.enqueue(makeTask(read));}catch{return {value:9};}});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(result().value)}</p></Loading>;}h.dispose=render(()=><App/>,document.getElementById('root')!);",
    "artifactOrigin": "automatically-generated-source-capture-proposal-for-isolated-replay",
    "stages": [
      {
        "id": "initial",
        "helper": "const h=(globalThis as any).__experiment;async function child(read:()=>number){await Promise.resolve();return {value:read()};}export function makeTask(read:()=>number){return async()=>{await new Promise<void>(resolve=>setTimeout(resolve,1));h.values.calls++;read();return Promise.reject(new Error(\"expected adoption rejection\"));};}",
        "initial": "9",
        "afterUpdate": "9",
        "desired": "9",
        "role": "control"
      }
    ]
  },
  {
    "id": "capture-replay-async-body-return-serial-caught-throwing-then-getter-deferred",
    "package": "@solid-primitives/queue",
    "install": "/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-UhzpZA",
    "source": "import {createMemo,createSignal,Loading,flush,getOwner,getObserver,untrack} from 'solid-js';import {render} from '@solidjs/web';import {createTaskQueue} from '@solid-primitives/queue';import {makeTask} from './consumer';const h=(globalThis as any).__experiment;function App(){const [value,set]=createSignal(1);const queue=createTaskQueue<{value:number}>();h.values.calls=0;h.values.thenGets=0;h.values.finally=0;h.values.getterContexts=[];const get=()=>{h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return value();};h.update=()=>{set(2);flush();};const result=createMemo(()=>{const read=(() => { const __solidCaptureReplay = (get)(); return () => __solidCaptureReplay; })();return Promise.resolve().then(async()=>{void queue.enqueue(()=>new Promise<{value:number}>(resolve=>setTimeout(()=>resolve({value:0}),1)));try{return await queue.enqueue(makeTask(read));}catch{return {value:9};}});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(result().value)}</p></Loading>;}h.dispose=render(()=><App/>,document.getElementById('root')!);",
    "artifactOrigin": "automatically-generated-source-capture-proposal-for-isolated-replay",
    "stages": [
      {
        "id": "initial",
        "helper": "const h=(globalThis as any).__experiment;async function child(read:()=>number){await Promise.resolve();return {value:read()};}export function makeTask(read:()=>number){return async()=>{await new Promise<void>(resolve=>setTimeout(resolve,1));h.values.calls++;read();return Object.defineProperty({value:0},\"then\",{get(){h.values.thenGets++;throw new Error(\"expected getter rejection\");}});};}",
        "initial": "9",
        "afterUpdate": "9",
        "desired": "9",
        "role": "control"
      }
    ]
  },
  {
    "id": "capture-replay-async-body-return-serial-never-settling-promise-deferred",
    "package": "@solid-primitives/queue",
    "install": "/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-UhzpZA",
    "source": "import {createMemo,createSignal,Loading,flush,getOwner,getObserver,untrack} from 'solid-js';import {render} from '@solidjs/web';import {createTaskQueue} from '@solid-primitives/queue';import {makeTask} from './consumer';const h=(globalThis as any).__experiment;function App(){const [value,set]=createSignal(1);const queue=createTaskQueue<{value:number}>();h.values.calls=0;h.values.thenGets=0;h.values.finally=0;h.values.getterContexts=[];const get=()=>{h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return value();};h.update=()=>{set(2);flush();};const result=createMemo(()=>{const read=(() => { const __solidCaptureReplay = (get)(); return () => __solidCaptureReplay; })();return Promise.resolve().then(async()=>{void queue.enqueue(()=>new Promise<{value:number}>(resolve=>setTimeout(()=>resolve({value:0}),1)));return await queue.enqueue(makeTask(read));});});return <Loading fallback={<p id=\"value\">waiting</p>}><p id='value'>{String(result().value)}</p></Loading>;}h.dispose=render(()=><App/>,document.getElementById('root')!);",
    "artifactOrigin": "automatically-generated-source-capture-proposal-for-isolated-replay",
    "stages": [
      {
        "id": "initial",
        "helper": "const h=(globalThis as any).__experiment;async function child(read:()=>number){await Promise.resolve();return {value:read()};}export function makeTask(read:()=>number){return async()=>{await new Promise<void>(resolve=>setTimeout(resolve,1));h.values.calls++;read();return new Promise<{value:number}>(()=>{});};}",
        "initial": "waiting",
        "afterUpdate": "waiting",
        "desired": "waiting",
        "role": "control"
      }
    ]
  },
  {
    "id": "capture-replay-async-body-return-concurrent-promise-object-deferred",
    "package": "@solid-primitives/queue",
    "install": "/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-UhzpZA",
    "source": "import {createMemo,createSignal,Loading,flush,getOwner,getObserver,untrack} from 'solid-js';import {render} from '@solidjs/web';import {createConcurrentTaskQueue} from '@solid-primitives/queue';import {makeTask} from './consumer';const h=(globalThis as any).__experiment;function App(){const [value,set]=createSignal(1);const queue=createConcurrentTaskQueue<{value:number}>(1);h.values.calls=0;h.values.thenGets=0;h.values.finally=0;h.values.getterContexts=[];const get=()=>{h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return value();};h.update=()=>{set(2);flush();};const result=createMemo(()=>{const read=(() => { const __solidCaptureReplay = (get)(); return () => __solidCaptureReplay; })();return Promise.resolve().then(async()=>{void queue.enqueue(()=>new Promise<{value:number}>(resolve=>setTimeout(()=>resolve({value:0}),1)));return await queue.enqueue(makeTask(read));});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(result().value)}</p></Loading>;}h.dispose=render(()=><App/>,document.getElementById('root')!);",
    "artifactOrigin": "automatically-generated-source-capture-proposal-for-isolated-replay",
    "stages": [
      {
        "id": "initial",
        "helper": "const h=(globalThis as any).__experiment;async function child(read:()=>number){await Promise.resolve();return {value:read()};}export function makeTask(read:()=>number){return async()=>{await new Promise<void>(resolve=>setTimeout(resolve,1));h.values.calls++;return Promise.resolve({value:read()});};}",
        "initial": "1",
        "afterUpdate": "2",
        "desired": "2",
        "role": "control"
      }
    ]
  },
  {
    "id": "capture-replay-async-body-return-concurrent-thenable-read-before-return-deferred",
    "package": "@solid-primitives/queue",
    "install": "/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-UhzpZA",
    "source": "import {createMemo,createSignal,Loading,flush,getOwner,getObserver,untrack} from 'solid-js';import {render} from '@solidjs/web';import {createConcurrentTaskQueue} from '@solid-primitives/queue';import {makeTask} from './consumer';const h=(globalThis as any).__experiment;function App(){const [value,set]=createSignal(1);const queue=createConcurrentTaskQueue<{value:number}>(1);h.values.calls=0;h.values.thenGets=0;h.values.finally=0;h.values.getterContexts=[];const get=()=>{h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return value();};h.update=()=>{set(2);flush();};const result=createMemo(()=>{const read=(() => { const __solidCaptureReplay = (get)(); return () => __solidCaptureReplay; })();return Promise.resolve().then(async()=>{void queue.enqueue(()=>new Promise<{value:number}>(resolve=>setTimeout(()=>resolve({value:0}),1)));return await queue.enqueue(makeTask(read));});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(result().value)}</p></Loading>;}h.dispose=render(()=><App/>,document.getElementById('root')!);",
    "artifactOrigin": "automatically-generated-source-capture-proposal-for-isolated-replay",
    "stages": [
      {
        "id": "initial",
        "helper": "const h=(globalThis as any).__experiment;async function child(read:()=>number){await Promise.resolve();return {value:read()};}export function makeTask(read:()=>number){return async()=>{await new Promise<void>(resolve=>setTimeout(resolve,1));h.values.calls++;const value=read();return {then(resolve:(value:{value:number})=>void){resolve({value});}};};}",
        "initial": "1",
        "afterUpdate": "2",
        "desired": "2",
        "role": "control"
      }
    ]
  },
  {
    "id": "capture-replay-async-body-return-concurrent-function-object-deferred",
    "package": "@solid-primitives/queue",
    "install": "/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-UhzpZA",
    "source": "import {createMemo,createSignal,Loading,flush,getOwner,getObserver,untrack} from 'solid-js';import {render} from '@solidjs/web';import {createConcurrentTaskQueue} from '@solid-primitives/queue';import {makeTask} from './consumer';const h=(globalThis as any).__experiment;function App(){const [value,set]=createSignal(1);const queue=createConcurrentTaskQueue<{value:number}>(1);h.values.calls=0;h.values.thenGets=0;h.values.finally=0;h.values.getterContexts=[];const get=()=>{h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return value();};h.update=()=>{set(2);flush();};const result=createMemo(()=>{const read=(() => { const __solidCaptureReplay = (get)(); return () => __solidCaptureReplay; })();return Promise.resolve().then(async()=>{void queue.enqueue(()=>new Promise<{value:number}>(resolve=>setTimeout(()=>resolve({value:0}),1)));return await queue.enqueue(makeTask(read));});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(result().value)}</p></Loading>;}h.dispose=render(()=><App/>,document.getElementById('root')!);",
    "artifactOrigin": "automatically-generated-source-capture-proposal-for-isolated-replay",
    "stages": [
      {
        "id": "initial",
        "helper": "const h=(globalThis as any).__experiment;async function child(read:()=>number){await Promise.resolve();return {value:read()};}export function makeTask(read:()=>number){return async()=>{await new Promise<void>(resolve=>setTimeout(resolve,1));h.values.calls++;return Object.assign(()=>0,{value:read()});};}",
        "initial": "1",
        "afterUpdate": "2",
        "desired": "2",
        "role": "control"
      }
    ]
  },
  {
    "id": "capture-replay-async-body-return-concurrent-awaited-child-object-deferred",
    "package": "@solid-primitives/queue",
    "install": "/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-UhzpZA",
    "source": "import {createMemo,createSignal,Loading,flush,getOwner,getObserver,untrack} from 'solid-js';import {render} from '@solidjs/web';import {createConcurrentTaskQueue} from '@solid-primitives/queue';import {makeTask} from './consumer';const h=(globalThis as any).__experiment;function App(){const [value,set]=createSignal(1);const queue=createConcurrentTaskQueue<{value:number}>(1);h.values.calls=0;h.values.thenGets=0;h.values.finally=0;h.values.getterContexts=[];const get=()=>{h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return value();};h.update=()=>{set(2);flush();};const result=createMemo(()=>{const read=(() => { const __solidCaptureReplay = (get)(); return () => __solidCaptureReplay; })();return Promise.resolve().then(async()=>{void queue.enqueue(()=>new Promise<{value:number}>(resolve=>setTimeout(()=>resolve({value:0}),1)));return await queue.enqueue(makeTask(read));});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(result().value)}</p></Loading>;}h.dispose=render(()=><App/>,document.getElementById('root')!);",
    "artifactOrigin": "automatically-generated-source-capture-proposal-for-isolated-replay",
    "stages": [
      {
        "id": "initial",
        "helper": "const h=(globalThis as any).__experiment;async function child(read:()=>number){await Promise.resolve();return {value:read()};}export function makeTask(read:()=>number){return async()=>{await new Promise<void>(resolve=>setTimeout(resolve,1));h.values.calls++;return await child(read);};}",
        "initial": "1",
        "afterUpdate": "2",
        "desired": "2",
        "role": "control"
      }
    ]
  },
  {
    "id": "capture-replay-async-body-return-concurrent-constant-object-deferred",
    "package": "@solid-primitives/queue",
    "install": "/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-UhzpZA",
    "source": "import {createMemo,createSignal,Loading,flush,getOwner,getObserver,untrack} from 'solid-js';import {render} from '@solidjs/web';import {createConcurrentTaskQueue} from '@solid-primitives/queue';import {makeTask} from './consumer';const h=(globalThis as any).__experiment;function App(){const [value,set]=createSignal(1);const queue=createConcurrentTaskQueue<{value:number}>(1);h.values.calls=0;h.values.thenGets=0;h.values.finally=0;h.values.getterContexts=[];const get=()=>{h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return value();};h.update=()=>{set(2);flush();};const result=createMemo(()=>{const read=(() => { const __solidCaptureReplay = (get)(); return () => __solidCaptureReplay; })();return Promise.resolve().then(async()=>{void queue.enqueue(()=>new Promise<{value:number}>(resolve=>setTimeout(()=>resolve({value:0}),1)));return await queue.enqueue(makeTask(read));});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(result().value)}</p></Loading>;}h.dispose=render(()=><App/>,document.getElementById('root')!);",
    "artifactOrigin": "automatically-generated-source-capture-proposal-for-isolated-replay",
    "stages": [
      {
        "id": "initial",
        "helper": "const h=(globalThis as any).__experiment;async function child(read:()=>number){await Promise.resolve();return {value:read()};}export function makeTask(read:()=>number){return async()=>{await new Promise<void>(resolve=>setTimeout(resolve,1));h.values.calls++;read();return {value:9};};}",
        "initial": "9",
        "afterUpdate": "9",
        "desired": "9",
        "role": "control"
      }
    ]
  },
  {
    "id": "capture-replay-async-body-return-concurrent-caught-rejected-promise-deferred",
    "package": "@solid-primitives/queue",
    "install": "/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-UhzpZA",
    "source": "import {createMemo,createSignal,Loading,flush,getOwner,getObserver,untrack} from 'solid-js';import {render} from '@solidjs/web';import {createConcurrentTaskQueue} from '@solid-primitives/queue';import {makeTask} from './consumer';const h=(globalThis as any).__experiment;function App(){const [value,set]=createSignal(1);const queue=createConcurrentTaskQueue<{value:number}>(1);h.values.calls=0;h.values.thenGets=0;h.values.finally=0;h.values.getterContexts=[];const get=()=>{h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return value();};h.update=()=>{set(2);flush();};const result=createMemo(()=>{const read=(() => { const __solidCaptureReplay = (get)(); return () => __solidCaptureReplay; })();return Promise.resolve().then(async()=>{void queue.enqueue(()=>new Promise<{value:number}>(resolve=>setTimeout(()=>resolve({value:0}),1)));try{return await queue.enqueue(makeTask(read));}catch{return {value:9};}});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(result().value)}</p></Loading>;}h.dispose=render(()=><App/>,document.getElementById('root')!);",
    "artifactOrigin": "automatically-generated-source-capture-proposal-for-isolated-replay",
    "stages": [
      {
        "id": "initial",
        "helper": "const h=(globalThis as any).__experiment;async function child(read:()=>number){await Promise.resolve();return {value:read()};}export function makeTask(read:()=>number){return async()=>{await new Promise<void>(resolve=>setTimeout(resolve,1));h.values.calls++;read();return Promise.reject(new Error(\"expected adoption rejection\"));};}",
        "initial": "9",
        "afterUpdate": "9",
        "desired": "9",
        "role": "control"
      }
    ]
  },
  {
    "id": "capture-replay-async-body-return-concurrent-caught-throwing-then-getter-deferred",
    "package": "@solid-primitives/queue",
    "install": "/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-UhzpZA",
    "source": "import {createMemo,createSignal,Loading,flush,getOwner,getObserver,untrack} from 'solid-js';import {render} from '@solidjs/web';import {createConcurrentTaskQueue} from '@solid-primitives/queue';import {makeTask} from './consumer';const h=(globalThis as any).__experiment;function App(){const [value,set]=createSignal(1);const queue=createConcurrentTaskQueue<{value:number}>(1);h.values.calls=0;h.values.thenGets=0;h.values.finally=0;h.values.getterContexts=[];const get=()=>{h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return value();};h.update=()=>{set(2);flush();};const result=createMemo(()=>{const read=(() => { const __solidCaptureReplay = (get)(); return () => __solidCaptureReplay; })();return Promise.resolve().then(async()=>{void queue.enqueue(()=>new Promise<{value:number}>(resolve=>setTimeout(()=>resolve({value:0}),1)));try{return await queue.enqueue(makeTask(read));}catch{return {value:9};}});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(result().value)}</p></Loading>;}h.dispose=render(()=><App/>,document.getElementById('root')!);",
    "artifactOrigin": "automatically-generated-source-capture-proposal-for-isolated-replay",
    "stages": [
      {
        "id": "initial",
        "helper": "const h=(globalThis as any).__experiment;async function child(read:()=>number){await Promise.resolve();return {value:read()};}export function makeTask(read:()=>number){return async()=>{await new Promise<void>(resolve=>setTimeout(resolve,1));h.values.calls++;read();return Object.defineProperty({value:0},\"then\",{get(){h.values.thenGets++;throw new Error(\"expected getter rejection\");}});};}",
        "initial": "9",
        "afterUpdate": "9",
        "desired": "9",
        "role": "control"
      }
    ]
  },
  {
    "id": "capture-replay-async-body-return-concurrent-never-settling-promise-deferred",
    "package": "@solid-primitives/queue",
    "install": "/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-UhzpZA",
    "source": "import {createMemo,createSignal,Loading,flush,getOwner,getObserver,untrack} from 'solid-js';import {render} from '@solidjs/web';import {createConcurrentTaskQueue} from '@solid-primitives/queue';import {makeTask} from './consumer';const h=(globalThis as any).__experiment;function App(){const [value,set]=createSignal(1);const queue=createConcurrentTaskQueue<{value:number}>(1);h.values.calls=0;h.values.thenGets=0;h.values.finally=0;h.values.getterContexts=[];const get=()=>{h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return value();};h.update=()=>{set(2);flush();};const result=createMemo(()=>{const read=(() => { const __solidCaptureReplay = (get)(); return () => __solidCaptureReplay; })();return Promise.resolve().then(async()=>{void queue.enqueue(()=>new Promise<{value:number}>(resolve=>setTimeout(()=>resolve({value:0}),1)));return await queue.enqueue(makeTask(read));});});return <Loading fallback={<p id=\"value\">waiting</p>}><p id='value'>{String(result().value)}</p></Loading>;}h.dispose=render(()=><App/>,document.getElementById('root')!);",
    "artifactOrigin": "automatically-generated-source-capture-proposal-for-isolated-replay",
    "stages": [
      {
        "id": "initial",
        "helper": "const h=(globalThis as any).__experiment;async function child(read:()=>number){await Promise.resolve();return {value:read()};}export function makeTask(read:()=>number){return async()=>{await new Promise<void>(resolve=>setTimeout(resolve,1));h.values.calls++;read();return new Promise<{value:number}>(()=>{});};}",
        "initial": "waiting",
        "afterUpdate": "waiting",
        "desired": "waiting",
        "role": "control"
      }
    ]
  }
];
