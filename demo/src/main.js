import init, { parseGpx, parseGpxFull } from "../pkg/navigo.js";

// ── Bootstrap ─────────────────────────────────────────────────────────────────

await init();

const response = await fetch("/grp-160-2026.gpx");
const buffer = await response.arrayBuffer();
const bytes = new Uint8Array(buffer);

const trace = parseGpx(bytes);
const full = parseGpxFull(bytes, 500, 0.002, 3600);

if (!trace || !full) {
  document.body.innerHTML =
    '<p style="padding:2rem;font-family:monospace">Failed to parse GPX.</p>';
  throw new Error("GPX parse failed");
}

// Cache all bulk arrays once — each getter copies WASM→JS heap.
const locsFlat = trace.locations_flat;
const dists = trace.cumulative_distances;
const peaks = trace.peaks;
const valleys = trace.valleys;
const climbs = trace.climbs();
const area = trace.area();

// ── Header ────────────────────────────────────────────────────────────────────

setText("race-title", full.metadata.name || "");

// ── Stats cards ───────────────────────────────────────────────────────────────

setText("stat-distance", full.trace.total_distance_km.toFixed(1) + " km");
setText(
  "stat-gain",
  "+" + Math.round(full.trace.total_elevation_gain_m) + " m",
);
setText(
  "stat-loss",
  "−" + Math.round(full.trace.total_elevation_loss_m) + " m",
);
setText("stat-locations", full.trace.location_count.toString());
setText("stat-climbs", climbs.length.toString());

if (area) {
  setText(
    "stat-bbox",
    `${area.min_latitude.toFixed(3)}°N – ${area.max_latitude.toFixed(3)}°N, ` +
      `${area.min_longitude.toFixed(3)}°E – ${area.max_longitude.toFixed(3)}°E`,
  );
}

// ── Elevation profile ─────────────────────────────────────────────────────────

const canvas = document.getElementById("profile");
drawProfile(canvas, locsFlat, dists, peaks, valleys, climbs);
window.addEventListener("resize", () =>
  drawProfile(canvas, locsFlat, dists, peaks, valleys, climbs),
);

// ── Climbs list ───────────────────────────────────────────────────────────────

renderClimbs(climbs);

// ── Checkpoints ───────────────────────────────────────────────────────────────

renderWaypoints(full.waypoints);

// ── Sections ─────────────────────────────────────────────────────────────────

renderSections(full.sections, full.waypoints);

// ── Stages ───────────────────────────────────────────────────────────────────

renderStages(full.stages, full.waypoints);

// ── Free WASM memory — all data already copied to JS ─────────────────────────
trace.free();

// ── Helpers ───────────────────────────────────────────────────────────────────

function setText(id, text) {
  const el = document.getElementById(id);
  if (el) el.textContent = text;
}

function formatDuration(totalSeconds) {
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  return `${hours}h ${String(minutes).padStart(2, "0")}m`;
}

function formatTimestamp(unixSeconds) {
  if (unixSeconds == null) return "—";
  const date = new Date(unixSeconds * 1000);
  const days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
  const hh = String(date.getUTCHours()).padStart(2, "0");
  const mm = String(date.getUTCMinutes()).padStart(2, "0");
  return `${days[date.getUTCDay()]} ${hh}:${mm}`;
}

function typeLabel(wptType) {
  const labels = {
    Start: "START",
    Arrival: "FINISH",
    LifeBase: "LIFE BASE",
    TimeBarrier: "BARRIER",
  };
  return labels[wptType] || wptType || "—";
}

function typeClass(wptType) {
  const classes = {
    Start: "type-start",
    Arrival: "type-arrival",
    LifeBase: "type-lifebase",
    TimeBarrier: "type-barrier",
  };
  return classes[wptType] || "";
}

function stars(difficulty) {
  return "★".repeat(difficulty) + "☆".repeat(5 - difficulty);
}

function renderWaypoints(waypoints) {
  const el = document.getElementById("waypoints-list");
  if (!el) return;
  if (!waypoints || waypoints.length === 0) {
    el.innerHTML = '<p class="empty">NO CHECKPOINTS FOUND.</p>';
    return;
  }
  el.innerHTML = `
    <table class="data-table">
      <thead>
        <tr>
          <th>#</th>
          <th>Name</th>
          <th>Type</th>
          <th>Elevation</th>
          <th>Cutoff (UTC)</th>
        </tr>
      </thead>
      <tbody>
        ${waypoints
          .map(
            (w, i) => `
          <tr>
            <td class="num">${String(i + 1).padStart(2, "0")}</td>
            <td>${w.name}</td>
            <td><span class="type-badge ${typeClass(w.wpt_type)}">${typeLabel(w.wpt_type)}</span></td>
            <td class="num">${w.elevation != null ? Math.round(w.elevation) + " m" : "—"}</td>
            <td class="num">${formatTimestamp(w.time)}</td>
          </tr>`,
          )
          .join("")}
      </tbody>
    </table>`;
}

