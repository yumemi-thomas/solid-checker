// Bind observed feedback to the complete recorded TypeScript input revision.
// Runtime source bodies outside that program still need their own witnesses.
import assert from 'node:assert/strict';
import {randomUUID} from 'node:crypto';
import {readFileSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {performance} from 'node:perf_hooks';
import {hash} from './catalog.mjs';
import {ts} from './lower.mjs';
import {oracleCompilerOptions} from '../../scripts/tsc-oracle.mjs';

export function validateProjectInputs(inputs){
  for(const input of inputs){
    let matches=false;
    if(input.kind==='read'){
      const text=ts.sys.readFile(input.path);
      matches=(text!==undefined)===input.exists&&(text===undefined?null:hash(text))===input.sha256;
    }else if(input.kind==='file')matches=ts.sys.fileExists(input.path)===input.exists;
    else if(input.kind==='directory')matches=ts.sys.directoryExists(input.path)===input.exists;
    else if(input.kind==='realpath')matches=(ts.sys.realpath?.(input.path)??input.path)===input.result;
    else if(input.kind==='listing')matches=JSON.stringify(ts.sys.readDirectory(...input.arguments))===input.result;
    else if(input.kind==='directories')matches=JSON.stringify(ts.sys.getDirectories(input.path))===input.result;
    else return {valid:false,reason:'unknown project input kind',path:input.path};
    if(!matches)return {valid:false,reason:'project input changed',path:input.path,kind:input.kind};
  }
  return {valid:true};
}
export function projectReadSession(root,{configPath=join(root,'tsconfig.json')}={}){
  root=resolve(root);configPath=resolve(configPath);
  const sessionId=randomUUID(),issued=new Map(),requested=new Set(),stats={builds:0,reuses:0,validationMs:0,buildMs:0,invalidations:0};
  let cached=null,generation=0,invalidation=0;
  function build(){
    const started=performance.now(),inputs=new Map();
    const record=input=>inputs.set(input.kind+':'+input.path+':'+JSON.stringify(input.arguments??[]),input);
    const system={...ts.sys,
      readFile(path){path=resolve(path);const text=ts.sys.readFile(path);record({kind:'read',path,exists:text!==undefined,sha256:text===undefined?null:hash(text)});return text;},
      fileExists(path){path=resolve(path);const exists=ts.sys.fileExists(path);record({kind:'file',path,exists});return exists;},
      directoryExists(path){path=resolve(path);const exists=ts.sys.directoryExists(path);record({kind:'directory',path,exists});return exists;},
      realpath(path){path=resolve(path);const result=ts.sys.realpath?.(path)??path;record({kind:'realpath',path,result});return result;},
      readDirectory(...args){const result=ts.sys.readDirectory(...args);record({kind:'listing',path:resolve(args[0]),arguments:args,result:JSON.stringify(result)});return result;},
      getDirectories(path){path=resolve(path);const result=ts.sys.getDirectories(path);record({kind:'directories',path,result:JSON.stringify(result)});return result;},
    };
    let parsed;
    if(system.fileExists(configPath)){
      const config=ts.readConfigFile(configPath,system.readFile);
      assert(!config.error,ts.flattenDiagnosticMessageText(config.error?.messageText??'','\n'));
      parsed=ts.parseJsonConfigFileContent(config.config,system,resolve(configPath,'..'),{},configPath);
      assert(!parsed.projectReferences?.length,'Referenced TypeScript projects need an explicit project session');
    }else{
      const options=ts.convertCompilerOptionsFromJson({...oracleCompilerOptions('v2',true,{customConditions:['browser','development']}),allowJs:true},root);
      parsed={fileNames:[...requested],options:options.options,errors:options.errors};
    }
    const host=ts.createCompilerHost(parsed.options);
    Object.assign(host,{readFile:system.readFile,fileExists:system.fileExists,directoryExists:system.directoryExists,realpath:system.realpath,getDirectories:system.getDirectories,
      getSourceFile(path,languageVersion,onError){try{const text=system.readFile(path);return text===undefined?undefined:ts.createSourceFile(path,text,languageVersion,true);}catch(error){onError?.(error.message);return undefined;}}});
    const program=ts.createProgram(parsed.fileNames,parsed.options,host),errors=[...parsed.errors,...ts.getPreEmitDiagnostics(program)].filter(error=>error.category===ts.DiagnosticCategory.Error);
    const configured=system.fileExists(configPath),manifest=[...inputs.values()].sort((a,b)=>JSON.stringify(a).localeCompare(JSON.stringify(b)));
    const validated=validateProjectInputs(manifest);assert(validated.valid,'Project changed while its TypeScript program was being built');
    const revision=Object.freeze({sessionId,generation:++generation,invalidation,inputSha256:hash(JSON.stringify(manifest))});
    issued.clear();issued.set(JSON.stringify(revision),{revision,inputs:manifest,program});
    const buildMs=performance.now()-started;stats.builds++;stats.buildMs+=buildMs;
    return {program,errors,inputs:manifest,generation,revision,buildMs,configured};
  }
  return {stats,get(path,code){
    path=resolve(path);assert(path.startsWith(root+'/'),'Consumer is outside the project session');
    assert.equal(readFileSync(path,'utf8'),code,'Consumer bytes differ from the served original file');
    const newRoot=!requested.has(path);requested.add(path);
    const started=performance.now(),reuse=!!cached&&(!newRoot||cached.configured)&&validateProjectInputs(cached.inputs).valid;
    const validationMs=performance.now()-started;stats.validationMs+=validationMs;
    if(reuse)stats.reuses++;else cached=build();
    const source=cached.program.getSourceFile(path);assert(source,'Consumer is absent from the configured TypeScript program');
    assert.equal(source.text,code,'TypeScript source bytes differ from the served consumer');
    return {program:cached.program,source,errors:cached.errors,generation:cached.generation,revision:cached.revision,reused:reuse,
      validationMs,buildMs:reuse?0:cached.buildMs,inputCount:cached.inputs.length};
  },acceptRevision(revision){
    if(revision?.sessionId===sessionId&&(revision.invalidation!==invalidation||revision.generation!==generation))return {valid:false,reason:'observation belongs to a retired project revision'};
    const entry=issued.get(JSON.stringify(revision));
    if(!entry)return {valid:false,reason:'observation has no issued project revision'};
    if(entry.revision.invalidation!==invalidation||entry.program!==cached?.program)return {valid:false,reason:'observation belongs to a retired project revision'};
    return validateProjectInputs(entry.inputs);
  },invalidate(){invalidation++;stats.invalidations++;cached=null;issued.clear();},
  inputs(){return cached?structuredClone(cached.inputs):[];}};
}
