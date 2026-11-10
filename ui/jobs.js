// jobs.js v1.20.146
'use strict';
window.OaclisJobs=(()=>{
 const text=value=>OaclisSettings.text(value);
 let panel,child,watch;
 const completions=new Map();
 function completed(job){
  const key='oaclis-job-complete:'+JSON.stringify([job.device_id,job.chat_id,job.id]),event=job.updated_at;
  const previous=completions.get(key);if(previous&&previous.event===event)return previous.promise;
  const claim=()=>{
   let saved;try{saved=JSON.parse(localStorage.getItem(key));}catch{}
   if(saved?.event===event&&Number.isFinite(saved.until))return saved.until;
   const until=Date.now()+5000;try{localStorage.setItem(key,JSON.stringify({event,until}));}catch{}
   window.OaclisReadAloud?.feedback();return until;
  };
  const promise=navigator.locks?.request?navigator.locks.request(key,claim).catch(claim):Promise.resolve(claim());
  completions.set(key,{event,promise});return promise;
 }
 function indicator(){
  const button=document.createElement('button');button.type='button';button.className='job-progress';button.setAttribute('aria-expanded','false');
  const detail=document.createElement('div');detail.className='job-text';detail.id='job-'+crypto.randomUUID();detail.setAttribute('popover','manual');detail.setAttribute('role','tooltip');button.setAttribute('aria-controls',detail.id);document.body.append(detail);
  let timer,doneTimer,doneEvent=null,disposed=false,lastUpdate;
  function hide(){clearTimeout(timer);if(detail.matches(':popover-open'))detail.hidePopover();button.setAttribute('aria-expanded','false');}
  function show(){
   clearTimeout(timer);if(!detail.textContent)return;
   detail.showPopover();button.setAttribute('aria-expanded','true');
   const rect=button.getBoundingClientRect(),box=detail.getBoundingClientRect();
   detail.style.left=Math.max(8,Math.min(rect.right-box.width,innerWidth-box.width-8))+'px';
   detail.style.top=Math.max(8,Math.min(rect.bottom+6,innerHeight-box.height-8))+'px';timer=setTimeout(hide,5000);
  }
  button.onclick=event=>{event.stopPropagation();show();};button.onkeydown=event=>{if(event.key==='Escape'){hide();event.stopPropagation();}};
  function dismiss(){
   button.hidden=true;button.classList.remove('job-complete');hide();
   const row=button.closest('.job-row');if(row)row.hidden=true;
   const parent=button.parentElement;if(parent?.classList.contains('jobs-inline')&&[...parent.children].every(item=>item.hidden))parent.hidden=true;
  }
  return {button,show,update(job){
   const fingerprint=JSON.stringify([job.id,job.chat_id,job.device_id,job.progress,job.text,job.updated_at,job.stale]);
   if(lastUpdate===fingerprint&&button.dataset.stale===String(job.stale===true))return !button.hidden;lastUpdate=fingerprint;
   if(job.progress===100){
    const event=JSON.stringify([job.device_id,job.chat_id,job.id,job.updated_at]);
    if(doneEvent!==event){doneEvent=event;clearTimeout(doneTimer);completed(job).then(until=>{if(disposed||doneEvent!==event)return;const remaining=until-Date.now();if(remaining<=0){dismiss();return;}button.hidden=false;const row=button.closest('.job-row');if(row)row.hidden=false;const parent=button.parentElement;if(parent?.classList.contains('jobs-inline'))parent.hidden=false;button.classList.add('job-complete');doneTimer=setTimeout(dismiss,remaining);});}
   }else{doneEvent=null;clearTimeout(doneTimer);button.hidden=false;button.classList.remove('job-complete');}
   const percent=typeof job.progress==='number'?new Intl.NumberFormat(OaclisSettings.language,{maximumFractionDigits:1}).format(Math.floor(job.progress*10)/10)+'%':null;
   if(button.textContent!==(percent||''))button.textContent=percent||'';button.classList.toggle('job-dot',!percent);button.dataset.stale=String(job.stale===true);
   button.setAttribute('aria-label',job.id+' · '+(percent||text('Avanzamento non disponibile')));if(detail.textContent!==(job.text||''))detail.textContent=job.text||'';return !button.hidden;
  },destroy(){disposed=true;clearTimeout(doneTimer);hide();detail.remove();button.remove();}};
 }
 function inline(root){
  root.classList.add('jobs-inline');const rows=new Map();
  return {update(jobs=[]){
   const seen=new Set();
   for(const job of jobs){const key=JSON.stringify([job.device_id,job.chat_id,job.id]);seen.add(key);let entry=rows.get(key);if(!entry){entry=indicator();rows.set(key,entry);root.append(entry.button);}entry.update(job);}
   for(const [key,entry]of rows)if(!seen.has(key)){entry.destroy();rows.delete(key);}
   root.hidden=[...rows.values()].every(entry=>entry.button.hidden);
  },stale(){for(const entry of rows.values())entry.button.dataset.stale='true';},destroy(){for(const entry of rows.values())entry.destroy();rows.clear();}};
 }
 function mount(root){
  const rows=new Map(),message=document.createElement('small');message.className='jobs-message';message.setAttribute('role','status');root.append(message);
  const endpoint=document.createElement('footer');endpoint.className='jobs-endpoint';
  const url=document.createElement('code'),command=document.createElement('code');url.textContent='http://127.0.0.1:23232/api/jobs/status';command.textContent="curl --max-time 2 -X POST "+url.textContent+" -H 'Content-Type: application/json' -d '{}'";endpoint.append(url,command);root.append(endpoint);
  let stopped=false,timer,controller,loading=false;
  function render(jobs){
   const seen=new Set();
   for(const job of jobs){
    const key=JSON.stringify([job.device_id,job.chat_id,job.id]);seen.add(key);let entry=rows.get(key);
    if(!entry){const item=document.createElement('div');item.className='job-row';const icon=document.createElement('span');icon.className='job-icon';icon.setAttribute('aria-hidden','true');const name=document.createElement('span');name.className='job-name';const progress=indicator();item.append(icon,name,progress.button);root.insertBefore(item,message);entry={item,icon,name,progress};rows.set(key,entry);}
    if(entry.name.textContent!==job.text)entry.name.textContent=job.text;
    const iconKey=JSON.stringify([job.icon,job.color]);
    if(entry.iconKey!==iconKey){
     entry.iconKey=iconKey;entry.icon.replaceChildren();
     if(typeof job.icon==='string'&&/^data:image\/png;base64,iVBORw0KGgo[A-Za-z0-9+/=]*$/.test(job.icon)&&job.icon.length<=48000){const image=document.createElement('img');image.src=job.icon;image.alt='';entry.icon.append(image);}
     else entry.icon.innerHTML=OaclisIcon(Object.hasOwn(OaclisIcons,job.icon)?job.icon:'chat');
     entry.icon.style.color=/^#[a-f0-9]{6}$/i.test(job.color||'')?job.color:'var(--ink)';
    }
    entry.item.hidden=!entry.progress.update(job);
   }
   for(const [key,entry]of rows)if(!seen.has(key)){entry.progress.destroy();entry.item.remove();rows.delete(key);}
   message.textContent=[...rows.values()].some(entry=>!entry.item.hidden)?'':text('Nessun dato disponibile');
  }
  async function poll(){
   clearTimeout(timer);if(stopped||document.hidden||loading)return;loading=true;controller=new AbortController();
   try{
    const response=await fetch('/api/jobs/list',{method:'POST',headers:{'Content-Type':'application/json'},body:'{}',cache:'no-store',signal:controller.signal});
    if(!response.ok)throw Error('unavailable');const reply=await response.json();if(!stopped)render(reply.jobs);
   }catch(error){if(!stopped&&error.name!=='AbortError'){message.textContent=text('Monitor temporaneamente non disponibile');for(const entry of rows.values())entry.progress.button.dataset.stale='true';}}
   finally{const delay=controller?.signal.aborted?0:2000;controller=null;loading=false;if(!stopped&&!document.hidden)timer=setTimeout(poll,delay);}
  }
  function visibility(){if(document.hidden){clearTimeout(timer);controller?.abort();}else poll();}
  document.addEventListener('visibilitychange',visibility);poll();
  return ()=>{stopped=true;clearTimeout(timer);controller?.abort();for(const entry of rows.values())entry.progress.destroy();document.removeEventListener('visibilitychange',visibility);};
 }
 function open(anchor){
  if(child&&!child.closed){child.focus();return;}
  if(panel?.isConnected){panel.hidePopover();panel.showPopover();return;}
  panel=document.createElement('section');panel.className='context-popover jobs-panel';panel.setAttribute('popover','manual');panel.setAttribute('role','dialog');panel.setAttribute('aria-label',text('Statistiche'));panel.opener=anchor;
  panel.innerHTML=OaclisSettings.html('<header class="jobs-header"><h2>Statistiche</h2><button type="button" class="icon-button jobs-detach" aria-label="Apri in una finestra separata">'+OaclisIcon('externalLink')+'</button><button type="button" class="icon-button jobs-close" aria-label="Chiudi">'+OaclisIcon('close')+'</button></header><div class="jobs-list"></div>');
  document.body.append(panel);panel.showPopover();positionContext(panel,anchor);bindPanelDrag(panel);
  const current=panel,stop=mount(panel.querySelector('.jobs-list'));
  function raise(event){if(event.target===current||event.target.matches?.('.job-text')||event.newState!=='open'||!current.isConnected)return;requestAnimationFrame(()=>{if(!current.isConnected)return;const focus=document.activeElement;current.hidePopover();current.showPopover();focus?.focus({preventScroll:true});});}
  function close(){stop();document.removeEventListener('toggle',raise,true);current.remove();if(panel===current)panel=null;}
  document.addEventListener('toggle',raise,true);
  current.querySelector('.jobs-close').onclick=close;
  current.querySelector('.jobs-detach').onclick=()=>{
   child=window.open('/jobs','oaclis-jobs','popup=yes,width=380,height=320');
   if(!child){notice(text('Consenti l’apertura della finestra nel browser.'));return;}
   child.focus();close();clearInterval(watch);watch=setInterval(()=>{if(child.closed){clearInterval(watch);child=null;open(anchor);}},500);
  };
 }
 if(document.querySelector('[data-jobs-page]')){
  try{document.documentElement.dataset.theme=localStorage.getItem('oaclis-theme')||(matchMedia('(prefers-color-scheme:dark)').matches?'dark':'light');}catch{}
  OaclisSettings.translate(document.body);const stop=mount(document.querySelector('.jobs-list'));window.addEventListener('pagehide',stop,{once:true});
 }
 return {open,mount,inline};
})();
