// Native reads and executed package shortcuts remain distinct observations.
import {readFileSync} from 'node:fs';
import {dirname,join} from 'node:path';
import {hash,packageRoot} from './catalog.mjs';
import {ts} from './lower.mjs';
import {nativeReadSites} from './native-read-sites-v2.mjs';
import {nativeReadFeedback as nativeFeedback} from './native-read-feedback-v2.mjs';
import {nativeReaderPremise} from './native-read-hook-v3.mjs';
import {packageShortcuts} from './package-shortcut-v1.mjs';
export function nativeReadFeedback(program,source,events){
  const native=nativeFeedback(program,source,events.filter(event=>event.identity?.kind!=='package-observer-guard'));
  if(ts.getPreEmitDiagnostics(program).some(d=>d.category===ts.DiagnosticCategory.Error))return native;
  const admitted=nativeReadSites(program,source),cache=new Map(),seen=new Set(native.notes.map(note=>note.witness.start+':'+note.witness.end));
  for(const event of events.filter(event=>event.identity?.kind==='package-observer-guard')){
    const site=event.site,current=admitted.sites.find(item=>item.start===site?.start&&item.end===site?.end),premise=event.identity.premise;
    try{
      if(!current||JSON.stringify(current)!==JSON.stringify(site))throw Error('Package shortcut lacks the exact current consumer witness');
      if(!cache.has(premise.path)){const text=readFileSync(premise.path,'utf8'),parsed=packageShortcuts(text,premise.path);cache.set(premise.path,{text,...parsed});}
      const parsed=cache.get(premise.path),model=parsed.models.find(model=>JSON.stringify(model)===JSON.stringify(premise));if(!model)throw Error('Changed package shortcut premise');
      for(const declaration of [...model.observerDeclarations,...model.ownerDeclarations,...model.signalDeclarations.flat()])if(hash(readFileSync(declaration.path))!==declaration.sha256)throw Error('Changed package shortcut declaration');
      const shared=join(packageRoot(dirname(premise.path),'@solidjs/signals'),'dist/dev-shared.js');nativeReaderPremise(readFileSync(shared,'utf8'),shared);
      if(!event.nativeRead?.originalFrames?.some(frame=>{
        if(frame?.path!==premise.path||frame.sourceSha256!==hash(parsed.text))return false;
        const offset=parsed.source.getPositionOfLineAndCharacter(frame.line-1,frame.column-1);return offset>=model.returned.start&&offset<model.returned.end;
      }))throw Error('Missing mapped package shortcut return frame');
      if(!event.originalFrames?.some(frame=>{
        if(frame?.path!==source.fileName||frame.sourceSha256!==hash(source.text))return false;
        const offset=source.getPositionOfLineAndCharacter(frame.line-1,frame.column-1);return offset>=site.start&&offset<site.end;
      }))throw Error('Missing mapped original consumer expression');
      if(event.context?.owner!==false||event.context?.observer!==false)continue;
      const key=site.start+':'+site.end;if(seen.has(key))continue;seen.add(key);
      native.notes.push({code:'OBSERVED_MEMO_CALLBACK_PACKAGE_SHORTCUT',severity:'info',category:'intent-open',channel:'observed-package-observer-shortcut',authority:false,certification:false,staticDispatch:'open',
        message:'This package took its no-observer return path during the memo callback. If this lookup should drive updates, read it during the memo compute and pass the captured value into the callback.',
        basis:'observed successful package return guarded by the exact core observer import, before a source-enrolled signal path, during the original consumer expression',
        location:{path:source.fileName,line:site.line,column:site.column,startByte:Buffer.byteLength(source.text.slice(0,site.start))},witness:site,guardShortcut:model,observedContext:event.context,occurrences:event.occurrences,
        limitation:'A native reactive read, counterfactual signal execution, callback timing, returned value flow and developer intent are not proved by this shortcut observation.'});
    }catch(error){native.open.push({reason:error.message});}
  }return native;
}
