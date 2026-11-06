// voice-local.js v1.21.75
'use strict';
window.OaclisLocalVoice=(()=>{
 const toolbar=document.getElementById('toolbar-actions')||document.querySelector('.toolbar');
 const host=document.createElement('span');host.id='local-voice-controls';toolbar.append(host);
 const voice=OaclisVoice.create();let active=null;
 const chats=()=>visibleChats.filter(chat=>!(data.deleted_chats||[]).includes(chat.id)&&(showHiddenChats||!data.chat_settings?.[chat.id]?.hidden)).map(chat=>({...chat,name:chatLabel(chat),number:data.chat_settings?.[chat.id]?.number??chat.number}));
 function bind(panel){
  panel?.querySelector('.voice-controls:empty')?.remove();
  const destination=panel?.querySelector('.terminal-keys')||toolbar;
  if(host.parentElement!==destination)destination.append(host);
  if(panel===active&&host.childElementCount)return;
  active=panel;
  const adapter=panel?.voiceAdapter,search=document.getElementById('search');
  voice.bind({...adapter,fields:()=>[adapter?.compose||search].filter(Boolean),chats,
   selectChat:chat=>openChatTerminal({entry:chat},host,sourceId||'master'),
  },host);
 }
 document.addEventListener('focusin',event=>{const panel=event.target.closest('.chat-terminal');if(panel?.voiceAdapter)bind(panel);else if(event.target.id==='search')bind(null);},true);
 bind(null);
 return {activate:bind,library:()=>bind(null),close:panel=>{if(panel===active)bind([...document.querySelectorAll('.chat-terminal')].at(-1)||null);},request:panel=>panel===active?voice.reader?.request()||{}:{},stamp:panel=>panel===active&&voice.reader?{reader:voice.reader,generation:voice.reader.stamp()}:null,
  update:(panel,result,stamp,input)=>{if(panel===active&&stamp?.reader===voice.reader)voice.reader.update(result.speech,stamp.generation,input);},sent:panel=>{if(panel===active)voice.afterSend();},
 };
})();