function renderSections(sections, waypoints) {
  const el = document.getElementById("sections-list");
  if (!el) return;
  if (!sections || sections.length === 0) {
    el.innerHTML = '<p class="empty">NO SECTIONS FOUND.</p>';
    return;
  }

  let cumulativeKm = 0;
  el.innerHTML = `
    <table class="data-table">
      <thead>
        <tr>
          <th>#</th>
          <th>From → To</th>
          <th>Dist.</th>
          <th>Cum.</th>
          <th>Gain</th>
          <th>Loss</th>
          <th>Est. time</th>
          <th>Difficulty</th>
        </tr>
      </thead>
      <tbody>
        ${sections
          .map((s) => {
            cumulativeKm += s.total_distance_km;
            const fromName = waypoints[s.id]?.name ?? "—";
            const toName = waypoints[s.id + 1]?.name ?? "—";
            return `
          <tr>
            <td class="num">${String(s.id + 1).padStart(2, "0")}</td>
            <td class="leg-label">${fromName} → ${toName}</td>
            <td class="num">${s.total_distance_km.toFixed(1)} km</td>
            <td class="num muted">${cumulativeKm.toFixed(1)} km</td>
            <td class="num gain">+${Math.round(s.total_elevation_gain_m)} m</td>
            <td class="num loss">−${Math.round(s.total_elevation_loss_m)} m</td>
            <td class="num">${formatDuration(s.estimated_duration_s)}</td>
            <td class="num stars">${stars(s.difficulty)}</td>
          </tr>`;
          })
          .join("")}
      </tbody>
    </table>`;
}

function renderStages(stages, waypoints) {
  const el = document.getElementById("stages-list");
  if (!el) return;
  if (!stages || stages.length === 0) {
    el.innerHTML = '<p class="empty">NO STAGES FOUND.</p>';
    return;
  }

  const stageBoundaries = waypoints
    ? waypoints.filter((w) =>
        ["Start", "LifeBase", "Arrival"].includes(w.wpt_type),
      )
    : [];

  el.innerHTML = `
    <table class="data-table">
      <thead>
        <tr>
          <th>Stage</th>
          <th>From → To</th>
          <th>Distance</th>
          <th>Gain</th>
          <th>Loss</th>
          <th>Est. time</th>
          <th>Difficulty</th>
        </tr>
      </thead>
      <tbody>
        ${stages
          .map((s) => {
            const fromName = stageBoundaries[s.id]?.name ?? "—";
            const toName = stageBoundaries[s.id + 1]?.name ?? "—";
            return `
          <tr>
            <td class="num">${String(s.id + 1).padStart(2, "0")}</td>
            <td class="leg-label">${fromName} → ${toName}</td>
            <td class="num">${s.total_distance_km.toFixed(1)} km</td>
            <td class="num gain">+${Math.round(s.total_elevation_gain_m)} m</td>
            <td class="num loss">−${Math.round(s.total_elevation_loss_m)} m</td>
            <td class="num">${formatDuration(s.estimated_duration_s)}</td>
            <td class="num stars">${stars(s.difficulty)}</td>
          </tr>`;
          })
          .join("")}
      </tbody>
    </table>`;
}

