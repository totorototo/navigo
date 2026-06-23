import init, { buildTrace } from '../pkg/navigo.js';
import { generateAlpineRoute } from './sample-trace.js';

// ── Bootstrap ─────────────────────────────────────────────────────────────────

await init();

const pts   = generateAlpineRoute();
const trace = buildTrace(pts);

// Cache all bulk arrays once — each getter copies WASM→JS heap.
const locsFlat = trace.locations_flat;
const dists    = trace.cumulative_distances;
const gains    = trace.cumulative_elevation_gains;
const losses   = trace.cumulative_elevation_losses;
const peaks    = trace.peaks;
const valleys  = trace.valleys;
const climbs   = trace.climbs();
const area     = trace.area();

// ── Stats cards ───────────────────────────────────────────────────────────────

setText('stat-distance',    trace.total_distance.toFixed(1) + ' km');
setText('stat-gain',       '+' + Math.round(trace.total_elevation_gain) + ' m');
setText('stat-loss',       '−' + Math.round(trace.total_elevation_loss) + ' m');
setText('stat-locations',   trace.location_count.toString());
setText('stat-climbs',      climbs.length.toString());

if (area) {
  setText('stat-bbox',
    `${area.min_latitude.toFixed(3)}°N – ${area.max_latitude.toFixed(3)}°N, ` +
    `${area.min_longitude.toFixed(3)}°E – ${area.max_longitude.toFixed(3)}°E`
  );
}

// ── Elevation profile ─────────────────────────────────────────────────────────

const canvas = document.getElementById('profile');
drawProfile(canvas, locsFlat, dists, peaks, valleys, climbs);
window.addEventListener('resize', () =>
  drawProfile(canvas, locsFlat, dists, peaks, valleys, climbs)
);

// ── Climbs list ───────────────────────────────────────────────────────────────

renderClimbs(climbs);

// ── Free WASM memory — all data already copied to JS ─────────────────────────
trace.free();

// ── Helpers ───────────────────────────────────────────────────────────────────

function setText(id, text) {
  const el = document.getElementById(id);
  if (el) el.textContent = text;
}

