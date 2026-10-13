// Current-source observations with lexical attribution inside native async helpers.
import assert from 'node:assert/strict';
import {readFileSync,realpathSync} from 'node:fs';
import {dirname,join} from 'node:path';
import {hash,nativeRuntimeRoots,packageRoot,read} from './catalog.mjs';
import {validateProjectInputs} from './project-read-session-v2.mjs';
import {packageSourceSession} from './package-source-session-v2.mjs';
import {instrumentNativeReads} from './native-read-hook-v3.mjs';
import {packageShortcutHook} from './package-shortcut-v2.mjs';
import {transformAsyncContinuations} from './async-continuation-transform-v5.mjs';
export default function projectReadPlugin(){const transformed=[],refused=[],native={transformed:[],refused:[]},shortcutHook=packageShortcutHook(),cacheInvalidations=[],unchangedUpdates={events:0,files:[],unlistedEvents:0};let project,session;
  const nativeHook={...native,transform(code,path){
    if(!path.endsWith('/dist/dev.js')&&!path.endsWith('/dist/dev-shared.js'))return null;
    let root;try{root=packageRoot(dirname(path),'@solidjs/signals');}catch{return null;}
    if(realpathSync(path)!==realpathSync(join(root,'dist',path.endsWith('/dev-shared.js')?'dev-shared.js':'dev.js')))return null;
    try{assert.equal(read(join(root,'package.json')).version,'2.0.0-rc.9');assert.equal(readFileSync(path,'utf8'),code);
      const result=instrumentNativeReads(code,path,{runtimeSpecifier:'/@fs'+new URL('./native-read-runtime-v8.mjs',import.meta.url).pathname});native.transformed.push(result.premise);if(result.store)native.transformed.push(result.store);return {code:result.code,map:result.map};
    }catch(error){native.refused.push({path,reason:error.message});return null;}
  }};
  return {name:'observed-installed-package-async-helper-reads',enforce:'pre',transformed,refused,nativeHook,shortcutHook,cacheInvalidations,unchangedUpdates,get session(){return session;},configResolved(config){project=config.root;session=packageSourceSession(project);},
    handleHotUpdate(context){const inputs=session.inputs(),validation=validateProjectInputs(inputs);if(validation.valid){unchangedUpdates.events++;let file=unchangedUpdates.files.find(row=>row.file===context.file);if(file){file.events++;file.lastTimestamp=context.timestamp;}else if(unchangedUpdates.files.length<64)unchangedUpdates.files.push({file:context.file,events:1,firstTimestamp:context.timestamp,lastTimestamp:context.timestamp});else unchangedUpdates.unlistedEvents++;return;}session.invalidate();const seen=new Set(),consumers=[];for(const path of new Set(transformed.map(row=>row.path)))for(const module of context.server.moduleGraph.getModulesByFile(path)??[]){context.server.moduleGraph.invalidateModule(module,seen,context.timestamp,true,false);consumers.push(path);}cacheInvalidations.push({file:context.file,timestamp:context.timestamp,inputChange:validation,consumers:[...new Set(consumers)]});},
    transform(code,id){const path=id.split('?')[0],core=nativeHook.transform(code,path);if(core)return core;const shortcut=shortcutHook.transform(code,path);if(shortcut)return shortcut;
      const external=session.runtimeSources().some(row=>row.path===path);
      if(!external&&(!path.startsWith(project+'/src/')||!(/\.[jt]sx?$/.test(path))))return null;
      try{assert.equal(readFileSync(path,'utf8'),code,'Another transform changed the original consumer before observation');for(const root of nativeRuntimeRoots(dirname(path)))assert.equal(read(join(root,'package.json')).version,'2.0.0-rc.9');
        const facts=external?{get:(path,code)=>session.getExternal(path,code)}:session;
        const result=transformAsyncContinuations(code,path,facts);transformed.push({path,sourceSha256:hash(code),transformedSha256:hash(result.code),sites:result.sites,helpers:result.helpers,callbackSlots:result.callbackSlots,open:result.open,session:result.session});return {code:result.code,map:result.map};
      }catch(error){refused.push({path,reason:error.message});return null;}}
  };
}
