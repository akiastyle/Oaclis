// badge-motion.js v1.7.139
/* Passive presentation only: no network, storage, timers for progress or transfer decisions. */
(function () {
  const names = ['chat', 'paths', 'apps'];
  class BadgeMotion {
    constructor(host, symbols) {
      this.host = host; this.phase = null; this.signature = null; this.active = false;
      host.classList.add('motion-badge');
      const style = getComputedStyle(host);
      const radius = (parseFloat(style.getPropertyValue('--link-badge-size')) || 44) / 2;
      const gap = parseFloat(style.getPropertyValue('--badge-ring-gap')) || 2;
      const width = parseFloat(style.getPropertyValue('--badge-ring-width')) || 2;
      this.surface = document.createElement('div'); this.surface.className = 'badge-motion';
      this.surface.innerHTML = '<svg class="badge-gauges" viewBox="0 0 120 120" aria-hidden="true">' + names.map((name, index) => {
        const r = radius + gap + width / 2 + index * (gap + width);
        return `<g class="badge-ring" data-module="${name}"><circle class="badge-track" cx="60" cy="60" r="${r}"/><circle class="badge-fill" cx="60" cy="60" r="${r}" pathLength="100" stroke-dasharray="100" stroke-dashoffset="100"/></g>`;
      }).join('') + '</svg><svg class="badge-check" viewBox="0 0 120 120" aria-hidden="true"><path d="M109.5 46.7 A51.3 51.3 0 0 1 109.5 73.3"/></svg><div class="badge-modules">' + names.map(name => `<span class="badge-module" data-module="${name}" title="${{chat:'Chat',paths:'Percorsi',apps:'App'}[name]}"><svg viewBox="0 0 24 24" aria-hidden="true">${symbols[name]}</svg></span>`).join('') + '</div>';
      host.prepend(this.surface);
      this.rings = Object.fromEntries(names.map(name => [name, this.surface.querySelector('.badge-ring[data-module="' + name + '"]')]));
      this.icons = Object.fromEntries(names.map(name => [name, this.surface.querySelector('.badge-module[data-module="' + name + '"]')]));
      names.forEach(name => this.progress(name, 0));
      this.surface.addEventListener('animationend',event=>{
        if(event.animationName==='badge-contract'&&event.target===this.rings.chat&&this.phase==='finishing'){this.active=false;this.setPhase(this.restPhase);}
      });
      this.entrance = requestAnimationFrame(() => { if (!this.phase) this.setPhase('idle'); });
    }
    setPhase(phase) {
      if (phase === this.phase) return;
      this.phase = phase; this.host.dataset.motionPhase = phase;
      const hidden = ['transferring', 'paused', 'finishing'].includes(phase);
      this.host.querySelectorAll('.link-edit,.link-open,.link-web').forEach(button => { button.disabled = hidden; button.inert = hidden; button.setAttribute('aria-hidden', String(hidden)); });
    }
    progress(name, progress) {
      if (!Number.isFinite(progress)) return;
      this.host.style.setProperty('--badge-'+name+'-progress',Math.min(1,Math.max(0,progress)));
    }
    update(state) {
      const signature = JSON.stringify(state);
      if (signature === this.signature) return;
      this.signature = signature;
      const working = ['transferring', 'paused'].includes(state.phase);
      if (working) {
        names.forEach(name => {
          const ring=this.rings[name],icon=this.icons[name];
          if(!this.active||ring.classList.contains('is-leaving')) {
            ring.classList.remove('is-leaving'); ring.classList.add('is-active'); icon.classList.add('is-active');
          }
          this.progress(name, state.modules?.[name]?.changed === true ? state.modules[name].progress : 1);
        });
        this.active = true; this.setPhase(state.phase);
      } else if (this.active) {
        names.forEach(name => this.progress(name, state.modules?.[name]?.changed === true ? state.modules[name].progress : 1));
        this.restPhase = state.phase === 'checking' ? 'checking' : 'idle';
        if (this.phase === 'finishing') return;
        this.setPhase('finishing');
        names.forEach(name=>{
          this.rings[name].classList.replace('is-active','is-leaving');this.icons[name].classList.remove('is-active');
        });
      } else this.setPhase(state.phase === 'checking' ? 'checking' : 'idle');
    }
    destroy() { cancelAnimationFrame(this.entrance); this.surface.remove(); }
  }
  window.OaclisBadgeMotion = BadgeMotion;
})();