function drawProfile(canvas, locsFlat, dists, peaks, valleys, climbs) {
  const DPR = window.devicePixelRatio || 1;
  const W = canvas.parentElement.clientWidth;
  const H = 260;
  canvas.style.width = W + "px";
  canvas.style.height = H + "px";
  canvas.width = W * DPR;
  canvas.height = H * DPR;

  const ctx = canvas.getContext("2d");
  ctx.scale(DPR, DPR);

  const PAD = { top: 20, right: 20, bottom: 40, left: 64 };
  const CW = W - PAD.left - PAD.right;
  const CH = H - PAD.top - PAD.bottom;

  // Extract altitudes
  const alts = [];
  for (let i = 2; i < locsFlat.length; i += 3) alts.push(locsFlat[i]);

  const maxDist = dists[dists.length - 1];
  const minAlt = Math.min(...alts) - 80;
  const maxAlt = Math.max(...alts) + 80;

  const xP = (d) => PAD.left + (d / maxDist) * CW;
  const yP = (a) => PAD.top + (1 - (a - minAlt) / (maxAlt - minAlt)) * CH;

  // ── White background
  ctx.fillStyle = "#fff";
  ctx.fillRect(0, 0, W, H);

  // ── Yellow climb zones (behind everything)
  climbs.forEach((c) => {
    ctx.fillStyle = "#ffe000";
    const x1 = xP(c.start_dist_km);
    const x2 = xP(c.start_dist_km + c.climb_dist_km);
    ctx.fillRect(x1, PAD.top, x2 - x1, CH);
  });

  // ── Horizontal gridlines
  const altStep = 200;
  ctx.strokeStyle = "rgba(0,0,0,0.08)";
  ctx.lineWidth = 1;
  for (
    let a = Math.ceil(minAlt / altStep) * altStep;
    a <= maxAlt;
    a += altStep
  ) {
    const y = yP(a);
    if (y < PAD.top || y > H - PAD.bottom) continue;
    ctx.beginPath();
    ctx.moveTo(PAD.left, y);
    ctx.lineTo(W - PAD.right, y);
    ctx.stroke();
  }

  // ── Solid black elevation fill
  ctx.beginPath();
  ctx.moveTo(xP(dists[0]), yP(alts[0]));
  for (let i = 1; i < dists.length; i++) ctx.lineTo(xP(dists[i]), yP(alts[i]));
  ctx.lineTo(xP(maxDist), H - PAD.bottom);
  ctx.lineTo(xP(0), H - PAD.bottom);
  ctx.closePath();
  ctx.fillStyle = "#000";
  ctx.fill();

  // ── Valley dots (white circle, black outline)
  Array.from(valleys).forEach((idx) => {
    ctx.beginPath();
    ctx.arc(xP(dists[idx]), yP(alts[idx]), 5, 0, Math.PI * 2);
    ctx.fillStyle = "#fff";
    ctx.strokeStyle = "#000";
    ctx.lineWidth = 2;
    ctx.fill();
    ctx.stroke();
  });

  // ── Peak dots (solid red)
  Array.from(peaks).forEach((idx) => {
    ctx.beginPath();
    ctx.arc(xP(dists[idx]), yP(alts[idx]), 5, 0, Math.PI * 2);
    ctx.fillStyle = "#ff2800";
    ctx.fill();
  });

  // ── Axes (thick black)
  ctx.strokeStyle = "#000";
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.moveTo(PAD.left, PAD.top);
  ctx.lineTo(PAD.left, H - PAD.bottom);
  ctx.lineTo(W - PAD.right, H - PAD.bottom);
  ctx.stroke();

  // ── Y labels + ticks
  ctx.fillStyle = "#000";
  ctx.font = "bold 10px Courier New, monospace";
  ctx.textAlign = "right";
  ctx.textBaseline = "middle";
  for (
    let a = Math.ceil(minAlt / altStep) * altStep;
    a <= maxAlt;
    a += altStep
  ) {
    const y = yP(a);
    if (y < PAD.top || y > H - PAD.bottom) continue;
    ctx.fillText(a + "m", PAD.left - 6, y);
    ctx.beginPath();
    ctx.moveTo(PAD.left - 4, y);
    ctx.lineTo(PAD.left, y);
    ctx.lineWidth = 1.5;
    ctx.stroke();
  }

  // ── X labels + ticks — step chosen so labels never overlap
  const minTickSpacingPx = 45;
  const rawStepKm = (minTickSpacingPx / CW) * maxDist;
  const niceSteps = [1, 2, 5, 10, 20, 25, 50, 100];
  const tickStepKm = niceSteps.find((s) => s >= rawStepKm) ?? 100;

  ctx.textAlign = "center";
  ctx.textBaseline = "top";
  for (let d = 0; d <= maxDist; d += tickStepKm) {
    const x = xP(d);
    ctx.fillText(d + "km", x, H - PAD.bottom + 6);
    ctx.beginPath();
    ctx.moveTo(x, H - PAD.bottom);
    ctx.lineTo(x, H - PAD.bottom + 4);
    ctx.lineWidth = 1.5;
    ctx.stroke();
  }

  // ── Climb gradient labels (on the yellow bands, above the black fill)
  ctx.textAlign = "center";
  ctx.textBaseline = "bottom";
  ctx.font = "bold 10px Courier New, monospace";
  climbs.forEach((c) => {
    const midX = xP(c.start_dist_km + c.climb_dist_km / 2);
    ctx.fillStyle = "#000";
    ctx.fillText(`↑ ${c.avg_gradient.toFixed(1)}%`, midX, H - PAD.bottom - 4);
  });
}

function renderClimbs(climbs) {
  const el = document.getElementById("climbs-list");
  if (!el) return;

  if (climbs.length === 0) {
    el.innerHTML = '<p class="empty">NO QUALIFYING CLIMBS DETECTED.</p>';
    return;
  }

  el.innerHTML = climbs
    .map(
      (c, i) => `
    <div class="climb-card">
      <div class="climb-rank">#${String(i + 1).padStart(2, "0")}</div>
      <div class="climb-metrics">
        <div class="metric">
          <span class="label">dist</span>
          <span class="val">${c.climb_dist_km.toFixed(1)}<small>km</small></span>
        </div>
        <div class="metric">
          <span class="label">gain</span>
          <span class="val">+${Math.round(c.elevation_gain)}<small>m</small></span>
        </div>
        <div class="metric">
          <span class="label">grade</span>
          <span class="val">${c.avg_gradient.toFixed(1)}<small>%</small></span>
        </div>
        <div class="metric">
          <span class="label">summit</span>
          <span class="val">${Math.round(c.summit_elev)}<small>m</small></span>
        </div>
        <div class="metric score">
          <span class="label">score</span>
          <span class="val">${(c.climb_dist_km * c.avg_gradient).toFixed(0)}</span>
        </div>
      </div>
    </div>
  `,
    )
    .join("");
}
