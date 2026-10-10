// A retained observation can support a conditional note; discarded history cannot
// support a claim about absent reads or complete package coverage.
import {nativeReadFeedback as previousFeedback} from './native-read-feedback-v18.mjs';
export function nativeReadFeedback(session,path,code,events,stats){
  let retention;
  try{
    if(stats?.observationsComplete!==false||!stats.retention)throw Error('bounded runtime retention ledger is missing');
    retention=stats.retention;
    for(const name of ['events','metadata','gaps','guards']){
      const row=retention[name];
      if(!row||!['records','bytes','maxRecords','maxBytes','evicted','refused'].every(key=>Number.isSafeInteger(row[key])&&row[key]>=0)
        ||row.maxRecords<1||row.maxBytes<1||row.records>row.maxRecords||row.bytes>row.maxBytes)throw Error(`invalid ${name} retention ledger`);
    }
    if(retention.events.records!==stats.eventRecords||retention.events.records!==stats.seenRecords||retention.events.records<events.length
      ||retention.metadata.records!==stats.metadataRecords||retention.gaps.records!==stats.gapRecords||retention.guards.records!==stats.guardRecords)throw Error('retention counts differ from the runtime snapshot');
  }catch(error){
    return {revision:session.get(path,code).revision,notes:[],suppressed:[],acceptedEvents:0,
      observationCoverage:{complete:false,scope:'bounded runtime history',status:'unavailable'},
      open:[{reason:error.message,authority:false,certification:false}]};
  }
  const result=previousFeedback(session,path,code,events),open=[...result.open];
  const losses=['events','gaps'].map(kind=>[kind,retention[kind]]).filter(([,row])=>row.evicted||row.refused).map(([kind,row])=>({kind,evicted:row.evicted,refused:row.refused}));
  if(losses.length)open.push({reason:'bounded runtime history discarded records; absent observations remain uncertifiable',losses,authority:false,certification:false});
  return {...result,open,observationCoverage:{complete:false,scope:'bounded runtime history',status:losses.length?'partial':'retained',retention,cacheOnly:['metadata','guards']}};
}
