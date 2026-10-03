// Execute unchanged retained app source; runtime context/result coverage remains partial.
import assert from 'node:assert/strict';
import {existsSync,mkdirSync,readFileSync,readdirSync,writeFileSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {performance} from 'node:perf_hooks';
import {setTimeout as delay} from 'node:timers/promises';
import {hash,read,packageRoot,closurePins} from './catalog.mjs';
import factory from './async-read-transform-v22.mjs';
import {nativeReadFeedback} from './native-read-feedback-v15.mjs';
const [casesArg,sealArg,outArg,executablePath,variant='reads']=process.argv.slice(2);
const casesPath=resolve(casesArg),sealPath=resolve(sealArg),out=resolve(outArg);
assert(executablePath&&!existsSync(out)&&['reads','plain'].includes(variant));mkdirSync(out,{recursive:true});
const cases=(await import(pathToFileURL(casesPath))).default,seal=read(sealPath);
const authenticate=()=>{for(const pin of [...seal.files,seal.baseline])assert.equal(hash(readFileSync(pin.path)),pin.sha256,pin.path);};authenticate();
const tooling=resolve('rust/target/app-import-metric/apps/helge-dev');
const load=(name,entry)=>import(pathToFileURL(join(packageRoot(tooling,name),entry)));
const {createServer}=await load('vite','dist/node/index.js'),{default:solid}=await load('@solidjs/vite-plugin','dist/esm/index.mjs');
const {chromium}=await load('playwright','index.mjs'),{TraceMap,originalPositionFor}=await load('@jridgewell/trace-mapping','dist/trace-mapping.mjs');
const packageRoots=[...new Set(['vite','@solidjs/vite-plugin','playwright','@jridgewell/trace-mapping','solid-js','@solidjs/signals','@solidjs/web','@solidjs/router','@solidjs/meta','solid-icons'].map(name=>packageRoot(tooling,name)))];
function appPins(root){const files=[];function visit(folder){for(const entry of readdirSync(folder,{withFileTypes:true})){if(['node_modules','.git','build','dist','test-results','playwright-report'].includes(entry.name))continue;const path=join(folder,entry.name);if(entry.isDirectory())visit(path);else if(entry.isFile())files.push({path,sha256:hash(readFileSync(path))});}}visit(root);return files.sort((a,b)=>a.path.localeCompare(b.path));}
const report={authority:false,certification:false,variant,startedAt:new Date().toISOString(),scope:'offline execution of retained app code with reviewed adapted dev config; no seeded defects, real-app correctness or complete package coverage claim',
  inputs:{files:[casesPath,sealPath].map(path=>({path,sha256:hash(readFileSync(path))})),packages:packageRoots.map(root=>({root,pins:closurePins(root)})),applications:cases.map(row=>({id:row.id,root:row.root,files:appPins(row.root)}))},results:[]};
const save=()=>writeFileSync(join(out,'results.json'),JSON.stringify(report,null,2)+'\n');save();
const browser=await chromium.launch({executablePath,headless:true,timeout:15000});report.browser=browser.version();
try{for(const app of cases){
  const row={id:app.id,root:app.root,scope:app.scope,steps:[],pageErrors:[],consoleErrors:[],blockedRequests:[],requestFailures:[],authority:false,certification:false};report.results.push(row);save();
  const plugin=variant==='reads'?factory():null,timings=[];
  if(plugin){const transform=plugin.transform.bind(plugin);plugin.transform=(code,id)=>{const start=performance.now();try{return transform(code,id);}finally{timings.push({id,ms:performance.now()-start});}};}
  const start=performance.now();let context,server;
  try{
    server=await createServer({configFile:false,root:app.root,cacheDir:join(out,app.id,'vite-cache'),publicDir:app.config.publicDir,
      plugins:[...(plugin?[plugin]:[]),{name:'retained-app-read-entry',transformIndexHtml:{order:'pre',handler(){return[{tag:'script',attrs:{type:'module'},injectTo:'head-prepend',children:`import {nativeReads} from ${JSON.stringify('/@fs'+new URL('./native-read-runtime-v8.mjs',import.meta.url).pathname)};import * as Solid from 'solid-js';globalThis.__realReadApp={diagnostics:[],events:nativeReads.events};Solid.OBSERVE?.diagnostics?.subscribe(event=>globalThis.__realReadApp.diagnostics.push({code:event.code,kind:event.kind,severity:event.severity}));` }];}}},solid({hot:false})],
      resolve:{alias:app.config.alias,dedupe:['solid-js','@solidjs/signals','@solidjs/web']},optimizeDeps:{noDiscovery:true,include:[],exclude:['solid-js','@solidjs/signals','@solidjs/web','solid-icons']},server:{host:'127.0.0.1',port:0,fs:{allow:[resolve('.'),app.root]}},logLevel:'error'});
    await server.listen();const origin=`http://127.0.0.1:${server.httpServer.address().port}`;row.serverStartupMs=performance.now()-start;
    context=await browser.newContext({viewport:{width:1280,height:900}});context.setDefaultTimeout(15000);
    await context.route('**/*',route=>{const url=new URL(route.request().url());if(url.origin===origin)return route.continue();row.blockedRequests.push({url:url.href,type:route.request().resourceType()});return route.abort();});
    const page=await context.newPage();page.on('pageerror',error=>row.pageErrors.push({message:error.message,stack:error.stack}));page.on('console',message=>{if(message.type()==='error')row.consoleErrors.push(message.text());});page.on('requestfailed',request=>row.requestFailures.push({path:new URL(request.url()).pathname,failure:request.failure()}));
    const maps=new Map();async function mapped(frame){const url=new URL(frame.path);if(url.origin!==origin)return null;const key=url.pathname+url.search;if(!maps.has(key)){let result;try{result=await server.transformRequest(key);}catch{}maps.set(key,result?.map?new TraceMap(result.map):null);}const map=maps.get(key);if(!map)return null;const position=originalPositionFor(map,{line:frame.line,column:frame.column-1}),served=url.pathname.startsWith('/@fs/')?decodeURIComponent(url.pathname.slice(4)):join(app.root,decodeURIComponent(url.pathname)),path=position.source&&position.line?resolve(dirname(served),position.source):null;return path&&existsSync(path)?{path,line:position.line,column:position.column+1,name:position.name,sourceSha256:hash(readFileSync(path))}:null;}
    for(const action of app.actions){const began=performance.now();
      if(action.kind==='goto')await page.goto(origin+action.path,{waitUntil:'networkidle'});
      else if(action.kind==='click')await page.locator(action.selector).first().click();
      else if(action.kind==='corner-click')await page.mouse.click(5,5);
      else if(action.kind==='viewport')await page.setViewportSize({width:action.width,height:action.height});
      else assert.fail('Unknown action');
      if(action.title)await page.waitForFunction(title=>document.title===title,action.title);
      if(action.visible)await page.locator(action.visible).first().waitFor({state:'visible'});
      if(action.hidden)await page.locator(action.hidden).first().waitFor({state:'hidden'});
      if(action.closedMenu)assert(!(await page.locator('.NavBar').getAttribute('class')).split(' ').includes('open'));
      await delay(50);
      const snapshot=await page.evaluate(()=>({title:document.title,path:location.pathname,text:document.body.innerText,headings:[...document.querySelectorAll('h1,h2,h3')].map(element=>element.textContent),links:[...document.querySelectorAll('a')].map(element=>({text:element.textContent,href:element.getAttribute('href')})),modal:!!document.querySelector('.modal'),menuOpen:document.querySelector('.NavBar')?.classList.contains('open')??false,diagnostics:globalThis.__realReadApp?.diagnostics??[],events:globalThis.__realReadApp?.events??[],continuationGaps:globalThis.__nativeNodeReads?.continuationGaps??[],identityStats:globalThis.__nativeNodeReads?.stats??null,callbackSlotStats:globalThis.__callbackSlots?.stats??null}));
      for(const event of snapshot.events){event.originalFrames=await Promise.all(event.frames.map(mapped));event.identity.originalCreationFrames=await Promise.all(event.identity.creationFrames.map(mapped));event.nativeRead.originalFrames=await Promise.all(event.nativeRead.frames.map(mapped));if(event.callbackRegistration){const registration=event.callbackRegistration;for(const [target,key]of [['originalRegistrationFrames','registrationFrames'],['originalInvocationFrames','invocationFrames'],['originalEntryFrames','entryFrames']])registration[target]=await Promise.all(registration[key].map(mapped));}if(event.asyncContinuation)for(const link of event.asyncContinuation.chain)link.originalEntryFrames=await Promise.all(link.entryFrames.map(mapped));}
      const projections=[];if(plugin)for(const path of [...new Set(snapshot.events.map(event=>event.site.path))]){const code=readFileSync(path,'utf8');projections.push({path,...nativeReadFeedback(plugin.session,path,code,snapshot.events.filter(event=>event.site.path===path))});}
      row.steps.push({id:action.id,elapsedMs:performance.now()-began,snapshot,projections});save();
    }
    if(plugin){let state=plugin.session.get(join(app.root,'src/index.tsx'),readFileSync(join(app.root,'src/index.tsx'),'utf8'));row.publishedTypingErrors=state.errors.map(d=>({code:d.code,message:String(d.messageText)}));row.sourceFactDiagnostics=state.sourceErrors.map(d=>({code:d.code,message:String(d.messageText)}));assert.deepEqual(row.publishedTypingErrors,[]);row.revision=state.revision;row.inputManifest=plugin.session.inputs();row.runtimeSources=plugin.session.runtimeSources();row.sourceOpen=plugin.session.sourceOpen();state=null;
      row.instrumentation={transformed:plugin.transformed,refused:plugin.refused,native:plugin.nativeHook.transformed,nativeRefused:plugin.nativeHook.refused,shortcuts:plugin.shortcutHook.transformed,shortcutRefused:plugin.shortcutHook.refused,stats:{...plugin.session.stats},cacheInvalidations:plugin.cacheInvalidations,unchangedUpdates:plugin.unchangedUpdates,timings};}
    row.completed=true;
  }catch(error){row.failure={message:error.message,stack:error.stack};save();}
  finally{await context?.close();await server?.close();plugin?.session?.invalidate();row.elapsedMs=performance.now()-start;row.lifecycle={analysisInputsRetiredAfterClose:!!plugin};globalThis.gc?.();row.memoryAfterCollection=process.memoryUsage();save();}
}}
finally{await browser.close();}
authenticate();for(const pin of report.inputs.files)assert.equal(hash(readFileSync(pin.path)),pin.sha256);for(const app of report.inputs.applications)assert.deepEqual(appPins(app.root),app.files);for(const pkg of report.inputs.packages)assert.deepEqual(closurePins(pkg.root),pkg.pins);
report.finishedAt=new Date().toISOString();save();console.log(JSON.stringify({variant,apps:report.results.length,completed:report.results.filter(row=>row.completed).length,steps:report.results.reduce((sum,row)=>sum+row.steps.length,0),failures:report.results.filter(row=>row.failure).map(row=>({id:row.id,message:row.failure.message}))}));
