// gaze.js v1.0.0
'use strict';
window.OaclisGaze=(()=>{
 let panel=null,stream=null,generation=0,preferred='';
 try{preferred=localStorage.getItem('oaclis-gaze-camera')||'';}catch{}
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
   panel.innerHTML=OaclisSettings.html('<button type="button" class="icon-button popover-close" aria-label="Chiudi pannello">'+OaclisIcon('close')+'</button><h2>Controllo sguardo</h2><label class="field">Webcam di questo computer<select aria-label="Webcam di questo computer"></select></label><video autoplay muted playsinline aria-label="Anteprima webcam"></video><p role="status" aria-live="polite"></p><button type="button" class="primary" data-preview aria-pressed="false">Avvia anteprima</button><div class="gaze-actions"><button type="button" class="primary" disabled>Calibra</button><button type="button" class="primary" disabled>Test</button></div><label class="gaze-focus"><input type="checkbox" disabled>Focus automatico</label><p class="gaze-note">Calibrazione e rilevamento dello sguardo non ancora disponibili.</p>');
   document.body.append(panel);
   panel.querySelector('.popover-close').onclick=()=>panel.hidePopover();
   panel.querySelector('[data-preview]').onclick=()=>stream?stop():start();
   panel.querySelector('select').onchange=event=>{preferred=event.target.value;try{localStorage.setItem('oaclis-gaze-camera',preferred);}catch{}if(stream)start();};
   panel.addEventListener('toggle',event=>{anchor.setAttribute('aria-expanded',String(event.newState==='open'));if(event.newState==='closed')stop();});
   navigator.mediaDevices?.addEventListener('devicechange',()=>{if(panel.matches(':popover-open'))cameras().catch(()=>status('Nessuna webcam disponibile.'));});
   window.addEventListener('pagehide',stop);
  }
  if(panel.matches(':popover-open')){panel.hidePopover();return;}
  panel.showPopover();positionContext(panel,anchor);status('Anteprima spenta');
  if(!window.isSecureContext||!navigator.mediaDevices?.getUserMedia){panel.querySelector('[data-preview]').disabled=true;status('Apri Oaclis su localhost o HTTPS per usare la webcam.');return;}
  cameras().catch(()=>status('Nessuna webcam disponibile.'));
 }
 return {open};
})();
