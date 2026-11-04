// terminal-format.js v1.29.212
"use strict";
const OaclisTerminal = (() => {
  const palette=['#000000','#cd3131','#0dbc79','#e5e510','#2472c8','#bc3fbc','#11a8cd','#e5e5e5','#666666','#f14c4c','#23d18b','#f5f543','#3b8eea','#d670d6','#29b8db','#ffffff'];
  function color(index) {
    if(index<16)return palette[index];
    if(index<232){const n=index-16,v=[0,95,135,175,215,255];return `rgb(${v[Math.floor(n/36)]},${v[Math.floor(n/6)%6]},${v[n%6]})`;}
    const gray=8+(index-232)*10;return `rgb(${gray},${gray},${gray})`;
  }
  function parse(ansi) {
    const runs=[];let state={},position=0,cursor=0;
    const pattern=/\x1b\[([0-9;:]*)m|\x1b\[[0-?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)/g;
    const push = text => {if(text){runs.push({text,start:position,end:position+text.length,style:{...state}});position+=text.length;}};
    for(const match of ansi.matchAll(pattern)){
      push(ansi.slice(cursor,match.index));cursor=match.index+match[0].length;
      if(match[1]===undefined)continue;
      const values=(match[1]||'0').replaceAll(':',';').split(';').map(Number);
      for(let i=0;i<values.length;i++){
        const n=values[i];
        if(n===0)state={};
        else if(n===1)state.fontWeight='700';
        else if(n===2)state.opacity='0.7';
        else if(n===3)state.fontStyle='italic';
        else if(n===4)state.textDecorationLine='underline';
        else if(n===7)state.inverse=true;
        else if(n===9)state.textDecorationLine='line-through';
        else if(n===22){delete state.fontWeight;delete state.opacity;}
        else if(n===23)delete state.fontStyle;
        else if(n===24||n===29)delete state.textDecorationLine;
        else if(n===27)delete state.inverse;
        else if(n===39)delete state.color;
        else if(n===49)delete state.backgroundColor;
        else if(n>=30&&n<=37)state.color=color(n-30);
        else if(n>=40&&n<=47)state.backgroundColor=color(n-40);
        else if(n>=90&&n<=97)state.color=color(n-90+8);
        else if(n>=100&&n<=107)state.backgroundColor=color(n-100+8);
        else if(n===38||n===48){
          const key=n===38?'color':'backgroundColor',mode=values[++i];
          if(mode===5){const v=values[++i];if(Number.isInteger(v)&&v>=0&&v<=255)state[key]=color(v);}
          else if(mode===2){
            if(match[1].includes(':')&&values[i+1]===0&&values.length-i>=5)i++;
            const rgb=values.slice(i+1,i+4);i+=3;
            if(rgb.length===3&&rgb.every(v=>Number.isInteger(v)&&v>=0&&v<=255))state[key]=`rgb(${rgb.join(',')})`;
          }
        }
      }
    }
    push(ansi.slice(cursor));return runs;
  }
  function render(element,text,ansi) {
    text=String(text??'');ansi=typeof ansi==='string'?ansi:'';
    if(element.terminalText===text&&element.terminalAnsi===ansi&&element.textContent===text)return;
    const runs=ansi?parse(ansi):[{text,start:0,end:text.length,style:{}}],plain=runs.map(run=>run.text).join('');
    const ranges=[];let cursor=0,valid=true;
    for(const [index,line] of text.split('\n').entries()){
      const at=plain.indexOf(line,cursor);
      if(at<0){valid=false;break;}
      if(index)ranges.push({start:-1,end:-1,newline:true});
      ranges.push({start:at,end:at+line.length});cursor=at+line.length+1;
    }
    const chunks=[];
    if(valid){
      let index=0;
      for(const range of ranges){
        if(range.newline){chunks.push({text:'\n',style:{}});continue;}
        while(index<runs.length&&runs[index].end<=range.start)index++;
        for(let j=index;j<runs.length&&runs[j].start<range.end;j++){
          const run=runs[j];chunks.push({text:run.text.slice(Math.max(0,range.start-run.start),range.end-run.start),style:run.style});
        }
      }
    }else chunks.push({text,style:{}});
    for(const [index,chunk] of chunks.entries()){
      let span=element.childNodes[index];
      if(span?.nodeType!==1||span.tagName!=='SPAN'){const replacement=document.createElement('span');if(span)span.replaceWith(replacement);else element.append(replacement);span=replacement;}
      const style={...chunk.style};
      if(style.inverse){delete style.inverse;const fg=style.color||'var(--text, var(--ink))';style.color=style.backgroundColor||'var(--bg)';style.backgroundColor=fg;}
      const signature=JSON.stringify(style);
      if(span.dataset.sgr!==signature){span.removeAttribute('style');Object.assign(span.style,style);span.dataset.sgr=signature;}
      if(span.textContent!==chunk.text){if(span.firstChild?.nodeType===3&&chunk.text.startsWith(span.textContent))span.firstChild.appendData(chunk.text.slice(span.textContent.length));else span.textContent=chunk.text;}
    }
    while(element.childNodes.length>chunks.length)element.lastChild.remove();
    element.terminalText=text;element.terminalAnsi=ansi;
  }
  function renderConsole(element,screen,ansi){
    if(element.consoleSource===screen&&element.consoleSourceAnsi===ansi&&element.textContent===element.terminalText)return false;
    render(element,consoleScreen(screen),ansi);element.consoleSource=screen;element.consoleSourceAnsi=ansi;return true;
  }
function consoleScreen(value){
 const lines=String(value??'').split('\n');
 let last=lines.findLastIndex(line=>line.trim());
 // Only terminal chrome at the screen edges; never remove mentions in messages.
 if(last>=0&&/^\s{2,}(?:GPT-[\w.-]+|gpt-[\w.-]+|o\d[\w.-]*)\b.*(?: · .* · |context left.*(?:shortcuts|agents))/.test(lines[last]))lines.splice(last,1);
 last=lines.findLastIndex(line=>line.trim());
 if(last>=0&&/^\s*[›>]\s*Ask Codex to do anything[.!?]?\s*$/i.test(lines[last]))lines.splice(last,1);
 for(let i=0;i<Math.min(12,lines.length);i++){
  if(!/^\s*[╭┌].*[╮┐]\s*$/.test(lines[i])||!/^\s*[│┃]\s*>_ OpenAI Codex\b/.test(lines[i+1]||''))continue;
  const end=lines.findIndex((line,index)=>index>i&&index<=i+12&&/^\s*[╰└].*[╯┘]\s*$/.test(line));
  if(end>i&&lines.slice(i,end).some(line=>/^\s*[│┃]\s*(?:model|directory):/i.test(line)))lines.splice(i,end-i+1);
  break;
 }
 while(lines.length&&!lines.at(-1).trim())lines.pop();
 while(lines.length&&!lines[0].trim())lines.shift();
 return lines.join('\n');
}
  async function imageData(file){
    if(!file.type.startsWith('image/')||file.size>25*1024*1024)throw Error(OaclisSettings.text('Scegli un’immagine fino a 25 MB.'));
    const bitmap=await createImageBitmap(file,{imageOrientation:'from-image'});
    let result;
    try{
     const ratio=Math.min(1,1600/Math.max(bitmap.width,bitmap.height)),canvas=document.createElement('canvas');
     canvas.width=Math.max(1,Math.round(bitmap.width*ratio));canvas.height=Math.max(1,Math.round(bitmap.height*ratio));
     const context=canvas.getContext('2d');context.fillStyle='#fff';context.fillRect(0,0,canvas.width,canvas.height);context.drawImage(bitmap,0,0,canvas.width,canvas.height);
     for(const quality of [.85,.7,.5]){result=canvas.toDataURL('image/jpeg',quality);if(result.length<=699075)break;}
    }finally{bitmap.close();}
    if(result.length>699075)throw Error();
    return result;
  }
  function attachImages(root,receive){
   const accept=event=>{
    const data=event.clipboardData||event.dataTransfer;
    const files=[...(data?.files||[])];
    if(!files.length)for(const item of data?.items||[]){if(item.kind==='file'){const file=item.getAsFile();if(file)files.push(file);}}
    const image=files.find(file=>file.type.startsWith('image/'));if(!image)return;
    event.preventDefault();event.stopPropagation();receive(image,event.target);
   };
   const drag=event=>{if([...event.dataTransfer?.types||[]].includes('Files')){event.preventDefault();event.stopPropagation();event.dataTransfer.dropEffect='copy';}};
   const drop=event=>{drag(event);accept(event);};
   root.addEventListener('paste',accept);root.addEventListener('dragover',drag);root.addEventListener('drop',drop);
   return ()=>{root.removeEventListener('paste',accept);root.removeEventListener('dragover',drag);root.removeEventListener('drop',drop);};
  }
  function attachSendMenu(form,sendButton,{pickImage,submit,closed}){
 const sendMenu=document.createElement('div');sendMenu.className='console-send-menu';sendMenu.setAttribute('popover','manual');sendMenu.setAttribute('role','menu');sendMenu.setAttribute('aria-label',OaclisSettings.text('Allega o invia'));
 for(const [label,icon,action]of [['Scatta foto','image',()=>pickImage(true)],['Archivio','folder',()=>pickImage(false)],['Invia','arrowUp',()=>submit()]]){
  const button=document.createElement('button');button.type='button';button.setAttribute('role','menuitem');button.innerHTML=OaclisIcon(icon);button.append(document.createTextNode(OaclisSettings.text(label)));button.onclick=()=>{sendMenu.hidePopover();action();};sendMenu.append(button);
 }
 form.append(sendMenu);
 const dismissSendMenu=event=>{
  if(!sendMenu.matches(':popover-open'))return;
  if(event.type==='keydown'?event.key==='Escape':!sendMenu.contains(event.target)&&!sendButton.contains(event.target)){
   sendMenu.hidePopover();if(event.type==='keydown'){event.preventDefault();sendButton.focus({preventScroll:true});}
  }
 };
 document.addEventListener('pointerdown',dismissSendMenu);document.addEventListener('keydown',dismissSendMenu);
 function showSendMenu(choice=0){
  if(sendButton.disabled||closed())return;
  sendMenu.showPopover();sendButton.setAttribute('aria-expanded','true');
  const rect=sendButton.getBoundingClientRect();sendMenu.style.left=Math.max(8,Math.min(innerWidth-sendMenu.offsetWidth-8,rect.right-sendMenu.offsetWidth))+'px';sendMenu.style.top=Math.max(8,rect.top-sendMenu.offsetHeight-8)+'px';
  sendMenu.querySelectorAll('button')[choice].focus({preventScroll:true});
 }
 sendMenu.addEventListener('toggle',event=>sendButton.setAttribute('aria-expanded',String(event.newState==='open')));
 let holdTimer=null,longPressed=false,holdStart;
 const cancelHold=()=>{clearTimeout(holdTimer);holdTimer=null;};
 sendButton.onpointerdown=event=>{if(event.button!==0||sendButton.disabled)return;cancelHold();longPressed=false;holdStart={x:event.clientX,y:event.clientY};holdTimer=setTimeout(()=>{holdTimer=null;longPressed=true;showSendMenu();},500);};
 sendButton.onpointermove=event=>{if(holdStart&&Math.hypot(event.clientX-holdStart.x,event.clientY-holdStart.y)>10)cancelHold();};
 sendButton.onpointerup=sendButton.onpointercancel=sendButton.onpointerleave=cancelHold;
 sendButton.onclick=event=>{if(longPressed){event.preventDefault();event.stopPropagation();longPressed=false;}};
 sendButton.oncontextmenu=event=>{event.preventDefault();cancelHold();longPressed=true;showSendMenu();};
 sendButton.onkeydown=event=>{if(event.key==='ArrowDown'||event.key==='ContextMenu'||event.shiftKey&&event.key==='F10'){event.preventDefault();showSendMenu();}};
 return Object.assign(()=>{document.removeEventListener('pointerdown',dismissSendMenu);document.removeEventListener('keydown',dismissSendMenu);cancelHold();sendMenu.remove();},{show:showSendMenu});
  }
  function chatIcon(chat){
 const icon=document.createElement('span');icon.className='authorized-icon';icon.setAttribute('aria-hidden','true');
 if(typeof chat.icon==='string'&&/^data:image\/png;base64,iVBORw0KGgo[A-Za-z0-9+/=]*$/.test(chat.icon)&&chat.icon.length<=48000){const image=document.createElement('img');image.src=chat.icon;image.alt='';icon.append(image);}
 else if(Object.hasOwn(OaclisIcons,chat.icon))icon.innerHTML=OaclisIcon(chat.icon);
 else icon.textContent=[...chat.name.trim()][0]?.toLocaleUpperCase()||'?';
 if(/^#[a-f0-9]{6}$/i.test(chat.color||''))icon.style.color=chat.color;
 return icon;
}
  function createConsole(){
   const view=document.createElement('section');view.className='oaclis-console';
   view.innerHTML=OaclisSettings.html('<div class="console-panes"></div><form class="terminal-input"><div class="console-draft"><div class="console-attachment" hidden><img alt=""><button type="button" data-console-action="remove-image" aria-label="Rimuovi immagine">'+OaclisIcon('close')+'</button></div><textarea name="text" maxlength="8000" aria-label="Messaggio" rows="2" placeholder="Scrivi un messaggio" disabled></textarea></div><button type="submit" class="console-send icon-button" data-console-action="send" aria-label="Invia messaggio" aria-haspopup="menu" aria-expanded="false" disabled>'+OaclisIcon('arrowUp')+'</button></form><div class="terminal-keys"></div>');
   const keys=view.querySelector('.terminal-keys');
   for(const [key,label]of [['Escape','Esc'],['1','1'],['2','2'],['3','3'],['C-c','Ctrl+C']]){
    const button=document.createElement('button');button.type='button';button.className='icon-button';button.textContent=label;button.dataset.key=key;button.setAttribute('aria-label',label);keys.append(button);
   }
   const voice=document.createElement('span');voice.className='voice-controls';keys.append(voice);
   keys.addEventListener('click',event=>{const key=event.target.closest('button[data-key]')?.dataset.key;if(key)view.activeConsole.send(key==='C-c'?'stop':'input',{key});});
   view.querySelector('form').onsubmit=event=>{event.preventDefault();view.activeConsole.send('input',{text:view.querySelector('textarea').value,key:'Enter'});};
   view.querySelector('textarea').onkeydown=event=>{if(event.key==='Enter'&&!event.shiftKey&&!event.isComposing){event.preventDefault();view.querySelector('form').requestSubmit();}};
   return view;
  }
  function createPane({id,name,icon},transport){
   const pane=document.createElement('article');pane.className='console-pane';pane.setAttribute('aria-label',name);
   pane.innerHTML=OaclisSettings.html('<h2 class="console-title"><span class="console-name"></span></h2><pre class="console-screen" tabindex="0" aria-label="Console">Connessione…</pre>');
   pane.querySelector('.console-name').textContent=name;if(icon)pane.querySelector('h2').prepend(icon);
   const screen=pane.querySelector('pre');
   const chat=pane.console={id,send:transport,follow:true,revision:'',
    render(result,operation='read',saved=null){
     if(operation==='open'||operation==='input')chat.follow=saved?.follow??true;
     const top=saved?.top??screen.scrollTop,left=saved?.left??screen.scrollLeft;
     const changed=result.unchanged===true?false:renderConsole(screen,result.screen,result.screen_ansi);
     if(typeof result.screen_revision==='string')chat.revision=result.screen_revision;
     if(changed||saved||operation!=='read'){if(chat.follow)screen.scrollTop=screen.scrollHeight;else{screen.scrollTop=top;screen.scrollLeft=left;}}
    },
    reset(){chat.revision='';screen.replaceChildren();for(const key of ['consoleSource','consoleSourceAnsi','terminalText','terminalAnsi'])delete screen[key];}
   };
   screen.addEventListener('scroll',()=>{chat.follow=screen.scrollHeight-screen.clientHeight-screen.scrollTop<=24;},{passive:true});
   screen.onkeydown=event=>{
    const key=event.ctrlKey&&event.key.toLowerCase()==='c'&&!String(window.getSelection())?'C-c':({ArrowUp:'Up',ArrowDown:'Down',ArrowLeft:'Left',ArrowRight:'Right',Enter:'Enter',Tab:'Tab',Backspace:'BSpace'})[event.key];
    if(key){event.preventDefault();chat.send(key==='C-c'?'stop':'input',{key});}
   };
   return pane;
  }
  return {render,renderConsole,parse,consoleScreen,imageData,attachImages,attachSendMenu,chatIcon,createConsole,createPane};
})();
