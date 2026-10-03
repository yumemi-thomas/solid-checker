// Independently reconstruct published typings and every reported declaration owner.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {hash,read} from './catalog.mjs';
import {ts} from './lower.mjs';
const [inputArg,outArg]=process.argv.slice(2),input=resolve(inputArg),out=resolve(outArg);assert(!existsSync(out));const observed=read(input);assert(observed.finishedAt);
for(const pin of observed.inputs)assert.equal(hash(readFileSync(pin.path)),pin.sha256);
const sealPin=observed.inputs.find(pin=>pin.path.includes('freeze')),seal=read(sealPin.path);for(const pin of [...seal.files,seal.baseline])assert.equal(hash(readFileSync(pin.path)),pin.sha256);
const rows=[];
for(const row of observed.results){
  if(row.refused){assert.equal(row.app,'jandibat-web');assert.match(row.refused.message,/Installed package missing: @solidjs\/web/);continue;}
  await(async()=>{
    const config=ts.readConfigFile(row.configPath,ts.sys.readFile);assert(!config.error);const parsed=ts.parseJsonConfigFileContent(config.config,ts.sys,row.root,{},row.configPath),program=ts.createProgram(parsed.fileNames,parsed.options),checker=program.getTypeChecker();
    const errors=ts.getPreEmitDiagnostics(program).filter(d=>d.category===ts.DiagnosticCategory.Error).map(d=>({code:d.code,message:ts.flattenDiagnosticMessageText(d.messageText,'\n')}));assert.deepEqual(errors,row.publishedTypingErrors);
    assert.equal(hash(JSON.stringify(row.inputManifest)),row.revision.inputSha256);
    const target=symbol=>symbol?.flags&ts.SymbolFlags.Alias?checker.getAliasedSymbol(symbol):symbol;
    function unwrap(node){while(ts.isParenthesizedExpression(node)||ts.isAsExpression(node)||ts.isNonNullExpression(node)||ts.isSatisfiesExpression(node)||ts.isTypeAssertionExpression(node))node=node.expression;return node;}
    function exact(source,start,end,predicate){let found;function visit(node){if(node.getStart(source)===start&&node.end===end&&predicate(node))found=node;ts.forEachChild(node,visit);}visit(source);return found;}
    let calls=0,jsx=0,files=0,otherCalls=0,otherJSX=0,outsideTypes=0,overlap=0;
    for(const file of row.files){const source=program.getSourceFile(file.path);assert(source&&hash(source.text)===file.sourceSha256);assert.equal(hash(readFileSync(file.path)),file.sourceSha256);files++;
      for(const reference of file.externalReferences){
        const node=exact(source,reference.start,reference.end,node=>reference.kind==='call'?ts.isCallExpression(node):ts.isJsxOpeningElement(node)||ts.isJsxSelfClosingElement(node));assert(node);
        const callee=unwrap(reference.kind==='call'?node.expression:node.tagName);assert.equal(callee.getStart(source),reference.calleeStart);assert.equal(callee.end,reference.calleeEnd);
        const location=ts.isIdentifier(callee)?callee:ts.isPropertyAccessExpression(callee)?callee.name:null,symbol=location&&target(checker.getSymbolAtLocation(location));assert(symbol?.declarations?.length);
        assert.deepEqual(reference.declarations,symbol.declarations.map(declaration=>({path:declaration.getSourceFile().fileName,start:declaration.getStart(),end:declaration.end,sha256:hash(declaration.getSourceFile().text)})));
        assert.equal(reference.name,symbol.getName());assert.equal(reference.dispatch,'open');assert.equal(reference.runtimeBinding,'unresolved-by-this-study');assert.equal(reference.authority,false);assert.equal(reference.certification,false);
        for(const owner of reference.declarationOwners){assert.equal(hash(readFileSync(owner.metadata)),owner.metadataSha256);const pkg=read(owner.metadata);assert.equal(pkg.name,owner.package);assert.equal(pkg.version,owner.version);assert.equal(owner.metadata,join(dirname(owner.metadata),'package.json'));assert(reference.declarations.some(declaration=>declaration.path.startsWith(dirname(owner.metadata)+'/')));assert(!row.root.startsWith(owner.root+'/')&&owner.root!==row.root);}
        if(reference.kind==='call')calls++;else jsx++;
        if(file.sourceRole==='other-configured-source'){if(reference.kind==='call'){otherCalls++;if(reference.declarationOwners.some(owner=>!owner.package.startsWith('@types/')&&owner.package!=='bun-types'))outsideTypes++;}else otherJSX++;}
      }
      for(const candidate of file.candidates)if(file.externalReferences.some(reference=>reference.kind==='call'&&reference.start===candidate.start&&reference.end===candidate.end))overlap++;
      if(errors.length){assert.equal(file.admission,'closed-public-typing-errors');assert.deepEqual(file.candidates,[]);assert.equal(file.asyncFunctions,0);assert.equal(file.callbackEntries,0);}
    }
    assert.equal(files,row.summary.files);assert.equal(calls,row.summary.externalCalls);assert.equal(jsx,row.summary.externalJSX);assert.equal(otherCalls,row.referenceScope.otherConfiguredCallReferences);assert.equal(otherJSX,row.referenceScope.otherConfiguredJSXReferences);assert.equal(outsideTypes,row.referenceScope.otherConfiguredCallReferencesOutsideTypesPackages);assert.equal(overlap,row.referenceScope.candidateSitesAtExactExternalCall);
    for(const item of row.ownerMetadata)assert.equal(hash(readFileSync(item.metadata)),item.metadataSha256);
    rows.push({app:row.app,files,calls,jsx,typingErrors:errors.length,otherConfiguredCallReferences:otherCalls,otherConfiguredJSXReferences:otherJSX,otherConfiguredCallReferencesOutsideTypesPackages:outsideTypes,candidateSitesAtExactExternalCall:overlap});
  })();await new Promise(resolve=>setImmediate(resolve));globalThis.gc?.();
}
writeFileSync(out,JSON.stringify({authority:false,certification:false,finishedAt:new Date().toISOString(),scope:'published typing reconstruction, source/declaration/metadata identities and reported reference totals; no runtime provider binding, candidate-selection completeness, implementation-discovery completeness or real-app defect verdict',validator:{path:new URL(import.meta.url).pathname,sha256:hash(readFileSync(new URL(import.meta.url)))},inputs:[{path:input,sha256:hash(readFileSync(input))}],results:rows},null,2)+'\n');console.log(JSON.stringify({projects:rows.length,files:rows.reduce((sum,row)=>sum+row.files,0),calls:rows.reduce((sum,row)=>sum+row.calls,0),jsx:rows.reduce((sum,row)=>sum+row.jsx,0)}));
