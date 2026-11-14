// skills.js v1.0.29
'use strict';
window.OaclisSkills=(()=>{
 const text=value=>OaclisSettings.text(value);
 let panel,child,watch,previous;
 const notified=new Map();
 function render(root,state){
  const devices=(state.devices||[]).map(d=>({...d}));
  if(!devices.some(d=>d.id==='master'))devices.unshift({id:'master',name:text('Questo dispositivo')});
  for(const device of devices)device.name=state.appearance?.['device:'+device.id]?.label||device.name;
  devices.sort((a,b)=>a.name.localeCompare(b.name,OaclisSettings.language));
  const signature=JSON.stringify([devices,state.device_runtime&&Object.entries(state.device_runtime).map(([id,r])=>[id,r.available,r.skills])]);
  if(root.dataset.snapshot===signature)return;root.dataset.snapshot=signature;root.replaceChildren();
  for(const device of devices){
   const runtime=state.device_runtime?.[device.id],skills=runtime?.skills;
   const section=document.createElement('section'),heading=document.createElement('h3');section.className='skills-device';heading.textContent=device.name;section.append(heading);
   const status=document.createElement('small');status.className='jobs-message';status.setAttribute('role','status');
   if(runtime?.available===false)status.textContent=text('Dispositivo non raggiungibile');
   else if(!skills)status.textContent=text('Rilevamento skill non disponibile su questo device');
   else if(skills.state==='updating')status.textContent=text('Aggiornamento skill in corso…');
   else if(skills.state==='checking')status.textContent=text('Verifica skill in corso…');
   else if(skills.state==='unavailable')status.textContent=text('Verifica skill non riuscita');
   else if(!skills.items.length)status.textContent=text('Nessuna skill installata');
   section.append(status);
   if(skills){
    const setting=document.createElement('label');setting.className='skills-auto';
    const check=document.createElement('input');check.type='checkbox';check.checked=skills.auto_update===true;check.disabled=runtime?.available===false;check.setAttribute('aria-label',text('Aggiorna automaticamente')+' · '+device.name);
    check.onchange=async()=>{
     check.disabled=true;
     try{const response=await fetch('/api/device/control',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({device_id:device.id,action:check.checked?'skills-auto-on':'skills-auto-off'})});const result=await response.json();if(!response.ok)throw Error(result.error||text('Operazione non riuscita'));state.device_runtime[device.id]=result;update(state);}
     catch(error){check.checked=!check.checked;if(typeof notice==='function')notice(error.message);else if(window.opener&&!window.opener.closed)window.opener.notice(error.message);else status.textContent=error.message;}
     finally{check.disabled=false;}
    };
    setting.append(check,document.createTextNode(text('Aggiorna automaticamente')));section.append(setting);
   }
   for(const skill of skills?.items||[]){
    const row=document.createElement('div');row.className='skill-row';
    const name=document.createElement('span');name.className='skill-name';name.textContent=skill.name;
    const enabled=document.createElement('span');enabled.className='skill-status';enabled.dataset.state=skill.invalid?'invalid':skill.enabled?'enabled':'disabled';enabled.textContent=text(skill.invalid?'Non disponibile':skill.enabled?'Attiva':'Disabilitata');
    const update=document.createElement('span');update.className='skill-status';update.dataset.state=skill.update;update.textContent=text(({current:'Aggiornata',outdated:'Da aggiornare',unknown:'Non verificabile'})[skill.update]||'Non verificabile');
    row.title=[skill.version,skill.latest&&skill.latest!==skill.version?'→ '+skill.latest:''].filter(Boolean).join(' ');row.append(name,enabled,update);section.append(row);
   }
   if(skills?.checked_at){const time=document.createElement('small');time.className='skills-checked';time.textContent=text('Ultima verifica')+' · '+new Date(skills.checked_at*1000).toLocaleString(OaclisSettings.language);section.append(time);}
   root.append(section);
  }
 }
 function update(state){
  previous=state;
  if(typeof notice==='function')for(const [id,runtime]of Object.entries(state.device_runtime||{})){
   const report=runtime.skills;if(!report)continue;
   const token=JSON.stringify([report.state,report.checked_at]);if(notified.get(id)===token)continue;notified.set(id,token);
   const name=state.appearance?.['device:'+id]?.label||(state.devices||[]).find(d=>d.id===id)?.name||text('Questo dispositivo');
   if(report.state==='updating')notice(text('Aggiornamento skill in corso…')+' · '+name,true);
   else if(report.state==='ready'){
    const outdated=(report.items||[]).filter(s=>s.update==='outdated').length;
    if(report.update_failed)notice(text('Aggiornamento skill non riuscito')+' · '+name);
    else if(report.updated)notice(text('Skill aggiornate')+' · '+name);
    else if(outdated)notice(text('Skill da aggiornare')+' · '+name+' ('+outdated+')');
   }
  }
 if(panel?.isConnected)render(panel.querySelector('.skills-list'),state);const count=Object.values(state.device_runtime||{}).reduce((n,r)=>n+(r.skills?.items||[]).filter(s=>s.update==='outdated').length,0);const button=document.getElementById('skills-monitor');if(button){button.dataset.updates=String(count);button.title=text('Skill')+(count?' · '+count+' '+text('Da aggiornare'):'');}}
 function open(anchor){
  if(child&&!child.closed){child.focus();return;}
  if(panel?.isConnected){panel.hidePopover();panel.showPopover();return;}
  panel=document.createElement('section');panel.className='context-popover jobs-panel skills-panel';panel.setAttribute('popover','manual');panel.setAttribute('role','dialog');panel.setAttribute('aria-label',text('Skill'));panel.opener=anchor;
  panel.innerHTML=OaclisSettings.html('<header class="jobs-header"><h2>Skill</h2><button type="button" class="icon-button skills-detach" aria-label="Apri in una finestra separata">'+OaclisIcon('externalLink')+'</button><button type="button" class="icon-button skills-close" aria-label="Chiudi">'+OaclisIcon('close')+'</button></header><div class="skills-list"></div>');
  document.body.append(panel);panel.showPopover();render(panel.querySelector('.skills-list'),previous||data);positionContext(panel,anchor);bindPanelDrag(panel);
  const current=panel;const close=()=>{current.remove();if(panel===current)panel=null;};
  current.querySelector('.skills-close').onclick=close;
  current.querySelector('.skills-detach').onclick=()=>{child=window.open('/skills','oaclis-skills','popup=yes,width=480,height=500');if(!child){notice(text('Consenti l’apertura della finestra nel browser.'));return;}child.focus();close();clearInterval(watch);watch=setInterval(()=>{if(child.closed){clearInterval(watch);child=null;open(anchor);}},500);};
 }
 if(document.querySelector('[data-skills-page]')){
  try{document.documentElement.dataset.theme=localStorage.getItem('oaclis-theme')||(matchMedia('(prefers-color-scheme:dark)').matches?'dark':'light');}catch{}
  OaclisSettings.translate(document.body);let timer,controller,stopped=false;
  async function poll(){
   clearTimeout(timer);if(stopped||document.hidden)return;controller=new AbortController();
   try{const response=await fetch('/api/skills/list',{method:'POST',headers:{'Content-Type':'application/json'},body:'{}',signal:controller.signal});if(!response.ok)throw Error();const value=await response.json();if(!stopped){document.querySelector('[role=status]').textContent='';render(document.querySelector('.skills-list'),value);}}
   catch(error){if(error.name!=='AbortError')document.querySelector('[role=status]').textContent=text('Monitor temporaneamente non disponibile');}
   finally{controller=null;if(!stopped&&!document.hidden)timer=setTimeout(poll,10000);}
  }
  document.addEventListener('visibilitychange',()=>{if(document.hidden){clearTimeout(timer);controller?.abort();}else poll();});window.addEventListener('pagehide',()=>{stopped=true;clearTimeout(timer);controller?.abort();},{once:true});poll();
 }
 return {open,update};
})();
