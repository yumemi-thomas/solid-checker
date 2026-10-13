// Native identity observations establish a read, while reactive intent stays open.
import {readFileSync} from 'node:fs';
import {join,dirname} from 'node:path';
import {nativeStorePremise} from './native-store-premise-v1.mjs';
import {nativeReaderPremise} from './native-read-hook-v2.mjs';
import {hash} from './catalog.mjs';
import {ts} from './lower.mjs';
import {nativeReadSites} from './native-read-sites-v3.mjs';
import {nativeAccessorPremise} from './native-accessor-hook-v1.mjs';
export function nativeReadFeedback(program,source,events){
  const notes=[],open=[],seen=new Set(),premises=new Map();
  if(ts.getPreEmitDiagnostics(program).some(d=>d.category===ts.DiagnosticCategory.Error))
    return {notes,open:[{reason:'TypeScript owns this input'}]};
  const admitted=nativeReadSites(program,source);open.push(...admitted.open);
  for(const event of events){
    const site=event.site,current=admitted.sites.find(item=>item.start===site?.start&&item.end===site?.end);
    if(!current||JSON.stringify(current)!==JSON.stringify(site)){
      open.push({reason:'native read lacks the exact current consumer and memo witness'});continue;
    }
    const premise=event.identity?.premise,store=event.identity?.kind==='native-store-target',cacheKey=event.identity?.kind+':'+premise?.path;let nativeSource;
    try{
      if(!premises.has(cacheKey)){
        const text=readFileSync(premise.path,'utf8');
        premises.set(cacheKey,{text,current:store?nativeStorePremise(text,premise.path):nativeAccessorPremise(text,premise.path),
          source:ts.createSourceFile(premise.path,text,ts.ScriptTarget.Latest,true,ts.ScriptKind.JS)});
      }
      nativeSource=premises.get(cacheKey);
      if(!['native-store-target','native-node'].includes(event.identity?.kind))throw Error('unknown native identity kind');
      if(store&&(typeof event.storeKey!=='string'||event.storeKey==='then'))throw Error('store key is outside the data-read profile');
      if(JSON.stringify(nativeSource.current)!==JSON.stringify(premise))throw Error('changed native identity premise');
      const witnessed=event.identity.originalCreationFrames?.some(frame=>{
        if(frame?.path!==premise.path||frame.sourceSha256!==hash(nativeSource.text))return false;
        const offset=nativeSource.source.getPositionOfLineAndCharacter(frame.line-1,frame.column-1);
        return offset>=premise.returned.start&&offset<premise.returned.end;
      });
      if(!witnessed)throw Error('missing mapped native accessor creation frame');
      const reader=event.nativeRead?.premise,readerPath=store?premise.path:join(dirname(premise.path),'dev-shared.js');
      if(reader?.path!==readerPath)throw Error('native reader is not the accessor artifact provider');
      const readerText=readFileSync(readerPath,'utf8'),currentReader=store?nativeStorePremise(readerText,readerPath):nativeReaderPremise(readerText,readerPath),readerSource=ts.createSourceFile(readerPath,readerText,ts.ScriptTarget.Latest,true,ts.ScriptKind.JS);
      if(JSON.stringify(currentReader)!==JSON.stringify(reader))throw Error('changed native reader premise');
      if(!event.nativeRead.originalFrames?.some(frame=>{
        if(frame?.path!==readerPath||frame.sourceSha256!==hash(readerText))return false;
        const offset=readerSource.getPositionOfLineAndCharacter(frame.line-1,frame.column-1);return offset>=reader.entry.start&&offset<reader.entry.end;
      }))throw Error('missing mapped native reader entry frame');
      if(!event.originalFrames?.some(frame=>{
        if(frame?.path!==source.fileName||frame.sourceSha256!==hash(source.text))return false;
        const offset=source.getPositionOfLineAndCharacter(frame.line-1,frame.column-1);
        return offset>=site.start&&offset<site.end;
      }))throw Error('missing mapped original read frame');
    }catch(error){open.push({reason:error.message});continue;}
    if(event.context?.observer!==false||event.context?.owner!==false)continue;
    const key=site.start+':'+site.end;if(seen.has(key))continue;seen.add(key);
    notes.push({code:'OBSERVED_MEMO_CALLBACK_UNTRACKED_READ',severity:'info',category:'intent-open',
      channel:'observed-native-read-in-expression',authority:false,certification:false,staticDispatch:'open',
      message:'This expression made a reactive read without registering a dependency for the memo. If that source should drive the result, read it during the memo compute and pass the captured value into the callback.',
      basis:'observed successful native accessor or store data read during the original expression, with neither owner nor observer',
      location:{path:source.fileName,line:site.line,column:site.column,startByte:Buffer.byteLength(source.text.slice(0,site.start))},
      witness:site,nativeIdentity:event.identity,nativeRead:event.nativeRead,storeKey:event.storeKey??null,observedContext:event.context,occurrences:event.occurrences,
      limitation:'Package callback timing, result flow and intent remain open; deliberate snapshots are possible.'});
  }
  return {notes,open};
}
