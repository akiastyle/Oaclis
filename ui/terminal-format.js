// terminal-format.js v1.4.84
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
  return {render,parse};
})();
