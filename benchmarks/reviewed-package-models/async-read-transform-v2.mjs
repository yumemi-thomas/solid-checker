import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {dirname,join} from 'node:path';
import {ts} from './lower.mjs';
import {asyncReadSitesV2} from './async-read-sites-v2.mjs';
import {hash,nativeRuntimeRoots,read} from './catalog.mjs';
import {oracleCompilerOptions} from '../../scripts/tsc-oracle.mjs';
export function readProgram(code,path){
  const options=ts.convertCompilerOptionsFromJson({...oracleCompilerOptions('v2',true,{customConditions:['browser','development']}),allowJs:true},dirname(path)).options;
  const host=ts.createCompilerHost(options),get=host.getSourceFile,exists=host.fileExists;
  host.getSourceFile=(file,...args)=>file===path?ts.createSourceFile(path,code,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX):get(file,...args);
  host.fileExists=file=>file===path||exists(file);return ts.createProgram([path],options,host);
}
export function transformAsyncReadsV2(code,path){
  for(const root of nativeRuntimeRoots(dirname(path)))assert.equal(read(join(root,'package.json')).version,'2.0.0-rc.9');
  const program=readProgram(code,path),source=program.getSourceFile(path);
  const errors=ts.getPreEmitDiagnostics(program).filter(d=>d.category===ts.DiagnosticCategory.Error);
  if(errors.length)return {code,map:null,sites:[],open:[{reason:'TypeScript owns this input',codes:errors.map(d=>d.code)}]};
  const {sites,open}=asyncReadSitesV2(program,source);if(!sites.length)return {code,map:null,sites,open};
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
      ];
      return f.updateSourceFile(next,[...imports,...next.statements]);
    }]}});
  return {code:emitted.outputText,map:JSON.parse(emitted.sourceMapText),sites,open};
}
export default function asyncReadPlugin(){const transformed=[],refused=[];let project;
  return {name:'observed-native-accessor-reads',enforce:'pre',transformed,refused,configResolved(config){project=config.root;},
    transform(code,id){const path=id.split('?')[0];if(!path.startsWith(project+'/src/')||!path.endsWith('.tsx'))return null;
      try{if(readFileSync(path,'utf8')!==code)return null;const result=transformAsyncReadsV2(code,path);
        transformed.push({path,sourceSha256:hash(code),transformedSha256:hash(result.code),sites:result.sites,open:result.open});
        return {code:result.code,map:result.map};
      }catch(error){refused.push({path,reason:error.message});return null;}}
  };
}
