// voice-local.js v1.15.43
'use strict';
window.OaclisLocalVoice=(()=>{
 const toolbar=document.getElementById('toolbar-actions')||document.querySelector('.toolbar');
 const host=document.createElement('span');host.id='local-voice-controls';toolbar.append(host);
 const micHost=document.createElement('span'),readHost=document.createElement('span');host.append(micHost,readHost);
 const voice=OaclisVoice.create();let active=null,reader=null;
 const chats=()=>visibleChats.filter(chat=>!(data.deleted_chats||[]).includes(chat.id)&&(showHiddenChats||!data.chat_settings?.[chat.id]?.hidden)).map(chat=>({...chat,name:chatLabel(chat)}));
 const list=()=>{const byId=new Map(chats().map(chat=>[chat.id,chat]));return [...document.querySelectorAll('#library-list [data-chat-id]')].map(button=>byId.get(button.dataset.chatId)).filter(Boolean);};
 function bind(panel){
  const destination=panel?.querySelector('.terminal-keys')||toolbar;
  if(host.parentElement!==destination)destination.append(host);
  if(panel===active&&micHost.childElementCount)return;
  reader?.destroy();reader=null;active=panel;
  const adapter=panel?.voiceAdapter,search=document.getElementById('search');
  if(adapter)reader=OaclisReadAloud.create(readHost);
  voice.bind({chatId:adapter?.chatId,compose:adapter?.compose,fields:()=>[adapter?.compose||search].filter(Boolean),chats,list,
   selectChat:chat=>openChatTerminal({entry:chat},host,sourceId||'master'),
   ...(adapter?{send:adapter.send,sending:adapter.sending,takePhoto:adapter.takePhoto,pickImage:adapter.pickImage,removeImage:adapter.removeImage,hasAttachment:adapter.hasAttachment,commandOnly:()=>reader?.commandOnly(),readOn:()=>reader?.enable(),readOff:()=>reader?.disable(),readStop:()=>reader?.skip()}:{}),
  },micHost);
  micHost.querySelector('button').classList.add('icon-button');readHost.querySelector('button')?.classList.add('icon-button');voice.ready();
 }
 const info=document.createElement('button');info.type='button';info.className='icon-button';info.title=OaclisSettings.text('Comandi vocali');info.setAttribute('aria-label',info.title);
 info.innerHTML=OaclisIcon('info');info.onclick=()=>voice.showHelp();host.append(info);
 document.addEventListener('focusin',event=>{const panel=event.target.closest('.chat-terminal');if(panel?.voiceAdapter)bind(panel);else if(event.target.id==='search')bind(null);},true);
 bind(null);
 return {activate:bind,library:()=>bind(null),close:panel=>{if(panel===active)bind([...document.querySelectorAll('.chat-terminal')].at(-1)||null);},request:panel=>panel===active?reader?.request()||{}:{},stamp:panel=>panel===active&&reader?{reader,generation:reader.stamp()}:null,
  update:(panel,result,stamp,input)=>{if(panel===active&&stamp?.reader===reader){if(input)reader.defer(2000);reader.update(result.speech,stamp.generation);}},sent:panel=>{if(panel===active)voice.afterSend();},
 };
})();
