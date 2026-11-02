// engines.js v1.3.37
'use strict';
window.OaclisEngines=(()=>{
 const names={codex:'ChatGPT · Codex',claude:'Claude',llamacpp:'llama.cpp'};
 function fields(){return OaclisSettings.html('<label class="field">Motore<select name="engine" required><option value="codex">ChatGPT · Codex</option><option value="claude">Claude</option><option value="llamacpp">llama.cpp</option></select></label><label class="field" data-engine-model>Modello<input name="model" maxlength="160" placeholder="Predefinito Codex"></label><small data-engine-status role="status"></small>');}
 async function bind(form,configuration,loadModels){
  const engine=form.elements.engine,slot=form.querySelector('[data-engine-model]'),status=form.querySelector('[data-engine-status]'),submit=form.querySelector('button:not([type="button"])');
  const drafts={};let providers=[],generation=0;
  async function change(){
   const token=++generation,provider=providers.find(item=>item.id===engine.value);
   submit.disabled=true;status.textContent='';slot.replaceChildren(document.createTextNode(OaclisSettings.text('Modello')));
   if(engine.value==='llamacpp'){
    const select=document.createElement('select');select.name='model';select.required=true;select.disabled=true;slot.append(select);
    if(!provider?.enabled){status.textContent=OaclisSettings.text('Configura questo motore nelle impostazioni.');return;}
    status.textContent=OaclisSettings.text('Caricamento modelli…');
    try{
     const reply=await loadModels();if(token!==generation||!form.isConnected)return;
     for(const model of reply.models||[]){const option=document.createElement('option');option.value=model.id;option.textContent=model.id;select.append(option);}
     if([...select.options].some(option=>option.value===(drafts.llamacpp||provider.model)))select.value=drafts.llamacpp||provider.model;
     select.disabled=!select.options.length;
     status.textContent=OaclisSettings.text(select.options.length?'Chat per questo motore non ancora disponibili':'Nessun modello disponibile');
    }catch(_){if(token===generation)status.textContent=OaclisSettings.text('Collegamento llama.cpp non disponibile');}
   }else{
    const input=document.createElement('input');input.name='model';input.maxLength=160;input.value=drafts[engine.value]??provider?.model??'';input.placeholder=OaclisSettings.text('Predefinito Codex');slot.append(input);
    input.disabled=!provider?.enabled||!provider?.available;
    submit.disabled=input.disabled;
    if(input.disabled)status.textContent=OaclisSettings.text(provider?.enabled?'Chat per questo motore non ancora disponibili':'Configura questo motore nelle impostazioni.');
   }
  }
  let previous='codex';engine.disabled=true;submit.disabled=true;
  engine.onchange=()=>{drafts[previous]=form.elements.model?.value||'';previous=engine.value;change();};
  try{
   const settings=await configuration();if(!form.isConnected)return;
   providers=settings.providers||[];engine.value=settings.default_engine||'codex';previous=engine.value;engine.disabled=false;await change();
  }catch(_){status.textContent=OaclisSettings.text('Configurazione non disponibile. Riapri il pannello.');}
 }
 return {names,fields,bind};
})();
