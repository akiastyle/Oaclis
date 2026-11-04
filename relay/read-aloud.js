// read-aloud.js v1.22.278
'use strict';
window.OaclisReadAloud=(()=>{
 function spokenText(raw){
  let fence=null;const lines=[];
  for(const line of raw.split('\n')){
   const marker=line.match(/^\s{0,3}(`{3,}|~{3,})/);
   if(marker){if(!fence)fence=marker[1][0];else if(marker[1][0]===fence)fence=null;continue;}
   if(fence||/^(?: {4}|\t)/.test(line))continue;
   let text=line.replace(/(`+)([\s\S]*?)\1/g,'');
   const unmatched=text.indexOf('`');if(unmatched>=0)text=text.slice(0,unmatched);
   text=text.replace(/!\[([^\]]*)\]\([^)]*\)/g,'$1').replace(/\[([^\]]+)\]\([^)]*\)/g,'$1')
    .replace(/https?:\/\/\S+/g,'').replace(/<[^>]*>/g,'').replace(/^\s*(?:#{1,6}\s+|>\s*|[-*+]\s+|\d+[.)]\s+)/,'')
    .replace(/[*_~]/g,'').replace(/[\u0000-\u0008\u000b-\u001f]/g,'');
   lines.push(text);
  }
  return lines.join('\n');
 }
 const activityHeaders=new Set(['Running','Ran','Exploring','Explored','Calling','Called','Failed','Browsing the web','Searching the web','Searched the web','Opening','Opened','Opening page','Opened page','Searching for','Searched for','Searching page','Searched page','Added','Deleted','Edited','Failed to apply patch','Viewed image','Updated Plan']);
 const audio=new Audio('/feedback.mp3');audio.preload='auto';audio.volume=1;
 let desired=window.OaclisVoice?.preferences?.startVoice===true;
 function feedback(){
  try{audio.pause();audio.currentTime=0;audio.play().catch(()=>{});}catch{}
 }
 function create(host){
  const engine=window.speechSynthesis,button=document.createElement('button');
  engine?.getVoices?.();
  button.type='button';button.className='read-aloud secondary';
  button.innerHTML=OaclisIcon('volume');
  let enabled=false,cursor=null,queue=[],speaking=false,current=null,generation=0,disposed=false,currentItem=null,echoUntil=0,quietUntil=0,playTimer=null,speechTimer=null,recoverSpeech=null,lastSpeechProgress=0,lastActivity='',lastActivityId=null,lastActivityAt=0;
  const items=new Map(),ignored=new Set();
  const paint=()=>{button.setAttribute('aria-pressed',String(enabled));button.title=OaclisSettings.text(enabled?(button.dataset.ready==='false'?'Lettura in collegamento':'Disattiva lettura'):'Attiva lettura');button.setAttribute('aria-label',button.title);};
  function play(){
   if(disposed||document.hidden||speaking||!enabled||!queue.length)return;
   const wait=quietUntil-Date.now();
   if(wait>0){if(!playTimer)playTimer=setTimeout(()=>{playTimer=null;play();},wait);return;}
   const {text,id,activity,retried=false}=queue.shift(),version=generation,utterance=new SpeechSynthesisUtterance(text);currentItem=id;
   if(activity){lastActivity=text;lastActivityId=id;lastActivityAt=Date.now();}
   utterance.lang=OaclisSettings.speechLanguage;
   const language=utterance.lang.toLowerCase(),voices=(engine.getVoices?.()||[]).filter(voice=>voice.lang.toLowerCase().split('-')[0]===language.split('-')[0]);
   utterance.voice=voices.find(voice=>/^Google\b/i.test(voice.name)&&voice.lang.toLowerCase()===language)||voices.find(voice=>/^Google\b/i.test(voice.name))||voices.find(voice=>voice.lang.toLowerCase()===language)||voices[0]||null;
   speaking=true;current=utterance;lastSpeechProgress=Date.now();let started=false;
   const done=()=>{if(version!==generation||disposed||current!==utterance)return;clearTimeout(speechTimer);speechTimer=null;recoverSpeech=null;speaking=false;current=null;currentItem=null;echoUntil=Date.now()+1000;if(queue.length)play();};
   const recover=()=>{
    speechTimer=null;
    if(version!==generation||disposed||current!==utterance)return;
    if(document.hidden)return;
    if(engine.paused===true)engine.resume();
    if((engine.speaking===false&&engine.pending!==true)||Date.now()-lastSpeechProgress>90000){
     utterance.onend=utterance.onerror=null;
     if(!retried&&!started)queue.unshift({text,id,activity,retried:true});
     quietUntil=Math.max(quietUntil,Date.now()+250);engine.cancel();done();return;
    }
    speechTimer=setTimeout(recover,1500);
   };
   recoverSpeech=recover;
   utterance.onstart=utterance.onboundary=()=>{if(current===utterance){started=true;lastSpeechProgress=Date.now();}};
   speechTimer=setTimeout(recover,1500);
   utterance.onend=done;utterance.onerror=event=>{if(version!==generation||disposed||current!==utterance)return;if(['interrupted','canceled','audio-busy','network','synthesis-failed'].includes(event.error)){if(!retried)queue.unshift({text,id,activity,retried:true});quietUntil=Math.max(quietUntil,Date.now()+500);done();return;}stop(false,true);button.title=OaclisSettings.text('Lettura non disponibile');button.setAttribute('aria-label',button.title);};try{engine.speak(utterance);}catch{utterance.onerror({error:'synthesis-failed'});}
  }
  function defer(milliseconds=2000){
   quietUntil=Date.now()+milliseconds;clearTimeout(playTimer);playTimer=null;play();
  }
  function stop(preserve=false,silent=false){
   clearTimeout(playTimer);playTimer=null;clearTimeout(speechTimer);speechTimer=null;recoverSpeech=null;quietUntil=0;
   lastActivity='';lastActivityId=null;lastActivityAt=0;
   if(!preserve)desired=false;
   if(enabled&&!silent)feedback();
   generation++;enabled=false;cursor=null;queue=[];items.clear();ignored.clear();speaking=false;current=null;currentItem=null;echoUntil=Date.now()+1000;
   engine?.cancel();paint();
  }
  function enable(silent=false){
   if(disposed||!engine||!window.SpeechSynthesisUtterance)return false;
   if(enabled)return true;
   if(!silent)feedback();desired=true;enabled=true;cursor=null;button.dataset.ready='false';paint();return true;
  }
  function skip(){
   const id=currentItem||queue[0]?.id||[...items.keys()].at(-1);
   if(!id)return;
   if(ignored.size>=16)ignored.delete(ignored.values().next().value);
   ignored.add(id);items.delete(id);queue=queue.filter(part=>part.id!==id);
   clearTimeout(speechTimer);speechTimer=null;recoverSpeech=null;generation++;speaking=false;current=null;currentItem=null;echoUntil=Date.now()+1000;
   engine?.cancel();if(queue.length)play();
  }
  function flush(item,complete){
   const clean=spokenText(item.raw);
   if(!clean.startsWith(item.spoken))return;
   const rest=clean.slice(item.spoken.length);
   const matches=[...rest.matchAll(/[.!?](?:[ \t]+|\n)|\n[ \t]*\n/g)];
   const end=complete?rest.length:matches.length?matches.at(-1).index+matches.at(-1)[0].length:0;
   if(!end)return;
   const ready=rest.slice(0,end);item.spoken=clean.slice(0,item.spoken.length+end).trimEnd();
   const sentence=ready.replace(/\s+/g,' ').trim();if(sentence){let rest=sentence;while(rest.length>220){const space=rest.lastIndexOf(' ',220),end=space>0?space:220;queue.push({id:item.id,text:rest.slice(0,end)});rest=rest.slice(end).trimStart();}if(rest)queue.push({id:item.id,text:rest});play();}
  }
  function update(data,version=generation){
   if(version!==generation||!enabled||!data||!Array.isArray(data.events))return;
   if(data.reset){clearTimeout(playTimer);playTimer=null;clearTimeout(speechTimer);speechTimer=null;recoverSpeech=null;generation++;queue=[];items.clear();speaking=false;current=null;currentItem=null;echoUntil=Date.now()+1000;engine.cancel();}
   let activity=cursor===null||data.reset?data.activity:null,activityId='working-status';
   for(const event of data.events){
    if(!Number.isSafeInteger(event.seq)||event.seq<0||typeof event.item!=='string'||(cursor!==null&&event.seq<=cursor))continue;
    if(ignored.has(event.item)){cursor=event.seq;continue;}
    if(event.kind==='working'){activity=event.activity;activityId=event.item;}
    if(event.kind==='start'){if(items.size>=8)items.delete(items.keys().next().value);items.set(event.item,{id:event.item,raw:'',spoken:''});}
    const item=items.get(event.item);
    if(item&&event.kind==='text'&&typeof event.text==='string'&&item.raw.length+event.text.length<=262144){item.raw+=event.text;flush(item,false);}
    if(item&&event.kind==='done'){flush(item,true);items.delete(event.item);}
    cursor=event.seq;
   }
   if(data.ready===true&&typeof activity==='string'&&activityHeaders.has(activity)){
    queue=queue.filter(part=>!part.activity);queue.push({id:activityId,text:activity,activity:true});play();
   }
   if(data.ready===true&&!activity&&!speaking&&!queue.length&&data.activity===lastActivity&&lastActivity&&!ignored.has(lastActivityId)&&Date.now()-lastActivityAt>=10000){
    queue.push({id:lastActivityId,text:lastActivity,activity:true});play();
   }
   if(Number.isSafeInteger(data.cursor))cursor=data.cursor;
   button.dataset.ready=String(data.ready===true);
   paint();
  }
  button.onclick=()=>{if(enabled)stop();else enable();};
  if(!engine||!window.SpeechSynthesisUtterance){button.disabled=true;button.title=OaclisSettings.text('Lettura non disponibile');}else paint();
  const hidden=()=>{if(!enabled)return;if(document.hidden){clearTimeout(speechTimer);speechTimer=null;engine.pause?.();}else{lastSpeechProgress=Date.now();engine.resume?.();if(recoverSpeech&&!speechTimer)speechTimer=setTimeout(recoverSpeech,1500);play();}};document.addEventListener('visibilitychange',hidden);host.append(button);if(desired)enable(true);
  return {reset:(active=true)=>{stop(true,true);echoUntil=0;if(active&&desired)enable(true);},enable,disable:stop,skip,defer,commandOnly:()=>speaking||Date.now()<echoUntil,stamp:()=>generation,request:()=>({speech:enabled,speech_cursor:cursor}),update,speaking:()=>speaking,stop,destroy:()=>{stop(true,true);disposed=true;document.removeEventListener('visibilitychange',hidden);button.remove();}};
 }
 return {create,spokenText,feedback};
})();