function drawProfile(canvas, locsFlat, dists, peaks, valleys, climbs) {
  const DPR  = window.devicePixelRatio || 1;
  const W    = canvas.parentElement.clientWidth;
  const H    = 240;
  canvas.style.width  = W + 'px';
  canvas.style.height = H + 'px';
  canvas.width  = W * DPR;
  canvas.height = H * DPR;

  const ctx = canvas.getContext('2d');
  ctx.scale(DPR, DPR);

  const PAD = { top: 16, right: 16, bottom: 36, left: 56 };
  const CW  = W - PAD.left - PAD.right;
  const CH  = H - PAD.top  - PAD.bottom;

  // Extract altitudes
  const alts = [];
  for (let i = 2; i < locsFlat.length; i += 3) alts.push(locsFlat[i]);

  const maxDist = dists[dists.length - 1];
  const minAlt  = Math.min(...alts) - 80;
  const maxAlt  = Math.max(...alts) + 80;

  const xP = d => PAD.left + (d / maxDist) * CW;
  const yP = a => PAD.top  + (1 - (a - minAlt) / (maxAlt - minAlt)) * CH;

  // ── Background
  ctx.fillStyle = '#111827';
  ctx.fillRect(0, 0, W, H);

  // ── Gridlines
  ctx.strokeStyle = 'rgba(255,255,255,0.06)';
  ctx.lineWidth   = 1;
  const altStep   = 200;
  for (let a = Math.ceil(minAlt / altStep) * altStep; a <= maxAlt; a += altStep) {
    const y = yP(a);
    if (y < PAD.top || y > H - PAD.bottom) continue;
    ctx.beginPath();
    ctx.moveTo(PAD.left, y);
    ctx.lineTo(W - PAD.right, y);
    ctx.stroke();
  }

  // ── Climb zones
  const zoneColors = ['rgba(239,68,68,0.12)', 'rgba(245,158,11,0.12)', 'rgba(34,197,94,0.12)'];
  climbs.forEach((c, i) => {
    ctx.fillStyle = zoneColors[i % zoneColors.length];
    const x1 = xP(c.start_dist_km);
    const x2 = xP(c.start_dist_km + c.climb_dist_km);
    ctx.fillRect(x1, PAD.top, x2 - x1, CH);
  });

  // ── Elevation fill
  const grad = ctx.createLinearGradient(0, PAD.top, 0, H - PAD.bottom);
  grad.addColorStop(0,   'rgba(99,102,241,0.55)');
  grad.addColorStop(1,   'rgba(99,102,241,0.04)');

  ctx.beginPath();
  ctx.moveTo(xP(dists[0]), yP(alts[0]));
  for (let i = 1; i < dists.length; i++) ctx.lineTo(xP(dists[i]), yP(alts[i]));
  ctx.lineTo(xP(maxDist), H - PAD.bottom);
  ctx.lineTo(xP(0),       H - PAD.bottom);
  ctx.closePath();
  ctx.fillStyle = grad;
  ctx.fill();

  // ── Elevation line
  ctx.beginPath();
  ctx.moveTo(xP(dists[0]), yP(alts[0]));
  for (let i = 1; i < dists.length; i++) ctx.lineTo(xP(dists[i]), yP(alts[i]));
  ctx.strokeStyle = '#818cf8';
  ctx.lineWidth   = 2;
  ctx.lineJoin    = 'round';
  ctx.stroke();

  // ── Valley dots
  Array.from(valleys).forEach(idx => {
    dot(ctx, xP(dists[idx]), yP(alts[idx]), '#60a5fa', 4);
  });

  // ── Peak dots
  Array.from(peaks).forEach(idx => {
    dot(ctx, xP(dists[idx]), yP(alts[idx]), '#f87171', 4);
  });

  // ── Axes
  ctx.strokeStyle = 'rgba(255,255,255,0.15)';
  ctx.lineWidth   = 1;
  ctx.beginPath();
  ctx.moveTo(PAD.left, PAD.top);
  ctx.lineTo(PAD.left, H - PAD.bottom);
  ctx.lineTo(W - PAD.right, H - PAD.bottom);
  ctx.stroke();

  // ── Y labels
  ctx.fillStyle  = '#6b7280';
  ctx.font       = '11px system-ui, sans-serif';
  ctx.textAlign  = 'right';
  ctx.textBaseline = 'middle';
  for (let a = Math.ceil(minAlt / altStep) * altStep; a <= maxAlt; a += altStep) {
    const y = yP(a);
    if (y < PAD.top || y > H - PAD.bottom) continue;
    ctx.fillText(a + ' m', PAD.left - 6, y);
  }

  // ── X labels
  ctx.textAlign   = 'center';
  ctx.textBaseline = 'top';
  const distStep = 5;
  for (let d = 0; d <= maxDist; d += distStep) {
    ctx.fillText(d + ' km', xP(d), H - PAD.bottom + 6);
  }

  // ── Climb number labels
  ctx.textAlign  = 'center';
  ctx.textBaseline = 'top';
  const labelColors = ['#f87171', '#fbbf24', '#4ade80'];
  climbs.forEach((c, i) => {
    const midX = xP(c.start_dist_km + c.climb_dist_km / 2);
    ctx.fillStyle = labelColors[i % labelColors.length];
    ctx.font = 'bold 11px system-ui, sans-serif';
    ctx.fillText(`↑ ${c.avg_gradient.toFixed(1)}%`, midX, PAD.top + 4);
  });
}

function dot(ctx, x, y, color, r) {
  ctx.beginPath();
  ctx.arc(x, y, r, 0, Math.PI * 2);
  ctx.fillStyle = color;
  ctx.fill();
}

function renderClimbs(climbs) {
  const el = document.getElementById('climbs-list');
  if (!el) return;

  if (climbs.length === 0) {
    el.innerHTML = '<p class="empty">No qualifying climbs detected.</p>';
    return;
  }

  const icons = ['🔴', '🟡', '🟢'];
  el.innerHTML = climbs.map((c, i) => `
    <div class="climb-card">
      <div class="climb-rank">${icons[i % icons.length]} Climb ${i + 1}</div>
      <div class="climb-metrics">
        <span class="metric"><span class="label">dist</span>${c.climb_dist_km.toFixed(1)} km</span>
        <span class="metric"><span class="label">gain</span>+${Math.round(c.elevation_gain)} m</span>
        <span class="metric"><span class="label">grade</span>${c.avg_gradient.toFixed(1)}%</span>
        <span class="metric"><span class="label">summit</span>${Math.round(c.summit_elev)} m</span>
        <span class="metric"><span class="label">score</span>${(c.climb_dist_km * c.avg_gradient).toFixed(0)}</span>
      </div>
    </div>
  `).join('');
}
