// gaze-calibration.js v1.0.0
'use strict';
(async()=>{
 const $=selector=>document.querySelector(selector),text=value=>OaclisSettings.text(value);
 OaclisSettings.translate(document.body);
 const source=window.opener?.OaclisGaze?.calibrationSource();
 if(!source){$('[role="status"]').textContent=text('Apri la calibrazione dal pannello Controllo sguardo.');return;}
 document.documentElement.dataset.theme=source.theme;document.documentElement.dataset.palette=source.palette;
 const {video,monitors}=source,canvas=$('canvas'),ctx=canvas.getContext('2d'),stage=$('.gaze-calibration-stage'),next=$('[data-next]'),reset=$('[data-reset]'),note=$('[role="status"]'),instruction=$('[data-instructions]'),screenSelect=$('[data-screen]');
 const roi=document.createElement('canvas');roi.width=80;roi.height=40;const pixels=roi.getContext('2d',{willReadFrequently:true});
 let eyes=[],drag=null,frozen=null,details=null,monitorIndex=0,pointIndex=0,busy=false,closed=false,sampling=false,epoch=0;
 const samples=[],points=[...Array(9)].map((_,i)=>[0.1+(i%3)*0.4,0.1+Math.floor(i/3)*0.4]);
 const request=async(path,value)=>{const response=await fetch('/api/gaze/'+path,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(value)}),result=await response.json();if(!response.ok||result.error)throw Error(text(result.error)||text('Impossibile completare la calibrazione.'));return result;};
 const pause=ms=>new Promise(resolve=>window.setTimeout(resolve,ms));
 function message(value){note.textContent=text(value);}
 function draw(){ctx.putImageData(frozen,0,0);ctx.strokeStyle=getComputedStyle(document.documentElement).getPropertyValue('--ink');ctx.lineWidth=3;eyes.forEach(eye=>ctx.strokeRect(eye.x,eye.y,eye.width,eye.height));}
 function freeze(){
  if(!video.videoWidth){message('La webcam non è più disponibile. Riapri l’anteprima.');return;}
  eyes=[];canvas.width=video.videoWidth;canvas.height=video.videoHeight;ctx.drawImage(video,0,0);frozen=ctx.getImageData(0,0,canvas.width,canvas.height);next.disabled=true;
  instruction.textContent=text('Trascina un rettangolo intorno a ciascun occhio, includendo pupilla e parte bianca. Mantieni ferma la testa.');message('Seleziona il primo occhio.');
 }
 function position(event){const box=canvas.getBoundingClientRect();return {x:Math.max(0,Math.min(canvas.width,(event.clientX-box.left)*canvas.width/box.width)),y:Math.max(0,Math.min(canvas.height,(event.clientY-box.top)*canvas.height/box.height))};}
 canvas.onpointerdown=event=>{if(eyes.length===2||busy||event.button!==0)return;drag=position(event);canvas.setPointerCapture(event.pointerId);};
 canvas.onpointermove=event=>{if(!drag)return;const p=position(event);draw();ctx.strokeRect(drag.x,drag.y,p.x-drag.x,p.y-drag.y);};
 canvas.onpointerup=event=>{
  if(!drag)return;const p=position(event),eye={x:Math.round(Math.min(p.x,drag.x)),y:Math.round(Math.min(p.y,drag.y)),width:Math.round(Math.abs(p.x-drag.x)),height:Math.round(Math.abs(p.y-drag.y))};drag=null;
  if(eye.width<12||eye.height<6||eye.width>canvas.width/2||eye.height>canvas.height/3||eyes.some(a=>eye.x<a.x+a.width&&eye.x+eye.width>a.x&&eye.y<a.y+a.height&&eye.y+eye.height>a.y)){draw();message('Seleziona una zona distinta e abbastanza grande intorno all’occhio.');return;}
  eyes.push(eye);draw();next.disabled=eyes.length!==2;message(eyes.length===2?'Occhi selezionati. Continua per scegliere lo schermo.':'Seleziona il secondo occhio.');
 };
 canvas.onpointercancel=()=>{drag=null;draw();};
 function frame(){
  if(!video.srcObject?.active||video.videoWidth!==canvas.width||video.videoHeight!==canvas.height)throw Error(text('La webcam non è più disponibile. Riapri l’anteprima.'));
  return eyes.map(eye=>{pixels.drawImage(video,eye.x,eye.y,eye.width,eye.height,0,0,80,40);const data=pixels.getImageData(0,0,80,40).data,gray=[];for(let i=0;i<data.length;i+=4)gray.push(Math.round(data[i]*0.299+data[i+1]*0.587+data[i+2]*0.114));return {width:80,height:40,pixels:gray};});
 }
 function monitorName(monitor){return text('Monitor')+' '+(monitor.id.match(/DISPLAY(\d+)$/i)?.[1]||monitorIndex+1);}
 function chooseScreen(){
  const monitor=monitors[monitorIndex];instruction.textContent=monitorName(monitor)+' · '+monitor.width+' × '+monitor.height+' · '+monitor.x+', '+monitor.y;
  canvas.hidden=true;reset.hidden=true;$('[data-screen-field]').hidden=false;
  screenSelect.replaceChildren(new Option(text('Seleziona lo schermo corrispondente'),''));
  details.screens.forEach((screen,index)=>screenSelect.add(new Option((screen.label||text('Monitor')+' '+(index+1))+' · '+screen.width+' × '+screen.height+' · '+screen.left+', '+screen.top,String(index))));
  const candidates=details.screens.filter(s=>s.width*s.devicePixelRatio===monitor.width&&s.height*s.devicePixelRatio===monitor.height&&s.left*s.devicePixelRatio===monitor.x&&s.top*s.devicePixelRatio===monitor.y&&s.isPrimary===monitor.primary);
  const match=candidates.length===1?candidates[0]:monitor.primary?details.screens.find(s=>s.isPrimary):null;
  screenSelect.value=match?String(details.screens.indexOf(match)):'';next.disabled=!screenSelect.value;next.textContent=text('Avvia i 9 punti');
  message('Conferma lo schermo corrispondente. Guarda ogni punto e premi Spazio, senza muovere la testa.');
 }
 screenSelect.onchange=()=>next.disabled=screenSelect.value==='';
 function showPoint(){
  const p=points[pointIndex],dot=$('.gaze-target');dot.style.left=p[0]*100+'%';dot.style.top=p[1]*100+'%';dot.disabled=false;
  $('[data-progress]').textContent=monitorName(monitors[monitorIndex])+' · '+(pointIndex+1)+' / 9';$('[data-hint]').textContent=text('Guarda il punto e premi Spazio.');
 }
 async function finishMonitor(){
  sampling=false;if(document.fullscreenElement)await document.exitFullscreen();stage.hidden=true;
  monitorIndex++;
  if(monitorIndex<monitors.length){pointIndex=0;chooseScreen();return;}
  next.disabled=true;instruction.textContent=text('Calcolo della calibrazione…');message('');
  const model=await request('calibrate',{samples});
  // Keep only coordinates and the fitted model; camera frames are never persisted.
  const calibration={version:1,created_at:new Date().toISOString(),camera:source.camera,width:canvas.width,height:canvas.height,eyes,monitors,samples,model};
  window.opener.OaclisGaze.saveCalibration(calibration);next.hidden=true;$('[data-screen-field]').hidden=true;
  instruction.textContent=text('Calibrazione salvata. Focus automatico ancora disattivato.');note.textContent=text('Errore stimato')+': '+Math.round(model.validation_error_px)+' px · '+samples.length+' '+text('punti');
 }
 async function capture(){
  if(!sampling||busy)return;busy=true;const token=epoch;$('.gaze-target').disabled=true;$('[data-hint]').textContent=text('Acquisizione… mantieni lo sguardo sul punto.');
  try{
   await pause(300);const values=[];
   for(let i=0;i<12;i++){
    if(closed||token!==epoch)return;const result=await request('pupils',{eyes:frame()});if(result.valid)values.push(result.position);await pause(100);
   }
   if(closed||token!==epoch)return;
   if(values.length<8)throw Error(text('Pupille non distinguibili. Riprova il punto oppure riseleziona gli occhi.'));
   const median=k=>values.map(p=>p[k]).sort((a,b)=>a-b)[Math.floor(values.length/2)],eye=[median(0),median(1)];
   if(values.filter(p=>Math.hypot(p[0]-eye[0],p[1]-eye[1])<0.035).length<8)throw Error(text('Sguardo instabile. Guarda il punto e premi di nuovo Spazio.'));
   const monitor=monitors[monitorIndex],p=points[pointIndex];samples.push({eye,target:[monitor.x+p[0]*monitor.width,monitor.y+p[1]*monitor.height]});
   pointIndex++;if(pointIndex===9)await finishMonitor();else showPoint();
  }catch(error){if(token===epoch&&!closed){if(sampling){$('[data-hint]').textContent=error.message;$('.gaze-target').disabled=false;}else{message(error.message);next.hidden=true;reset.hidden=false;reset.textContent=text('Ricomincia');}}}
  finally{busy=false;}
 }
 next.onclick=async()=>{
  next.disabled=true;reset.disabled=true;
  try{
   if(!details){
    if(!window.getScreenDetails)throw Error(text('Per la calibrazione su più monitor apri Oaclis con Chrome sul computer Windows.'));
    details=await window.getScreenDetails();details.addEventListener('screenschange',()=>abort('Disposizione monitor cambiata. Riavvia la calibrazione.'));
    const result=await request('pupils',{eyes:frame()});if(!result.valid)throw Error(text('Pupille non distinguibili. Riseleziona gli occhi.'));
    chooseScreen();
   }else{
    const screen=details.screens[Number(screenSelect.value)];stage.hidden=false;
    await stage.requestFullscreen({screen,navigationUI:'hide'});await pause(250);
    if(details.currentScreen!==screen){await document.exitFullscreen();throw Error(text('Schermo non corrispondente. Seleziona lo schermo corretto.'));}
    sampling=true;showPoint();
   }
  }catch(error){stage.hidden=true;message(error.message);if(!sampling&&canvas.hidden===false)details=null;next.disabled=false;}finally{reset.disabled=false;}
 };
 function abort(value){
  epoch++;sampling=false;busy=false;if(document.fullscreenElement)document.exitFullscreen().catch(()=>{});stage.hidden=true;next.hidden=true;reset.hidden=false;reset.textContent=text('Ricomincia');message(value);
 }
 reset.onclick=()=>{epoch++;samples.length=0;details=null;monitorIndex=0;pointIndex=0;next.hidden=false;next.textContent=text('Continua');reset.textContent=text('Riseleziona occhi');canvas.hidden=false;$('[data-screen-field]').hidden=true;freeze();};
 $('.gaze-target').onclick=capture;$('[data-cancel]').onclick=()=>abort('Calibrazione annullata.');
 document.addEventListener('keydown',event=>{if(sampling&&event.code==='Space'){event.preventDefault();if(!event.repeat)capture();}});
 document.addEventListener('fullscreenchange',()=>{if(sampling&&!document.fullscreenElement)abort('Calibrazione annullata.');});
 $('[data-close]').onclick=()=>window.close();
 window.addEventListener('pagehide',()=>{closed=true;epoch++;window.opener?.OaclisGaze?.calibrationClosed();});
 if(!monitors.length){message('Seleziona almeno un monitor.');return;}
 freeze();
})();
