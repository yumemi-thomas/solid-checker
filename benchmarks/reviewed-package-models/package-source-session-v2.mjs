// Keep published typing diagnostics separate from additional installed JS facts.
// Runtime-source enrollment grants no package behavior or dispatch authority.
import assert from 'node:assert/strict';
import {randomUUID} from 'node:crypto';
import {readFileSync,realpathSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {performance} from 'node:perf_hooks';
import {ts} from './lower.mjs';
import {hash} from './catalog.mjs';
import {projectReadSession,validateProjectInputs} from './project-read-session-v2.mjs';
export function packageSourceSession(root,options={}){
  root=resolve(root);const published=projectReadSession(root,options),sessionId=randomUUID(),issued=new Map();let cached=null,anchor=null,generation=0,invalidation=0;
  const stats={builds:0,reuses:0,buildMs:0,validationMs:0,invalidations:0,sourcePrograms:0,scannedFiles:0,scannedBytes:0};
  function build(publicState){
    const started=performance.now(),inputs=new Map(),runtimeSources=[],packages=new Map(),open=[];let scannedFiles=0,scannedBytes=0;
    const record=input=>{const key=input.kind+':'+input.path+':'+JSON.stringify(input.arguments??[]),old=inputs.get(key);if(old)assert.deepEqual(input,old,'Source input changed during construction');inputs.set(key,input);};
    for(const input of published.inputs())record(input);
    const system={...ts.sys,
      readFile(path){path=resolve(path);const text=ts.sys.readFile(path);record({kind:'read',path,exists:text!==undefined,sha256:text===undefined?null:hash(text)});return text;},
      fileExists(path){path=resolve(path);const exists=ts.sys.fileExists(path);record({kind:'file',path,exists});return exists;},
      directoryExists(path){path=resolve(path);const exists=ts.sys.directoryExists(path);record({kind:'directory',path,exists});return exists;},
      realpath(path){path=resolve(path);const result=ts.sys.realpath?.(path)??path;record({kind:'realpath',path,result});return result;},
      readDirectory(...args){const result=ts.sys.readDirectory(...args);record({kind:'listing',path:resolve(args[0]),arguments:args,result:JSON.stringify(result)});return result;},
      getDirectories(path){path=resolve(path);const result=ts.sys.getDirectories(path);record({kind:'directories',path,result:JSON.stringify(result)});return result;},
    };
    function owner(file){for(let dir=dirname(file);;dir=dirname(dir)){const metadata=join(dir,'package.json');if(system.fileExists(metadata)){const text=system.readFile(metadata),manifest=JSON.parse(text);return {root:system.realpath(dir),metadata,metadataSha256:hash(text),package:manifest.name,version:manifest.version,manifest};}if(dirname(dir)===dir)return null;}}
    function discover(pkg){
      if(!pkg||packages.has(pkg.root))return;if(packages.size>=64){open.push({package:pkg.package,reason:'Runtime source package discovery budget exhausted; package unadmitted'});return;}packages.set(pkg.root,pkg);
      // These packages are the fixed native integration boundary, already hooked.
      if(['solid-js','@solidjs/signals','@solidjs/web'].includes(pkg.package))return;
      const files=system.readDirectory(pkg.root,['.js','.mjs','.cjs'],['**/node_modules/**'],['**/*']).sort();
      const dependencies=[];
      for(const path of files){if(scannedFiles>=512){open.push({package:pkg.package,reason:'Runtime source file discovery budget exhausted; remaining modules unadmitted'});break;}const code=system.readFile(path);scannedFiles++;const bytes=Buffer.byteLength(code);if(scannedBytes+bytes>4*1024*1024){open.push({path,reason:'Runtime source byte budget exhausted; module unadmitted'});continue;}scannedBytes+=bytes;
        const source=ts.createSourceFile(path,code,ts.ScriptTarget.Latest,true);let functions=0;function visit(node){if(ts.isFunctionLike(node)&&node.body&&!node.asteriskToken&&node.modifiers?.some(mod=>mod.kind===ts.SyntaxKind.AsyncKeyword))functions++;ts.forEachChild(node,visit);}visit(source);
        function imports(node){if((ts.isImportDeclaration(node)||ts.isExportDeclaration(node))&&node.moduleSpecifier&&ts.isStringLiteral(node.moduleSpecifier)&&!node.isTypeOnly&&!node.importClause?.isTypeOnly)dependencies.push({specifier:node.moduleSpecifier.text,path});else if(ts.isCallExpression(node)&&node.expression.kind===ts.SyntaxKind.ImportKeyword){if(node.arguments.length===1&&ts.isStringLiteral(node.arguments[0]))dependencies.push({specifier:node.arguments[0].text,path});else open.push({path,start:node.getStart(),reason:'dynamic package runtime import is unresolved'});}ts.forEachChild(node,imports);}imports(source);
        if(!functions)continue;if(!ts.isExternalModule(source)){open.push({path,reason:'runtime async source is not an admitted ES module'});continue;}
        runtimeSources.push({path:system.realpath(path),sourceSha256:hash(code),package:pkg.package,version:pkg.version,root:pkg.root,metadata:pkg.metadata,metadataSha256:pkg.metadataSha256,asyncFunctions:functions});
      }
      for(const item of dependencies){const resolved=ts.resolveModuleName(item.specifier,item.path,{...publicState.program.getCompilerOptions(),allowJs:true,checkJs:false},system).resolvedModule;if(resolved){const child=owner(resolved.resolvedFileName);if(child)discover(child);}else open.push({package:pkg.package,dependency:item.specifier,path:item.path,reason:'package runtime import lacks resolved declaration facts'});}
    }
    let sourceProgram=publicState.program,sourceErrors=[];
    try{
      const checker=publicState.program.getTypeChecker();
      // Only the application's served source seeds external runtime discovery.
      // Declaration imports and build/test configuration do not execute here.
      function importSource(specifier){
        const symbol=checker.getSymbolAtLocation(specifier);
        if(!symbol?.declarations?.length){open.push({path:specifier.getSourceFile().fileName,start:specifier.getStart(),reason:'runtime module import lacks an exact declaration'});return;}
        for(const declaration of symbol.declarations){
          const file=declaration.getSourceFile(),pkg=owner(file.fileName);
          if(!pkg||pkg.root===root||root.startsWith(pkg.root+'/'))continue;discover(pkg);
          if(!file.isDeclarationFile&&!['solid-js','@solidjs/signals','@solidjs/web'].includes(pkg.package)&&ts.isExternalModule(file)){
            let functions=0;function visit(node){if(ts.isFunctionLike(node)&&node.body&&!node.asteriskToken&&node.modifiers?.some(mod=>mod.kind===ts.SyntaxKind.AsyncKeyword))functions++;ts.forEachChild(node,visit);}visit(file);
            const path=system.realpath(file.fileName);if(functions&&!runtimeSources.some(row=>row.path===path))runtimeSources.push({path,sourceSha256:hash(file.text),package:pkg.package,version:pkg.version,root:pkg.root,metadata:pkg.metadata,metadataSha256:pkg.metadataSha256,asyncFunctions:functions});
          }
        }
      }
      for(const file of publicState.program.getSourceFiles()){
        if(file.isDeclarationFile||!file.fileName.startsWith(root+'/src/')||!(/\.[jt]sx?$/.test(file.fileName)))continue;
        function visit(node){
          if(ts.isImportDeclaration(node)&&ts.isStringLiteral(node.moduleSpecifier)&&!node.importClause?.isTypeOnly){
            const binding=node.importClause?.namedBindings,allTypes=binding&&ts.isNamedImports(binding)&&binding.elements.length&&binding.elements.every(item=>item.isTypeOnly)&&!node.importClause.name;
            if(!allTypes)importSource(node.moduleSpecifier);
          }else if(ts.isExportDeclaration(node)&&node.moduleSpecifier&&ts.isStringLiteral(node.moduleSpecifier)&&!node.isTypeOnly){
            const allTypes=node.exportClause&&ts.isNamedExports(node.exportClause)&&node.exportClause.elements.length&&node.exportClause.elements.every(item=>item.isTypeOnly);
            if(!allTypes)importSource(node.moduleSpecifier);
          }else if(ts.isCallExpression(node)&&node.expression.kind===ts.SyntaxKind.ImportKeyword){
            if(node.arguments.length===1&&ts.isStringLiteral(node.arguments[0]))importSource(node.arguments[0]);else open.push({path:file.fileName,start:node.getStart(),reason:'dynamic runtime module import is unresolved'});
          }else if(ts.isImportEqualsDeclaration(node)&&!node.isTypeOnly)open.push({path:file.fileName,start:node.getStart(),reason:'CommonJS runtime import is outside the source enrollment profile'});
          ts.forEachChild(node,visit);
        }
        visit(file);
      }
      runtimeSources.sort((a,b)=>a.path.localeCompare(b.path));
      if(runtimeSources.length){const sourceOptions={...publicState.program.getCompilerOptions(),allowJs:true,checkJs:false,noEmit:true},host=ts.createCompilerHost(sourceOptions);
        Object.assign(host,{readFile:system.readFile,fileExists:system.fileExists,directoryExists:system.directoryExists,realpath:system.realpath,getDirectories:system.getDirectories,
          getSourceFile(path,language,onError){try{const text=system.readFile(path);return text===undefined?undefined:ts.createSourceFile(path,text,language,true);}catch(error){onError?.(error.message);return undefined;}}});
        sourceProgram=ts.createProgram([...publicState.program.getRootFileNames(),...runtimeSources.map(row=>row.path)],sourceOptions,host);stats.sourcePrograms++;
        sourceErrors=ts.getPreEmitDiagnostics(sourceProgram).filter(error=>error.category===ts.DiagnosticCategory.Error);
        if(sourceErrors.length&&!publicState.errors.length){open.push({reason:'additional runtime source facts have diagnostics; published typing results are unchanged',codes:sourceErrors.map(d=>d.code)});sourceProgram=publicState.program;runtimeSources.length=0;}
      }
    }catch(error){open.push({reason:error.message});sourceProgram=publicState.program;runtimeSources.length=0;}
    const manifest=[...inputs.values()].sort((a,b)=>JSON.stringify(a).localeCompare(JSON.stringify(b)));assert(validateProjectInputs(manifest).valid,'Source inputs changed during construction');assert(published.acceptRevision(publicState.revision).valid,'Published typing revision retired during source construction');
    const revision=Object.freeze({sessionId,generation:++generation,invalidation,inputSha256:hash(JSON.stringify(manifest))});issued.clear();issued.set(JSON.stringify(revision),{program:sourceProgram,publishedRevision:publicState.revision,inputs:manifest});
    const buildMs=performance.now()-started;stats.builds++;stats.buildMs+=buildMs;stats.scannedFiles+=scannedFiles;stats.scannedBytes+=scannedBytes;
    return {program:sourceProgram,publishedProgram:publicState.program,publishedRevision:publicState.revision,errors:publicState.errors,sourceErrors,runtimeSources,open,inputs:manifest,revision,buildMs,generation};
  }
  function state(path,code,external){
    path=resolve(path);assert.equal(readFileSync(path,'utf8'),code,'Served source differs from installed bytes');
    if(!external){assert(path.startsWith(root+'/'),'Consumer is outside the package source session');anchor={path,code};}else assert(anchor,'Runtime source requires an initialized published project');
    const publicState=published.get(anchor.path,external?readFileSync(anchor.path,'utf8'):anchor.code),started=performance.now();
    const reuse=!!cached&&JSON.stringify(cached.publishedRevision)===JSON.stringify(publicState.revision)&&validateProjectInputs(cached.inputs).valid;
    const validationMs=performance.now()-started;stats.validationMs+=validationMs;if(reuse)stats.reuses++;else cached=build(publicState);
    if(external){const actual=realpathSync(path);assert(cached.runtimeSources.some(row=>row.path===actual),'Runtime source is outside the recorded enrollment');path=actual;}
    const source=cached.program.getSourceFile(path);assert(source,'Served source is absent from the recorded program');assert.equal(source.text,code,'Source facts differ from served bytes');
    return {...cached,source,reused:reuse,validationMs,buildMs:reuse?0:cached.buildMs,inputCount:cached.inputs.length};
  }
  return {stats,get(path,code){return state(path,code,false);},getExternal(path,code){return state(path,code,true);},
    acceptRevision(revision){if(revision?.sessionId===sessionId&&(revision.generation!==generation||revision.invalidation!==invalidation))return {valid:false,reason:'observation belongs to a retired package source revision'};const entry=issued.get(JSON.stringify(revision));if(!entry)return {valid:false,reason:'observation has no issued package source revision'};if(entry.program!==cached?.program||!published.acceptRevision(entry.publishedRevision).valid)return {valid:false,reason:'published typing revision retired'};return validateProjectInputs(entry.inputs);},
    invalidate(){invalidation++;stats.invalidations++;cached=null;issued.clear();published.invalidate();},inputs(){return cached?structuredClone(cached.inputs):[];},runtimeSources(){return cached?structuredClone(cached.runtimeSources):[];},sourceOpen(){return cached?structuredClone(cached.open):[];}};
}
