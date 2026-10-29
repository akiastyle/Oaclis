// read-aloud.js v1.17.151
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
 let audio,desired=false;
 function feedback(mode){
  try{
   const Audio=window.AudioContext||window.webkitAudioContext;if(!Audio)return;
   audio??=new Audio();audio.resume().catch(()=>{});
   const tone=audio.createOscillator(),gain=audio.createGain(),now=audio.currentTime;
   tone.frequency.value=mode==='off'?440:mode==='pause'?660:880;
   gain.gain.setValueAtTime(.35,now);gain.gain.exponentialRampToValueAtTime(.001,now+.09);
   tone.connect(gain);gain.connect(audio.destination);tone.start();tone.stop(now+.1);tone.onended=()=>{tone.disconnect();gain.disconnect();};
  }catch{}
 }
 function create(host,{pause=()=>{},resume=()=>{}}={}){
  const engine=window.speechSynthesis,button=document.createElement('button');
  button.type='button';button.className='read-aloud secondary';
  button.innerHTML='<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 9h4l5-4v14l-5-4H4zM16 8a6 6 0 0 1 0 8m2-11a10 10 0 0 1 0 14"/></svg>';
  let enabled=false,cursor=null,queue=[],speaking=false,current=null,generation=0,disposed=false,currentItem=null,echoUntil=0,lastWorkAt=0;
  const items=new Map(),ignored=new Set();
  const paint=()=>{button.setAttribute('aria-pressed',String(enabled));button.title=OaclisSettings.text(enabled?(button.dataset.ready==='false'?'Lettura in collegamento':'Disattiva lettura'):'Attiva lettura');button.setAttribute('aria-label',button.title);};
  function play(){
   if(disposed||speaking||!enabled||!queue.length)return;
   const {text,id}=queue.shift(),version=generation,utterance=new SpeechSynthesisUtterance(text);currentItem=id;
   utterance.lang=OaclisSettings.speechLanguage;speaking=true;current=utterance;pause();
   const done=()=>{if(version!==generation||disposed)return;speaking=false;current=null;currentItem=null;echoUntil=Date.now()+1000;if(queue.length)play();else resume();};
   utterance.onend=done;utterance.onerror=()=>{if(version!==generation||disposed)return;stop();button.title=OaclisSettings.text('Lettura non disponibile');button.setAttribute('aria-label',button.title);};engine.speak(utterance);
  }
  function stop(preserve=false,silent=false){
   if(!preserve)desired=false;
   if(enabled&&!silent)feedback('off');
   generation++;enabled=false;cursor=null;queue=[];items.clear();ignored.clear();speaking=false;current=null;currentItem=null;echoUntil=Date.now()+1000;
   engine?.cancel();paint();resume();
  }
  function enable(silent=false){
   if(disposed||!engine||!window.SpeechSynthesisUtterance)return false;
   if(enabled)return true;
   if(!silent)feedback('on');desired=true;enabled=true;cursor=null;lastWorkAt=0;button.dataset.ready='false';paint();return true;
  }
  function skip(){
   feedback('pause');
   const id=currentItem||queue[0]?.id||[...items.keys()].at(-1);
   if(!id)return;
   if(ignored.size>=16)ignored.delete(ignored.values().next().value);
   ignored.add(id);items.delete(id);queue=queue.filter(part=>part.id!==id);
   generation++;speaking=false;current=null;currentItem=null;echoUntil=Date.now()+1000;
   engine?.cancel();if(queue.length)play();else resume();
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
   if(data.reset){generation++;queue=[];items.clear();speaking=false;current=null;currentItem=null;echoUntil=Date.now()+1000;engine.cancel();resume();}
   let activity=data.activity;
   for(const event of data.events){
    if(!Number.isSafeInteger(event.seq)||event.seq<0||typeof event.item!=='string'||(cursor!==null&&event.seq<=cursor))continue;
    if(ignored.has(event.item)){cursor=event.seq;continue;}
    if(event.kind==='working')activity=event.activity||'commandExecution';
    if(event.kind==='start'){if(items.size>=8)items.delete(items.keys().next().value);items.set(event.item,{id:event.item,raw:'',spoken:''});}
    const item=items.get(event.item);
    if(item&&event.kind==='text'&&typeof event.text==='string'&&item.raw.length+event.text.length<=262144){item.raw+=event.text;flush(item,false);}
    if(item&&event.kind==='done'){flush(item,true);items.delete(event.item);}
    cursor=event.seq;
   }
   if(data.ready===true&&typeof activity==='string'&&!speaking&&!queue.length&&(!lastWorkAt||Date.now()-lastWorkAt>=30000)){
    const label=({commandExecution:'Esecuzione comando',fileChange:'Modifica file',webSearch:'Ricerca in corso',mcpToolCall:'Esecuzione comando',dynamicToolCall:'Esecuzione comando',processing:'Elaborazione in corso'})[activity];
    if(label){lastWorkAt=Date.now();queue.push({id:'working-status',text:OaclisSettings.text(label)});play();}
   }
   if(Number.isSafeInteger(data.cursor))cursor=data.cursor;
   button.dataset.ready=String(data.ready===true);
   paint();
  }
  button.onclick=()=>{if(enabled)stop();else enable();};
  if(!engine||!window.SpeechSynthesisUtterance){button.disabled=true;button.title=OaclisSettings.text('Lettura non disponibile');}else paint();
  const hidden=()=>{if(document.hidden)stop();};document.addEventListener('visibilitychange',hidden);host.append(button);if(desired)enable(true);
  return {enable,disable:stop,skip,commandOnly:()=>speaking||Date.now()<echoUntil,stamp:()=>generation,request:()=>({speech:enabled,speech_cursor:cursor}),update,speaking:()=>speaking,stop,destroy:()=>{stop(true,true);disposed=true;document.removeEventListener('visibilitychange',hidden);button.remove();}};
 }
 return {create,spokenText,feedback};
})();
