// Inspect retained real projects without manufacturing runtime observations.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {dirname,join,relative,resolve} from 'node:path';
import {performance} from 'node:perf_hooks';
import {hash,nativeRuntimeRoots,packageDigest,read} from './catalog.mjs';
import {ts} from './lower.mjs';
import {asyncReadSitesV2} from './async-read-sites-v2.mjs';
import {readProgram} from './async-read-transform-v2.mjs';

const [metricArg,outputArg]=process.argv.slice(2);
assert(metricArg&&outputArg,'Usage: async-read-app-inventory-v1.mjs <metric.json> <fresh-output.json>');
const metricPath=resolve(metricArg),output=resolve(outputArg),metric=read(metricPath);
assert(!existsSync(output));
const pins=new Map(),projects=[],startedAt=new Date().toISOString();
function pin(path,text=readFileSync(path)){path=resolve(path);const digest=hash(text);if(pins.has(path))assert.equal(pins.get(path),digest,path);pins.set(path,digest);}
pin(metricPath);pin(new URL(import.meta.url).pathname);
for(const name of ['async-read-sites-v2.mjs','async-read-transform-v2.mjs','lower.mjs','catalog.mjs'])pin(new URL(name,import.meta.url).pathname);
const runtimePins=new Map();
function save(){writeFileSync(output,JSON.stringify({authority:false,certification:false,startedAt,projects,
  inputs:[...pins].map(([path,sha256])=>({path,sha256})),runtimePins:[...runtimePins.values()]},null,2)+'\n');}
for(const app of metric.apps)for(const [configName,historical]of Object.entries(app.solid??{})){
  if(historical['solid-js']!=='2.0.0-rc.9')continue;
  const appRoot=join(dirname(metricPath),'apps',app.id),configPath=join(appRoot,configName),root=dirname(configPath),row={app:app.id,config:configName,root};
  projects.push(row);
  try{
    row.runtime=nativeRuntimeRoots(root).map(path=>({path,name:read(join(path,'package.json')).name,version:read(join(path,'package.json')).version}));
    if(row.runtime.some(item=>item.version!=='2.0.0-rc.9')){row.refused='current installed runtime differs from audited rc.9';save();continue;}
    for(const runtime of row.runtime)if(!runtimePins.has(runtime.path))runtimePins.set(runtime.path,{...runtime,digest:packageDigest(runtime.path)});
    const configReader=path=>{const text=ts.sys.readFile(path);if(text!==undefined)pin(path,text);return text;};
    const config=ts.readConfigFile(configPath,configReader);
    assert(!config.error,ts.flattenDiagnosticMessageText(config.error?.messageText??'','\n'));
    const parsed=ts.parseJsonConfigFileContent(config.config,{...ts.sys,readFile:configReader},root,{},configPath);
    const start=performance.now(),program=ts.createProgram(parsed.fileNames,parsed.options),errors=[...parsed.errors,...ts.getPreEmitDiagnostics(program)].filter(item=>item.category===ts.DiagnosticCategory.Error);
    row.programAndTypingMs=performance.now()-start;
    row.errors=errors.map(error=>({code:error.code,path:error.file?.fileName,start:error.start,message:ts.flattenDiagnosticMessageText(error.messageText,'\n')}));
    const sources=program.getSourceFiles().filter(source=>!source.isDeclarationFile&&source.fileName.startsWith(appRoot+'/')&&!source.fileName.includes('/node_modules/'));
    for(const source of program.getSourceFiles())if(existsSync(source.fileName))pin(source.fileName,source.text);
    row.sourceFiles=sources.length;row.sourceBytes=sources.reduce((sum,source)=>sum+Buffer.byteLength(source.text),0);
    row.files=[];const scan=performance.now();
    for(const source of sources){
      const selection=asyncReadSitesV2(program,source);
      row.files.push({path:source.fileName,relative:relative(appRoot,source.fileName),sha256:hash(source.text),
        currentPluginEligible:source.fileName.startsWith(root+'/src/')&&source.fileName.endsWith('.tsx'),sites:selection.sites,open:selection.open});
    }
    row.selectionMs=performance.now()-scan;
    row.candidateSites=row.files.reduce((sum,file)=>sum+file.sites.length,0);
    // Bounded comparison of the current per-file program on the exact same bytes.
    // This does not execute a package or assert a candidate is a defect.
    const probes=row.files.filter(file=>file.currentPluginEligible).slice(0,3);row.currentPerFileProbes=[];
    for(const file of probes){
      const text=readFileSync(file.path,'utf8'),start=performance.now(),isolated=readProgram(text,file.path),isolatedErrors=ts.getPreEmitDiagnostics(isolated).filter(error=>error.category===ts.DiagnosticCategory.Error);
      const source=isolated.getSourceFile(file.path),selection=asyncReadSitesV2(isolated,source);
      row.currentPerFileProbes.push({path:file.path,totalMs:performance.now()-start,typingErrors:isolatedErrors.map(error=>({code:error.code,path:error.file?.fileName,message:ts.flattenDiagnosticMessageText(error.messageText,'\n')})),candidateSites:selection.sites.length});
    }
  }catch(error){row.refused=error.message;}
  save();console.log(JSON.stringify({app:row.app,sources:row.sourceFiles,candidates:row.candidateSites,typingErrors:row.errors?.length,ms:row.programAndTypingMs,refused:row.refused}));
}
for(const [path,digest]of pins)assert.equal(hash(readFileSync(path)),digest,path);
for(const runtime of runtimePins.values())assert.equal(packageDigest(runtime.path),runtime.digest,runtime.path);
save();
const report=read(output);report.finishedAt=new Date().toISOString();
report.summary={projects:projects.length,refused:projects.filter(row=>row.refused).length,sourceFiles:projects.reduce((sum,row)=>sum+(row.sourceFiles??0),0),
  candidateSites:projects.reduce((sum,row)=>sum+(row.candidateSites??0),0),projectsWithTypingErrors:projects.filter(row=>row.errors?.length).length,
  claim:'source enrollment and measured local analysis cost only; no runtime detection or accuracy score'};
writeFileSync(output,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report.summary));
