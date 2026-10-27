// background.js v1.2.65
"use strict";
(() => {
  const canvas = document.querySelector('#map-particles');
  const context = canvas?.getContext('2d', { alpha: true });
  if (!context) return;
  const reduced = matchMedia('(prefers-reduced-motion: reduce)');
  let particles = [], colors = [], opacity = 0, width = 0, height = 0;
  let frame = 0, previous = 0, elapsed = 0;

  function paint(now) {
    frame = 0;
    if (document.hidden) return;
    // Thirty frames per second are enough for this slow, peripheral movement.
    if (!previous || now - previous >= 1000 / 30) {
      const dt = previous ? Math.min(0.1, (now - previous) / 1000) : 0;
      previous = now;
      if (!reduced.matches) elapsed += dt;
      context.clearRect(0, 0, width, height);
      for (const particle of particles) {
        if (!reduced.matches) particle.distance += dt * particle.speed;
        if (particle.distance > 1.4) particle.distance = 0.45;
        const distance = particle.distance;
        const angle = particle.angle + Math.sin(elapsed * 0.12 + particle.angle) * 0.025;
        const x = width * (0.5 + Math.cos(angle) * distance * 0.65);
        const y = height * (0.5 + Math.sin(angle) * distance * 0.65);
        context.globalAlpha = opacity * Math.min(1, (distance - 0.45) / 0.18, (1.4 - distance) / 0.25);
        context.fillStyle = colors[particle.color];
        context.beginPath();
        context.arc(x, y, particle.size, 0, Math.PI * 2);
        context.fill();
      }
    }
    if (!reduced.matches) frame = requestAnimationFrame(paint);
  }

  function refresh() {
    cancelAnimationFrame(frame);
    frame = 0;
    const style = getComputedStyle(document.documentElement);
    colors = [style.getPropertyValue('--particle-blue').trim(), style.getPropertyValue('--particle-cream').trim()];
    opacity = parseFloat(style.getPropertyValue('--particle-opacity'));
    width = canvas.clientWidth; height = canvas.clientHeight;
    // Cap the backing surface, including on high-DPI and large desktop screens.
    const scale = Math.min(devicePixelRatio || 1, 1.5, Math.sqrt(1400000 / Math.max(1, width * height)));
    canvas.width = Math.max(1, Math.round(width * scale));
    canvas.height = Math.max(1, Math.round(height * scale));
    context.setTransform(scale, 0, 0, scale, 0, 0);
    const count = width < 700 ? 32 : 64;
    if (particles.length !== count) particles = Array.from({ length: count }, () => ({
      angle: Math.random() * Math.PI * 2,
      distance: 0.45 + Math.random() * 0.95,
      speed: 0.012 + Math.random() * 0.025,
      size: 0.8 + Math.random() * 1.6,
      color: Math.random() < 0.55 ? 0 : 1,
    }));
    previous = 0;
    if (!document.hidden) frame = requestAnimationFrame(paint);
  }

  addEventListener('resize', refresh);
  document.addEventListener('visibilitychange', refresh);
  reduced.addEventListener('change', refresh);
  new MutationObserver(refresh).observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
  refresh();
})();
