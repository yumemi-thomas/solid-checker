// V2 selection with the real project configuration and a shared TypeScript session.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {dirname,join} from 'node:path';
import {ts} from './lower.mjs';
import {asyncReadSitesV2} from './async-read-sites-v2.mjs';
import {hash,nativeRuntimeRoots,read} from './catalog.mjs';
import {projectReadSession} from './project-read-session-v1.mjs';

export function transformProjectReads(code,path,session){
  for(const root of nativeRuntimeRoots(dirname(path)))assert.equal(read(join(root,'package.json')).version,'2.0.0-rc.9');
  const state=session.get(path,code),{program,source,errors}=state;
  const context={generation:state.generation,reused:state.reused,validationMs:state.validationMs,buildMs:state.buildMs,inputCount:state.inputCount};
  if(errors.length)return {code,map:null,sites:[],open:[{reason:'TypeScript owns this input',codes:errors.map(error=>error.code)}],session:context};
  const {sites,open}=asyncReadSitesV2(program,source);
  if(!sites.length)return {code,map:null,sites,open,session:context};
  const names=new Set();function namesIn(node){if(ts.isIdentifier(node))names.add(node.text);ts.forEachChild(node,namesIn);}namesIn(source);
  let prefix='__packageReadObservation';while([...names].some(name=>name.startsWith(prefix)))prefix+='_';
  const f=ts.factory,id=suffix=>f.createIdentifier(prefix+suffix),selected=new Map(sites.map(site=>[site.start+':'+site.end,site]));
  const emitted=ts.transpileModule(code,{fileName:path,compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext,jsx:ts.JsxEmit.Preserve,sourceMap:true,inlineSources:true},
    transformers:{before:[context=>root=>{
      function visit(node){const site=selected.get(node.getStart(root)+':'+node.end);
        if(site&&ts.isCallExpression(node)){
          const next=f.createCallExpression(id('Read'),undefined,[node,f.createStringLiteral(JSON.stringify(site)),id('Observer'),id('Owner')]);
          ts.setOriginalNode(next,node);return ts.setTextRange(next,node);
        }
        return ts.visitEachChild(node,visit,context);
      }
      const next=ts.visitNode(root,visit),imports=[
        f.createImportDeclaration(undefined,f.createImportClause(false,undefined,f.createNamedImports([
          f.createImportSpecifier(false,f.createIdentifier('getObserver'),id('Observer')),f.createImportSpecifier(false,f.createIdentifier('getOwner'),id('Owner')),
        ])),f.createStringLiteral('solid-js')),
        f.createImportDeclaration(undefined,f.createImportClause(false,undefined,f.createNamedImports([
          f.createImportSpecifier(false,f.createIdentifier('observedRead'),id('Read')),
        ])),f.createStringLiteral('/@fs'+new URL('./async-read-runtime-v1.mjs',import.meta.url).pathname)),
      ];return f.updateSourceFile(next,[...imports,...next.statements]);
    }]}});
  return {code:emitted.outputText,map:JSON.parse(emitted.sourceMapText),sites,open,session:context};
}

export default function projectReadPlugin(){const transformed=[],refused=[];let project,session;
  return {name:'observed-project-native-reads',enforce:'pre',transformed,refused,get session(){return session;},configResolved(config){project=config.root;session=projectReadSession(project);},
    transform(code,id){const path=id.split('?')[0];if(!path.startsWith(project+'/src/')||!path.endsWith('.tsx'))return null;
      try{assert.equal(readFileSync(path,'utf8'),code,'Another transform changed the original consumer before observation');
        const result=transformProjectReads(code,path,session);
        transformed.push({path,sourceSha256:hash(code),transformedSha256:hash(result.code),sites:result.sites,open:result.open,session:result.session});
        return {code:result.code,map:result.map};
      }catch(error){refused.push({path,reason:error.message});return null;}}
  };
}
