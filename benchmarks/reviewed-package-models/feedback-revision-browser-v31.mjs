// Live edits/reloads of authored consumers of retained published packages.
// This is a research session boundary, not a production HMR integration.
// Optional collection happens only after an application/server has closed.
import assert from 'node:assert/strict';
import {existsSync,mkdirSync,readFileSync,statSync,symlinkSync,writeFileSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {setTimeout as delay} from 'node:timers/promises';
import {hash,read,closurePins,packageRoot} from './catalog.mjs';
import pluginFactory from './async-read-transform-v27.mjs';
import {projectReadSession} from './project-read-session-v2.mjs';
import {nativeReadFeedback as sessionFeedback} from './native-read-feedback-v28.mjs';
import {nativeReadFeedback as earlierFeedback} from './native-read-feedback-v9.mjs';
import {ts} from './lower.mjs';

const [casesArg,sealArg,outArg,executablePath,variant='reads']=process.argv.slice(2),casesPath=resolve(casesArg),sealPath=resolve(sealArg),out=resolve(outArg);
assert(executablePath&&!existsSync(out)&&['plain','reads'].includes(variant));mkdirSync(out,{recursive:true});
const seal=read(sealPath),auth=()=>{for(const pin of [...seal.files,seal.baseline])assert.equal(hash(readFileSync(pin.path)),pin.sha256,pin.path);};auth();
const frozenBeforeChallenge=new Date(seal.frozenAt).getTime()<statSync(casesPath).mtimeMs;
const cases=(await import(pathToFileURL(casesPath))).default,repo=resolve('.'),tooling=join(repo,'rust/target/app-import-metric/apps/helge-dev');
const load=(name,entry)=>import(pathToFileURL(join(packageRoot(tooling,name),entry)));
const {createServer}=await load('vite','dist/node/index.js'),{default:solid}=await load('@solidjs/vite-plugin','dist/esm/index.mjs');
const {chromium}=await load('playwright','index.mjs'),{TraceMap,originalPositionFor}=await load('@jridgewell/trace-mapping','dist/trace-mapping.mjs');
const roots=[...new Set([...cases.map(row=>packageRoot(row.install,row.package)),...['vite','@solidjs/vite-plugin','playwright','@jridgewell/trace-mapping'].map(name=>packageRoot(tooling,name))])];
const before={files:[{path:casesPath,sha256:hash(readFileSync(casesPath))},{path:sealPath,sha256:hash(readFileSync(sealPath))}],packages:roots.map(root=>({root,pins:closurePins(root)}))};
const report={authority:false,certification:false,variant,startedAt:new Date().toISOString(),detectorFrozenBeforeChallenge:frozenBeforeChallenge,populationStatus:frozenBeforeChallenge?'authored-after-current-detector-freeze':'adapted-replay-of-prior-population',inputs:before,results:[]};
const save=()=>writeFileSync(join(out,'results.json'),JSON.stringify(report,null,2)+'\n');save();
const browser=await chromium.launch({executablePath,headless:true,timeout:15000});report.browser=browser.version();
try{
  for(const challenge of cases){
    const root=join(out,challenge.id);mkdirSync(join(root,'src'),{recursive:true});symlinkSync(join(challenge.install,'node_modules'),join(root,'node_modules'),'dir');
    const main=join(root,'src/main.tsx'),helper=join(root,'src/consumer.ts'),configPath=join(root,'tsconfig.json');
    writeFileSync(main,challenge.source);writeFileSync(helper,challenge.stages[0].helper);
    for(const [name,code]of Object.entries(challenge.files??{})){assert(!name.startsWith('/')&&!name.split('/').includes('..')&&!['main.tsx','consumer.ts'].includes(name));writeFileSync(join(root,'src',name),code);}
    writeFileSync(configPath,JSON.stringify({compilerOptions:{target:'ESNext',module:'ESNext',moduleResolution:'bundler',jsx:'preserve',jsxImportSource:'@solidjs/web',strict:true,skipLibCheck:true,allowJs:true,...(challenge.noEmit?{noEmit:true}:{})},include:['src']}));
    writeFileSync(join(root,'index.html'),'<div id="root"></div><script type="module" src="/src/main.tsx"></script>');
    writeFileSync(join(root,'feedback-entry.mjs'),`import {nativeReads} from ${JSON.stringify('/@fs'+new URL('./native-read-runtime-v12.mjs',import.meta.url).pathname)};import * as Solid from 'solid-js';const h=globalThis.__experiment={loadId:crypto.randomUUID(),values:{},feedback:[],errors:[],identityTrace:nativeReads.events};Solid.OBSERVE?.diagnostics?.subscribe(event=>h.feedback.push({code:event.code,kind:event.kind,severity:event.severity}));`);
    const plugin=variant==='reads'?pluginFactory():null,plainSession=variant==='plain'?projectReadSession(root):null;
    const simulatedConfigEvents=[];
    if(plugin&&challenge.duplicateConfigBeforeHelper){const transform=plugin.transform.bind(plugin);plugin.transform=(code,id)=>{
      if(id.split('?')[0]===helper){const mainRecord=plugin.transformed.findLast(row=>row.path===main);assert(mainRecord,'configuration event must follow the served consumer transform');const before=mainRecord.session.revision;plugin.handleHotUpdate({file:configPath,timestamp:Date.now(),server});const result=transform(code,id),after=plugin.transformed.findLast(row=>row.path===helper)?.session.revision;simulatedConfigEvents.push({scope:'simulated unchanged configuration event before helper transform',before,after});return result;}
      return transform(code,id);
    };}
    const server=await createServer({configFile:false,root,cacheDir:join(root,'.vite-cache'),publicDir:false,
      plugins:[...(plugin?[plugin]:[]),{name:'revision-observer',transformIndexHtml:{order:'pre',handler(){return[{tag:'script',attrs:{type:'module',src:'/feedback-entry.mjs'},injectTo:'head-prepend'}];}}},solid({hot:false})],
      resolve:{dedupe:['solid-js','@solidjs/signals','@solidjs/web']},optimizeDeps:{noDiscovery:true,include:[],exclude:['solid-js','@solidjs/signals','@solidjs/web']},
      server:{host:'127.0.0.1',port:0,fs:{allow:[repo,challenge.install,root]}},logLevel:'error'});
    const item={id:challenge.id,package:challenge.package,sourceProfile:{dependencyDiscovery:false},stages:[],pageErrors:[],blockedRequests:[],sourceChanges:[],authority:false,certification:false};report.results.push(item);save();let context;
    try{
      await server.listen();const origin=`http://127.0.0.1:${server.httpServer.address().port}`;
      context=await browser.newContext();context.setDefaultTimeout(12000);
      await context.route('**/*',route=>{const url=new URL(route.request().url());if(url.origin===origin)return route.continue();item.blockedRequests.push(url.href);return route.abort();});
      const page=await context.newPage();page.on('pageerror',error=>item.pageErrors.push({message:error.message,stack:error.stack}));
      const histories=[];
      for(const[index,stage]of challenge.stages.entries()){
        let previousLoad=null,invalidationsBefore=plugin?.session?.stats.invalidations??0,automaticReload=false;
        if(index){previousLoad=await page.evaluate(()=>globalThis.__experiment.loadId);const changedPath=stage.mainComment?main:stage.config?configPath:helper;
          const previous=readFileSync(changedPath,'utf8'),next=stage.mainComment?challenge.source+stage.mainComment:stage.config?JSON.stringify(stage.config):stage.helper;
          assert.notEqual(next,previous);writeFileSync(changedPath,next);item.sourceChanges.push({stage:stage.id,path:changedPath,before:hash(previous),after:hash(next)});
          const start=Date.now();while(Date.now()-start<8000){
            try{const id=await page.evaluate(()=>globalThis.__experiment?.loadId);if(id&&id!==previousLoad){automaticReload=true;break;}}catch{}
            if(stage.config&&Date.now()-start>800)break;await delay(50);
          }
          if(!automaticReload)await page.reload({waitUntil:'networkidle'});
        }else await page.goto(origin,{waitUntil:'networkidle'});
        if(stage.initial===null)await page.waitForFunction(()=>!!document.getElementById('value'));else await page.waitForFunction(value=>document.getElementById('value')?.textContent===value,stage.initial);
        const observedInitial=await page.evaluate(()=>document.getElementById('value')?.textContent);
        await page.evaluate(()=>globalThis.__experiment.update());await delay(80);
        const session=plugin?.session??plainSession;let state=session.get(main,readFileSync(main,'utf8'));
        const snapshot=await page.evaluate(()=>{const h=globalThis.__experiment;return{loadId:h.loadId,values:h.values,value:document.getElementById('value')?.textContent,visibleText:document.body.innerText,feedback:h.feedback,identityTrace:h.identityTrace,continuationGaps:globalThis.__nativeNodeReads.continuationGaps,identityStats:globalThis.__nativeNodeReads.stats,callbackSlotStats:globalThis.__callbackSlots?.stats??null,errors:h.errors};});
        if(stage.afterUpdate!==null)assert.equal(snapshot.value,stage.afterUpdate);
        const errors=state.errors.map(d=>({code:d.code,message:ts.flattenDiagnosticMessageText(d.messageText,'\n')}));
        if(stage.typingCode)assert(errors.some(error=>error.code===stage.typingCode));else assert.deepEqual(errors,[]);
        const maps=new Map();
        async function mapped(frame){
          const url=new URL(frame.path);if(url.origin!==origin)return null;const key=url.pathname+url.search;
          if(!maps.has(key)){let transformed;try{transformed=await server.transformRequest(key);}catch{}maps.set(key,transformed?.map?new TraceMap(transformed.map):null);}
          const map=maps.get(key);if(!map)return null;const position=originalPositionFor(map,{line:frame.line,column:frame.column-1});
          const served=url.pathname.startsWith('/@fs/')?decodeURIComponent(url.pathname.slice(4)):join(root,decodeURIComponent(url.pathname));
          const path=position.source&&position.line?resolve(dirname(served),position.source):null;
          return path&&existsSync(path)?{path,line:position.line,column:position.column+1,name:position.name,sourceSha256:hash(readFileSync(path))}:null;
        }
        for(const event of snapshot.identityTrace){if(event.callbackRegistration){const registration=event.callbackRegistration;registration.originalRegistrationFrames=await Promise.all(registration.registrationFrames.map(mapped));registration.originalInvocationFrames=await Promise.all(registration.invocationFrames.map(mapped));registration.originalEntryFrames=await Promise.all(registration.entryFrames.map(mapped));}if(event.asyncContinuation)for(const link of event.asyncContinuation.chain)link.originalEntryFrames=await Promise.all(link.entryFrames.map(mapped));event.originalFrames=await Promise.all(event.frames.map(mapped));event.identity.originalCreationFrames=await Promise.all(event.identity.creationFrames.map(mapped));if(event.nativeRead)event.nativeRead.originalFrames=await Promise.all(event.nativeRead.frames.map(mapped));}
        const current=variant==='reads'?sessionFeedback(session,main,state.source.text,snapshot.identityTrace,snapshot.identityStats):null;
        const retired=histories.map(old=>({stage:old.id,eventCount:old.events.length,...(variant==='reads'?sessionFeedback(session,main,state.source.text,old.events,old.identityStats):{})}));
        for(const old of retired)if(variant==='reads'){assert.equal(old.acceptedEvents,0);assert.deepEqual(old.notes,[]);assert.deepEqual(old.suppressed,[]);}
        const legacy=variant==='reads'&&histories.length?earlierFeedback(state.program,state.source,histories[0].events.map(event=>{const {projectRevision,...site}=event.site;return{...event,site};})):null;
        const record={id:stage.id,role:stage.role,sourceSha256:hash(state.source.text),helperSha256:hash(readFileSync(helper)),publishedTypingErrors:errors,
          initial:stage.initial===null?observedInitial:stage.initial,afterUpdate:snapshot.value,desired:stage.desired,behaviorPassed:stage.desired===null?null:snapshot.value===stage.desired,
          automaticReload,explicitReload:index>0&&!automaticReload,loadId:snapshot.loadId,revision:state.revision,
          invalidationsBefore,invalidationsAfter:plugin?.session?.stats.invalidations??0,events:snapshot.identityTrace,current,retired,
          earlierProjectionOfInitialEvents:legacy,feedback:snapshot.feedback,errors:snapshot.errors,continuationGaps:snapshot.continuationGaps,identityStats:snapshot.identityStats,callbackSlotStats:snapshot.callbackSlotStats,values:snapshot.values,visibleText:snapshot.visibleText};
        if(plugin){record.inputManifest=session.inputs();record.runtimeSources=session.runtimeSources();record.sourceOpen=session.sourceOpen();record.sourceFactDiagnostics=state.sourceErrors.map(d=>({code:d.code,message:ts.flattenDiagnosticMessageText(d.messageText,'\n')}));}item.stages.push(record);histories.push({id:stage.id,events:snapshot.identityTrace,identityStats:snapshot.identityStats});save();state=null;
      }
      if(plugin){assert.deepEqual(plugin.refused,[]);assert.deepEqual(plugin.nativeHook.refused,[]);assert.deepEqual(plugin.shortcutHook.refused,[]);item.instrumentation={transformed:plugin.transformed,refused:plugin.refused,native:plugin.nativeHook.transformed,shortcuts:plugin.shortcutHook.transformed,stats:{...plugin.session.stats},cacheInvalidations:plugin.cacheInvalidations,unchangedUpdates:plugin.unchangedUpdates,simulatedConfigEvents};}
      assert.deepEqual(item.pageErrors,[]);
    }catch(error){item.failure={message:error.message,stack:error.stack};save();throw error;}
    finally{await context?.close();await server.close();plugin?.session?.invalidate();plainSession?.invalidate();item.lifecycle={analysisInputsRetiredAfterClose:true};item.memory={beforeCollection:process.memoryUsage(),explicitCollection:typeof globalThis.gc==='function'};if(globalThis.gc)globalThis.gc();item.memory.afterCollection=process.memoryUsage();save();}
  }
  auth();for(const pin of before.files)assert.equal(hash(readFileSync(pin.path)),pin.sha256);for(const pkg of before.packages)assert.deepEqual(closurePins(pkg.root),pkg.pins);
  report.finishedAt=new Date().toISOString();save();console.log(JSON.stringify({variant,packages:report.results.length,stages:report.results.reduce((n,item)=>n+item.stages.length,0),authority:false,certification:false}));
}finally{await browser.close();}






