// Independent audit: no selector, transform or hint projector is imported.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {hash,read} from './catalog.mjs';
import {ts} from './lower.mjs';
import {oracleCompilerOptions} from '../../scripts/tsc-oracle.mjs';
const [populationArg,browserArg,studyArg,outArg]=process.argv.slice(2),inputs=[populationArg,browserArg,studyArg].map(resolve),output=resolve(outArg);
assert(!existsSync(output));const [population,browser,study]=inputs.map(read),witnesses=[];
for(const pin of [...population.files,...population.declarations,...study.inputs])assert.equal(hash(readFileSync(pin.path)),pin.sha256);
for(const row of browser.results){
  const evaluated=study.results.find(item=>item.id===row.id);if(!evaluated.hints.length)continue;
  const path=join(dirname(inputs[1]),row.id,'src/main.tsx'),text=readFileSync(path,'utf8');
  const options=ts.convertCompilerOptionsFromJson({...oracleCompilerOptions('v2',true,{customConditions:['browser','development']}),allowJs:true},dirname(path)).options;
  const program=ts.createProgram([path],options),source=program.getSourceFile(path),checker=program.getTypeChecker();
  assert.equal(ts.getPreEmitDiagnostics(program).filter(d=>d.category===ts.DiagnosticCategory.Error).length,0);
  const calls=[],functions=[];function visit(node){if(ts.isCallExpression(node))calls.push(node);if(ts.isFunctionLike(node))functions.push(node);ts.forEachChild(node,visit);}visit(source);
  const target=symbol=>symbol?.flags&ts.SymbolFlags.Alias?checker.getAliasedSymbol(symbol):symbol;
  for(const note of evaluated.hints){
    assert.equal(note.severity,'info');assert.equal(note.category,'intent-open');assert.equal(note.staticDispatch,'open');assert.equal(note.certification,false);
    const site=note.witness,event=row.identityTrace.find(event=>JSON.stringify(event.site)===JSON.stringify(site));assert(event);
    assert.deepEqual(event.context,{owner:false,observer:false});const call=calls.find(node=>node.getStart(source)===site.start&&node.end===site.end);
    assert(call&&ts.isIdentifier(call.expression)&&call.arguments.length===0&&!call.questionDotToken);
    const symbol=target(checker.getSymbolAtLocation(call.expression));assert(symbol?.declarations?.length);
    assert.deepEqual(symbol.declarations.map(d=>({path:d.getSourceFile().fileName,start:d.getStart(),end:d.end,sha256:hash(d.getSourceFile().text)})),site.calleeDeclarations);
    const callback=functions.find(node=>node.getStart(source)===site.callback.start&&node.end===site.callback.end),memo=calls.find(node=>node.getStart(source)===site.memo.start&&node.end===site.memo.end);
    assert(callback&&memo);assert(memo.arguments[0]);
    assert(call.getStart()>=callback.getStart()&&call.end<=callback.end);assert(callback.getStart()>memo.getStart()&&callback.end<memo.end);
    assert(site.memoDeclarations.length>0);for(const declaration of site.memoDeclarations)assert.equal(hash(readFileSync(declaration.path)),declaration.sha256);
    // Inspect the exact native artifact independently, including its bound read.
    const premise=event.identity.premise,nativeText=readFileSync(premise.path,'utf8');
    assert.equal(hash(nativeText),'sha256:f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120');
    const native=ts.createSourceFile(premise.path,nativeText,ts.ScriptTarget.Latest,true,ts.ScriptKind.JS),nodes=[];
    function nativeVisit(node){nodes.push(node);ts.forEachChild(node,nativeVisit);}nativeVisit(native);
    const accessor=nodes.find(node=>node.getStart(native)===premise.accessor.start&&node.end===premise.accessor.end),binding=nodes.find(node=>node.getStart(native)===premise.binding.start&&node.end===premise.binding.end),returned=nodes.find(node=>node.getStart(native)===premise.returned.start&&node.end===premise.returned.end);
    assert(ts.isFunctionDeclaration(accessor)&&accessor.parameters.length===1);assert(ts.isVariableDeclaration(binding)&&ts.isIdentifier(returned));
    assert.equal(returned.text,binding.name.text);assert(ts.isCallExpression(binding.initializer));assert.equal(binding.initializer.expression.name.text,'bind');
    assert.equal(binding.initializer.arguments[0].kind,ts.SyntaxKind.NullKeyword);assert.equal(binding.initializer.arguments[1].text,accessor.parameters[0].name.text);
    assert(event.identity.originalCreationFrames.some(frame=>{
      if(frame?.path!==premise.path||frame.sourceSha256!==hash(nativeText))return false;
      const offset=native.getPositionOfLineAndCharacter(frame.line-1,frame.column-1);return offset>=returned.getStart()&&offset<returned.end;
    }));
    assert(event.originalFrames.some(frame=>{
      if(frame?.path!==path||frame.sourceSha256!==hash(text))return false;
      const offset=source.getPositionOfLineAndCharacter(frame.line-1,frame.column-1);return offset>=call.getStart()&&offset<call.end;
    }));
    witnesses.push({id:row.id,read:{path,start:call.getStart(),end:call.end,sha256:hash(text)},nativeCreation:{path:premise.path,start:returned.getStart(),end:returned.end,sha256:hash(nativeText)},occurrences:event.occurrences});
  }
}
const report={authority:false,certification:false,finishedAt:new Date().toISOString(),inputs:inputs.map(path=>({path,sha256:hash(readFileSync(path))})),witnesses,
  limit:'Audit checks observed native creation and consumer provenance. Runtime trace integrity is research evidence, not an adversarial root of trust; callback intent stays open.'};
writeFileSync(output,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({witnesses:witnesses.length}));
