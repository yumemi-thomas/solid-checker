// One authenticated TypeScript program per project, reused while inputs agree.
// Includes configuration, ambient declarations and resolution misses in freshness.
import assert from 'node:assert/strict';
import {existsSync,readFileSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {performance} from 'node:perf_hooks';
import {hash} from './catalog.mjs';
import {ts} from './lower.mjs';
import {oracleCompilerOptions} from '../../scripts/tsc-oracle.mjs';

export function projectReadSession(root,{configPath=join(root,'tsconfig.json')}={}){
  root=resolve(root);configPath=resolve(configPath);
  let cached=null,generation=0;const requested=new Set(),stats={builds:0,reuses:0,validationMs:0,buildMs:0};
  function fresh(record){
    for(const input of record.inputs.values()){
      if(input.kind==='read'&&hash(ts.sys.readFile(input.path)??'<missing>')!==input.sha256)return false;
      if(input.kind==='file'&&ts.sys.fileExists(input.path)!==input.exists)return false;
      if(input.kind==='directory'&&ts.sys.directoryExists(input.path)!==input.exists)return false;
      if(input.kind==='realpath'&&(ts.sys.realpath?.(input.path)??input.path)!==input.result)return false;
      if(input.kind==='listing'&&JSON.stringify(ts.sys.readDirectory(...input.arguments))!==input.result)return false;
    }
    return true;
  }
  function build(){
    const start=performance.now(),inputs=new Map();
    function record(input){const key=input.kind+':'+input.path+':'+JSON.stringify(input.arguments??[]);inputs.set(key,input);}
    const system={...ts.sys,
      readFile(path){path=resolve(path);const text=ts.sys.readFile(path);record({kind:'read',path,sha256:hash(text??'<missing>')});return text;},
      fileExists(path){path=resolve(path);const exists=ts.sys.fileExists(path);record({kind:'file',path,exists});return exists;},
      directoryExists(path){path=resolve(path);const exists=ts.sys.directoryExists(path);record({kind:'directory',path,exists});return exists;},
      realpath(path){path=resolve(path);const result=ts.sys.realpath?.(path)??path;record({kind:'realpath',path,result});return result;},
      readDirectory(...args){const result=ts.sys.readDirectory(...args);record({kind:'listing',path:resolve(args[0]),arguments:args,result:JSON.stringify(result)});return result;},
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
    Object.assign(host,{readFile:system.readFile,fileExists:system.fileExists,directoryExists:system.directoryExists,realpath:system.realpath,
      getSourceFile(path,languageVersion,onError){try{const text=system.readFile(path);return text===undefined?undefined:ts.createSourceFile(path,text,languageVersion,true);}catch(error){onError?.(error.message);return undefined;}}});
    const program=ts.createProgram(parsed.fileNames,parsed.options,host),errors=[...parsed.errors,...ts.getPreEmitDiagnostics(program)].filter(error=>error.category===ts.DiagnosticCategory.Error);
    const buildMs=performance.now()-start;
    stats.builds++;stats.buildMs+=buildMs;
    return {program,errors,inputs,generation:++generation,buildMs,configured:system.fileExists(configPath)};
  }
  return {stats,get(path,code){
    path=resolve(path);assert(path.startsWith(root+'/'),'Consumer is outside the project session');
    assert.equal(readFileSync(path,'utf8'),code,'Consumer bytes differ from the served original file');
    const newRoot=!requested.has(path);requested.add(path);
    const start=performance.now(),reuse=cached&&(!newRoot||cached.configured)&&fresh(cached);
    const validationMs=performance.now()-start;stats.validationMs+=validationMs;
    if(reuse)stats.reuses++;else cached=build();
    const source=cached.program.getSourceFile(path);
    assert(source,'Consumer is absent from the configured TypeScript program');
    assert.equal(source.text,code,'TypeScript source bytes differ from the served consumer');
    return {program:cached.program,source,errors:cached.errors,generation:cached.generation,reused:!!reuse,
      validationMs,buildMs:reuse?0:cached.buildMs,inputCount:cached.inputs.size};
  },inputs(){return cached?[...cached.inputs.values()]:[];}};
}
