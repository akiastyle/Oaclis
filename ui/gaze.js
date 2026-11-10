// gaze.js v1.2.29
'use strict';
window.OaclisGaze=(()=>{
 let panel=null,stream=null,generation=0,preferred='',selected=null,monitorLoading=false;
 try{preferred=localStorage.getItem('oaclis-gaze-camera')||'';const saved=JSON.parse(localStorage.getItem('oaclis-gaze-monitors')||'null');if(Array.isArray(saved))selected=new Set(saved);}catch{}
 const text=value=>OaclisSettings.text(value);
 function status(value){panel.querySelector('[role="status"]').textContent=text(value);}
 function stop(){
  generation++;stream?.getTracks().forEach(track=>track.stop());stream=null;
  if(!panel)return;
  panel.querySelector('video').srcObject=null;panel.querySelector('select').disabled=false;
  const button=panel.querySelector('[data-preview]');button.disabled=false;button.textContent=text('Avvia anteprima');button.setAttribute('aria-pressed','false');
  status('Anteprima spenta');
 }
 async function cameras(){
  const select=panel.querySelector('select'),devices=await navigator.mediaDevices.enumerateDevices();
  select.replaceChildren(new Option(text('Webcam predefinita'),''));
  devices.filter(device=>device.kind==='videoinput').forEach((device,index)=>select.add(new Option(device.label||'Webcam '+(index+1),device.deviceId)));
  select.value=[...select.options].some(option=>option.value===preferred)?preferred:'';
 }
 async function monitors(){
  if(monitorLoading)return;monitorLoading=true;
  const map=panel.querySelector('.gaze-monitors'),note=panel.querySelector('[data-monitor-status]'),refresh=panel.querySelector('[data-monitor-refresh]');refresh.disabled=true;map.setAttribute('aria-busy','true');note.textContent=text('Rilevamento monitor…');
  try{
   const result=await api('gaze/monitors',{});map.replaceChildren();
   if(!result.supported){note.textContent=text('Rilevamento monitor disponibile sul computer Windows di Oaclis.');return;}
   if(!result.monitors.length){note.textContent=text('Nessun monitor attivo.');return;}
   const available=result.monitors.sort((a,b)=>Number(b.primary)-Number(a.primary)||a.y-b.y||a.x-b.x),desktop=result.desktop;
   selected=selected===null?new Set(available.map(m=>m.id)):new Set([...selected].filter(id=>available.some(m=>m.id===id)));
   map.style.setProperty('--gaze-aspect',desktop.width/desktop.height);
   const update=()=>{map.querySelectorAll('button').forEach(button=>button.setAttribute('aria-pressed',String(selected.has(button.dataset.id))));note.textContent=text('Monitor selezionati')+': '+selected.size+' / '+available.length;};
   available.forEach((monitor,index)=>{
    const number=monitor.id.match(/DISPLAY(\d+)$/i)?.[1]||index+1,button=document.createElement('button');button.type='button';button.dataset.id=monitor.id;button.title=monitor.id+' · '+monitor.width+' × '+monitor.height+' · '+monitor.x+', '+monitor.y;
    button.innerHTML=OaclisIcon('desktop');const label=document.createElement('span');label.textContent=number+(monitor.primary?' · '+text('Principale'):'');button.append(label);button.setAttribute('aria-label',text('Monitor')+' '+number+(monitor.primary?' · '+text('Principale'):''));
    button.style.left=(monitor.x-desktop.x)/desktop.width*100+'%';button.style.top=(monitor.y-desktop.y)/desktop.height*100+'%';button.style.width=monitor.width/desktop.width*100+'%';button.style.height=monitor.height/desktop.height*100+'%';
    button.onclick=()=>{if(selected.has(monitor.id))selected.delete(monitor.id);else selected.add(monitor.id);try{localStorage.setItem('oaclis-gaze-monitors',JSON.stringify([...selected]));}catch{}update();};map.append(button);
   });update();
  }catch{map.replaceChildren();note.textContent=text('Impossibile rilevare i monitor. Riprova.');}
  finally{monitorLoading=false;refresh.disabled=false;map.removeAttribute('aria-busy');}
 }
 async function start(){
  stop();const version=generation,button=panel.querySelector('[data-preview]');button.disabled=true;panel.querySelector('select').disabled=true;status('Collegamento webcam…');
  try{
   const incoming=await navigator.mediaDevices.getUserMedia({audio:false,video:{...(panel.querySelector('select').value?{deviceId:{exact:panel.querySelector('select').value}}:{}),width:{ideal:640},height:{ideal:480},frameRate:{ideal:15,max:15}}});
   if(version!==generation||!panel.matches(':popover-open')){incoming.getTracks().forEach(track=>track.stop());return;}
   stream=incoming;const video=panel.querySelector('video');video.srcObject=incoming;await video.play();
   if(version!==generation)return;
   button.textContent=text('Ferma anteprima');button.setAttribute('aria-pressed','true');status('Anteprima attiva · nessun rilevamento dello sguardo');
   incoming.getVideoTracks()[0].addEventListener('ended',()=>{if(stream===incoming){stop();status('Webcam scollegata');}});
   await cameras();
  }catch(error){if(version===generation){stop();status(({NotAllowedError:'Accesso alla webcam negato. Consenti la fotocamera nelle impostazioni del browser.',NotFoundError:'Nessuna webcam disponibile.',NotReadableError:'Webcam occupata da un altro programma.',OverconstrainedError:'Webcam selezionata non disponibile. Scegline un’altra.'})[error.name]||'Impossibile avviare la webcam.');}}
  finally{if(version===generation){button.disabled=false;panel.querySelector('select').disabled=false;}}
 }
 function open(anchor){
  if(!panel){
   panel=document.createElement('section');panel.id='gaze-panel';panel.className='context-popover';panel.setAttribute('popover','auto');panel.setAttribute('role','dialog');panel.setAttribute('aria-label',text('Controllo sguardo'));panel.opener=anchor;
   panel.innerHTML=OaclisSettings.html('<button type="button" class="icon-button popover-close" aria-label="Chiudi pannello">'+OaclisIcon('close')+'</button><h2>Controllo sguardo</h2><label class="field">Webcam di questo computer<select aria-label="Webcam di questo computer"></select></label><section class="gaze-display-settings"><div class="gaze-display-heading"><h3>Monitor per la calibrazione</h3><button type="button" class="icon-button" data-monitor-refresh aria-label="Rileva monitor" title="Rileva monitor">'+OaclisIcon('refresh')+'</button></div><div class="gaze-monitors" role="group" aria-label="Monitor per la calibrazione"></div><p data-monitor-status aria-live="polite"></p></section><video autoplay muted playsinline aria-label="Anteprima webcam"></video><p role="status" aria-live="polite"></p><button type="button" class="primary" data-preview aria-pressed="false">Avvia anteprima</button><div class="gaze-actions"><button type="button" class="primary" disabled>Calibra</button><button type="button" class="primary" disabled>Test</button></div><label class="gaze-focus"><input type="checkbox" disabled>Focus automatico</label><p class="gaze-note">Calibrazione e rilevamento dello sguardo non ancora disponibili.</p>');
   document.body.append(panel);
   panel.querySelector('[data-monitor-refresh]').onclick=monitors;
   panel.querySelector('.popover-close').onclick=()=>panel.hidePopover();
   panel.querySelector('[data-preview]').onclick=()=>stream?stop():start();
   panel.querySelector('select').onchange=event=>{preferred=event.target.value;try{localStorage.setItem('oaclis-gaze-camera',preferred);}catch{}if(stream)start();};
   panel.addEventListener('toggle',event=>{anchor.setAttribute('aria-expanded',String(event.newState==='open'));if(event.newState==='closed')stop();});
   navigator.mediaDevices?.addEventListener('devicechange',()=>{if(panel.matches(':popover-open'))cameras().catch(()=>status('Nessuna webcam disponibile.'));});
   window.addEventListener('pagehide',stop);
  }
  if(panel.matches(':popover-open')){panel.hidePopover();return;}
  panel.showPopover();positionContext(panel,anchor);status('Anteprima spenta');monitors();
  if(!window.isSecureContext||!navigator.mediaDevices?.getUserMedia){panel.querySelector('[data-preview]').disabled=true;status('Apri Oaclis su localhost o HTTPS per usare la webcam.');return;}
  cameras().catch(()=>status('Nessuna webcam disponibile.'));
 }
 return {open};
})();
