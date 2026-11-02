// jobs.js v1.17.79
'use strict';
window.OaclisJobs=(()=>{
 const text=value=>OaclisSettings.text(value);
 let panel,child,watch;
 function indicator(){
  const button=document.createElement('button');button.type='button';button.className='job-progress';button.setAttribute('aria-expanded','false');
  const detail=document.createElement('div');detail.className='job-text';detail.id='job-'+crypto.randomUUID();detail.setAttribute('popover','manual');detail.setAttribute('role','tooltip');button.setAttribute('aria-controls',detail.id);document.body.append(detail);
  let timer;
  function hide(){clearTimeout(timer);if(detail.matches(':popover-open'))detail.hidePopover();button.setAttribute('aria-expanded','false');}
  function show(){
   clearTimeout(timer);if(!detail.textContent)return;
   detail.showPopover();button.setAttribute('aria-expanded','true');
   const rect=button.getBoundingClientRect(),box=detail.getBoundingClientRect();
   detail.style.left=Math.max(8,Math.min(rect.right-box.width,innerWidth-box.width-8))+'px';
   detail.style.top=Math.max(8,Math.min(rect.bottom+6,innerHeight-box.height-8))+'px';timer=setTimeout(hide,5000);
  }
  button.onclick=event=>{event.stopPropagation();show();};button.onkeydown=event=>{if(event.key==='Escape'){hide();event.stopPropagation();}};
  return {button,show,update(job){
   const percent=typeof job.progress==='number'?new Intl.NumberFormat(OaclisSettings.language,{maximumFractionDigits:1}).format(Math.floor(job.progress*10)/10)+'%':null;
   if(button.textContent!==(percent||''))button.textContent=percent||'';button.classList.toggle('job-dot',!percent);button.dataset.stale=String(job.stale===true);
   button.setAttribute('aria-label',job.id+' · '+(percent||text('Avanzamento non disponibile')));if(detail.textContent!==(job.text||''))detail.textContent=job.text||'';
  },destroy(){hide();detail.remove();button.remove();}};
 }
 function inline(root){
  root.classList.add('jobs-inline');const rows=new Map();
  return {update(jobs=[]){
   const seen=new Set();
   for(const job of jobs){const key=JSON.stringify([job.device_id,job.chat_id,job.id]);seen.add(key);let entry=rows.get(key);if(!entry){entry=indicator();rows.set(key,entry);root.append(entry.button);}entry.update(job);}
   for(const [key,entry]of rows)if(!seen.has(key)){entry.destroy();rows.delete(key);}
  },stale(){for(const entry of rows.values())entry.button.dataset.stale='true';},destroy(){for(const entry of rows.values())entry.destroy();rows.clear();}};
 }
 function mount(root){
  const rows=new Map(),message=document.createElement('small');message.className='jobs-message';message.setAttribute('role','status');root.append(message);
  let stopped=false,timer,controller;
  function render(jobs){
   const seen=new Set();
   for(const job of jobs){
    const key=JSON.stringify([job.device_id,job.chat_id,job.id]);seen.add(key);let entry=rows.get(key);
    if(!entry){const item=document.createElement('div');item.className='job-row';const name=document.createElement('span');name.className='job-name';const progress=indicator();item.append(name,progress.button);root.insertBefore(item,message);entry={item,name,progress};rows.set(key,entry);}
    if(entry.name.textContent!==job.id)entry.name.textContent=job.id;entry.progress.update(job);
   }
   for(const [key,entry]of rows)if(!seen.has(key)){entry.progress.destroy();entry.item.remove();rows.delete(key);}
   message.textContent=jobs.length?'':text('Nessun lavoro collegato');
  }
  async function poll(){
   clearTimeout(timer);if(stopped||document.hidden)return;controller=new AbortController();
   try{
    const response=await fetch('/api/jobs/list',{method:'POST',headers:{'Content-Type':'application/json'},body:'{}',cache:'no-store',signal:controller.signal});
    if(!response.ok)throw Error('unavailable');const reply=await response.json();if(!stopped)render(reply.jobs);
   }catch(error){if(!stopped&&error.name!=='AbortError'){message.textContent=text('Monitor temporaneamente non disponibile');for(const entry of rows.values())entry.progress.button.dataset.stale='true';}}
   finally{controller=null;if(!stopped&&!document.hidden)timer=setTimeout(poll,2000);}
  }
  function visibility(){if(document.hidden){clearTimeout(timer);controller?.abort();}else poll();}
  document.addEventListener('visibilitychange',visibility);poll();
  return ()=>{stopped=true;clearTimeout(timer);controller?.abort();for(const entry of rows.values())entry.progress.destroy();document.removeEventListener('visibilitychange',visibility);};
 }
 function open(anchor){
  if(child&&!child.closed){child.focus();return;}
  if(panel?.isConnected){panel.hidePopover();panel.showPopover();return;}
  panel=document.createElement('section');panel.className='context-popover jobs-panel';panel.setAttribute('popover','manual');panel.setAttribute('role','dialog');panel.setAttribute('aria-label',text('Lavori'));panel.opener=anchor;
  panel.innerHTML=OaclisSettings.html('<header class="jobs-header"><h2>Lavori</h2><button type="button" class="icon-button jobs-detach" aria-label="Apri in una finestra separata">'+OaclisIcon('externalLink')+'</button><button type="button" class="icon-button jobs-close" aria-label="Chiudi">'+OaclisIcon('close')+'</button></header><div class="jobs-list"></div>');
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
