// badge-motion.js v1.6.97
/* Passive presentation only: no network, storage, timers for progress or transfer decisions. */
(function () {
  const names = ['chat', 'paths', 'apps'];
  class BadgeMotion {
    constructor(host, symbols) {
      this.host = host; this.phase = null; this.signature = null; this.active = [];
      host.classList.add('motion-badge');
      const style = getComputedStyle(host);
      const milliseconds = (value, fallback) => value.trim() ? parseFloat(value) * (value.trim().endsWith('ms') ? 1 : 1000) : fallback;
      this.duration = milliseconds(style.getPropertyValue('--duration-ui'), 500);
      this.stagger = milliseconds(style.getPropertyValue('--badge-stagger'), 300);
      const radius = (parseFloat(style.getPropertyValue('--link-badge-size')) || 44) / 2;
      const gap = parseFloat(style.getPropertyValue('--badge-ring-gap')) || 2;
      const width = parseFloat(style.getPropertyValue('--badge-ring-width')) || 2;
      const orange = style.getPropertyValue('--badge-progress-start').trim() || '#ee6c4d';
      const green = style.getPropertyValue('--badge-progress-end').trim() || '#22c55e';
      this.colors = [orange, green].map(hex => hex.replace('#', '').match(/.{2}/g).map(value => parseInt(value, 16)));
      this.surface = document.createElement('div'); this.surface.className = 'badge-motion';
      this.surface.innerHTML = '<svg class="badge-gauges" viewBox="0 0 120 120" aria-hidden="true">' + names.map((name, index) => {
        const r = radius + gap + width / 2 + index * (gap + width);
        return `<g class="badge-ring" data-module="${name}"><circle class="badge-track" cx="60" cy="60" r="${r}"/><circle class="badge-fill" cx="60" cy="60" r="${r}" pathLength="100" stroke-dasharray="100" stroke-dashoffset="100"/></g>`;
      }).join('') + '</svg><svg class="badge-check" viewBox="0 0 120 120" aria-hidden="true"><path d="M109.5 46.7 A51.3 51.3 0 0 1 109.5 73.3"/></svg><div class="badge-modules">' + names.map(name => `<span class="badge-module" data-module="${name}" title="${{chat:'Chat',paths:'Percorsi',apps:'App'}[name]}"><svg viewBox="0 0 24 24" aria-hidden="true">${symbols[name]}</svg></span>`).join('') + '</div>';
      host.prepend(this.surface);
      this.rings = Object.fromEntries(names.map(name => [name, this.surface.querySelector('.badge-ring[data-module="' + name + '"]')]));
      this.icons = Object.fromEntries(names.map(name => [name, this.surface.querySelector('.badge-module[data-module="' + name + '"]')]));
      names.forEach(name => this.progress(name, 0));
      this.entrance = requestAnimationFrame(() => { if (!this.phase) this.setPhase('idle'); });
    }
    setPhase(phase) {
      if (phase === this.phase) return;
      this.phase = phase; this.host.dataset.motionPhase = phase;
      const hidden = ['transferring', 'paused', 'finishing'].includes(phase);
      this.host.querySelectorAll('.link-edit,.link-open').forEach(button => { button.disabled = hidden; button.setAttribute('aria-hidden', String(hidden)); });
    }
    progress(name, progress) {
      if (!Number.isFinite(progress)) return;
      const value = Math.min(1, Math.max(0, progress)), fill = this.rings[name].querySelector('.badge-fill');
      const color = 'rgb(' + this.colors[0].map((v, i) => Math.round(v + (this.colors[1][i] - v) * value)).join(',') + ')';
      fill.setAttribute("stroke-dashoffset", 100 * (1 - value)); fill.setAttribute("stroke", color);
      this.icons[name].style.color = color;
    }
    update(state) {
      const signature = JSON.stringify(state);
      if (signature === this.signature) return;
      this.signature = signature;
      const working = ['transferring', 'paused'].includes(state.phase);
      if (working) {
        clearTimeout(this.exitTimer);
        const active = names;
        names.forEach(name => {
          const index = active.indexOf(name), ring = this.rings[name], icon = this.icons[name];
          if (index < 0) { ring.classList.remove('is-active', 'is-leaving'); icon.classList.remove('is-active'); return; }
          if (!this.active.includes(name) || ring.classList.contains('is-leaving')) {
            ring.style.setProperty('--badge-delay', (index * this.stagger) + 'ms');
            icon.style.setProperty('--badge-delay', (index * this.stagger) + 'ms');
            ring.classList.remove('is-leaving'); ring.classList.add('is-active'); icon.classList.add('is-active');
          }
          this.progress(name, state.modules?.[name]?.changed === true ? state.modules[name].progress : 1);
        });
        this.active = active; this.setPhase(state.phase);
      } else if (this.active.length) {
        this.active.forEach(name => this.progress(name, state.modules?.[name]?.changed === true ? state.modules[name].progress : 1));
        this.restPhase = state.phase === 'checking' ? 'checking' : 'idle';
        if (this.phase === 'finishing') return;
        this.setPhase('finishing');
        this.active.slice().reverse().forEach((name, index) => {
          this.rings[name].style.setProperty('--badge-delay', index * this.stagger + 'ms');
          this.icons[name].style.setProperty('--badge-delay', index * this.stagger + 'ms');
          this.rings[name].classList.replace('is-active', 'is-leaving'); this.icons[name].classList.remove('is-active');
        });
        this.exitTimer = setTimeout(() => { this.active = []; this.setPhase(this.restPhase); }, this.duration + (this.active.length - 1) * this.stagger);
      } else this.setPhase(state.phase === 'checking' ? 'checking' : 'idle');
    }
    destroy() { clearTimeout(this.exitTimer); cancelAnimationFrame(this.entrance); this.surface.remove(); }
  }
  window.OaclisBadgeMotion = BadgeMotion;
})();
