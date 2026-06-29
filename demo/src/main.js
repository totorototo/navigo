import init, { parseGpx, analyzeGpx } from "../pkg/navigo.js";

// ── Bootstrap ─────────────────────────────────────────────────────────────────

await init();

const response = await fetch("/grp-160-2026.gpx");
const buffer = await response.arrayBuffer();
const bytes = new Uint8Array(buffer);

let trace;
let full;

try {
  // parseGpx — lean path: only <trkpt> track-points are parsed.
  // Use it when you only need raw GPS arrays (elevation profile, distances, peaks…).
  trace = parseGpx(bytes);

  if (!trace) {
    document.body.innerHTML =
      '<p style="padding:2rem;font-family:monospace">Failed to parse GPX.</p>';
    throw new Error("GPX parse failed");
  }

  // Cache all bulk arrays once — each method copies WASM→JS heap (O(n)).
  const locsFlat = trace.getLocationsFlat();
  const dists = trace.getCumulativeDistances();
  const peaks = trace.getPeaks();
  const valleys = trace.getValleys();
  const climbs = trace.climbs();
  const area = trace.area();

  // analyzeGpx — full path: parses track-points + waypoints + metadata, then
  // runs the legs/sections/stages pipeline. Call this only when you need the
  // race analysis; it is independent of the Trace handle above.
  full = analyzeGpx(bytes, {
    basePaceSPerKm: 500,
    kFatigue: 0.002,
    lifeBaseStopS: 3600,
  });

  if (!full) {
    document.body.innerHTML =
      '<p style="padding:2rem;font-family:monospace">Failed to analyze GPX.</p>';
    throw new Error("GPX analysis failed");
  }

  // ── Header ────────────────────────────────────────────────────────────────────

  setText("race-title", full.metadata.name || "");

  // ── Stats cards ───────────────────────────────────────────────────────────────

  setText("stat-distance", trace.totalDistance.toFixed(1) + " km");
  setText("stat-gain", "+" + Math.round(trace.totalElevationGain) + " m");
  setText("stat-loss", "−" + Math.round(trace.totalElevationLoss) + " m");
  setText("stat-locations", trace.locationCount.toString());
  setText("stat-climbs", climbs.length.toString());

  if (area) {
    setText(
      "stat-bbox",
      `${area.minLatitude.toFixed(3)}°N – ${area.maxLatitude.toFixed(3)}°N, ` +
        `${area.minLongitude.toFixed(3)}°E – ${area.maxLongitude.toFixed(3)}°E`,
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
} finally {
  // ── Free WASM memory — all array data already copied to JS ───────────────────
  // `full` is a plain JS object (no WASM handle), nothing to free there.
  trace?.free();
}

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
            <td class="num" data-label="#">${String(i + 1).padStart(2, "0")}</td>
            <td data-label="Name">${w.name}</td>
            <td data-label="Type"><span class="type-badge ${typeClass(w.wptType)}">${typeLabel(w.wptType)}</span></td>
            <td class="num" data-label="Elevation">${w.elevation != null ? Math.round(w.elevation) + " m" : "—"}</td>
            <td class="num" data-label="Cutoff">${formatTimestamp(w.time)}</td>
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
            cumulativeKm += s.totalDistanceKm;
            const fromName = waypoints[s.id]?.name ?? "—";
            const toName = waypoints[s.id + 1]?.name ?? "—";
            return `
          <tr>
            <td class="num" data-label="#">${String(s.id + 1).padStart(2, "0")}</td>
            <td class="leg-label" data-label="From → To">${fromName} → ${toName}</td>
            <td class="num" data-label="Dist.">${s.totalDistanceKm.toFixed(1)} km</td>
            <td class="num muted" data-label="Cum.">${cumulativeKm.toFixed(1)} km</td>
            <td class="num gain" data-label="Gain">+${Math.round(s.totalElevationGainM)} m</td>
            <td class="num loss" data-label="Loss">−${Math.round(s.totalElevationLossM)} m</td>
            <td class="num" data-label="Est. time">${formatDuration(s.estimatedDurationS)}</td>
            <td class="num stars" data-label="Difficulty">${stars(s.difficulty)}</td>
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
        ["Start", "LifeBase", "Arrival"].includes(w.wptType),
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
            <td class="num" data-label="Stage">${String(s.id + 1).padStart(2, "0")}</td>
            <td class="leg-label" data-label="From → To">${fromName} → ${toName}</td>
            <td class="num" data-label="Distance">${s.totalDistanceKm.toFixed(1)} km</td>
            <td class="num gain" data-label="Gain">+${Math.round(s.totalElevationGainM)} m</td>
            <td class="num loss" data-label="Loss">−${Math.round(s.totalElevationLossM)} m</td>
            <td class="num" data-label="Est. time">${formatDuration(s.estimatedDurationS)}</td>
            <td class="num stars" data-label="Difficulty">${stars(s.difficulty)}</td>
          </tr>`;
          })
          .join("")}
      </tbody>
    </table>`;
}

function drawProfile(canvas, locsFlat, dists, peaks, valleys, climbs) {
  const DPR = window.devicePixelRatio || 1;
  const W = canvas.parentElement.clientWidth;
  const H = W < 600 ? 180 : 260;
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
  let minAlt = Infinity;
  let maxAlt = -Infinity;
  for (const alt of alts) {
    if (alt < minAlt) minAlt = alt;
    if (alt > maxAlt) maxAlt = alt;
  }
  minAlt -= 80;
  maxAlt += 80;

  const xP = (d) => PAD.left + (d / maxDist) * CW;
  const yP = (a) => PAD.top + (1 - (a - minAlt) / (maxAlt - minAlt)) * CH;

  // ── White background
  ctx.fillStyle = "#fff";
  ctx.fillRect(0, 0, W, H);

  // ── Yellow climb zones (behind everything)
  climbs.forEach((c) => {
    ctx.fillStyle = "#ffe000";
    const x1 = xP(c.startDistKm);
    const x2 = xP(c.startDistKm + c.climbDistKm);
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
    const midX = xP(c.startDistKm + c.climbDistKm / 2);
    ctx.fillStyle = "#000";
    ctx.fillText(`↑ ${c.avgGradient.toFixed(1)}%`, midX, H - PAD.bottom - 4);
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
          <span class="val">${c.climbDistKm.toFixed(1)}<small>km</small></span>
        </div>
        <div class="metric">
          <span class="label">gain</span>
          <span class="val">+${Math.round(c.elevationGain)}<small>m</small></span>
        </div>
        <div class="metric">
          <span class="label">grade</span>
          <span class="val">${c.avgGradient.toFixed(1)}<small>%</small></span>
        </div>
        <div class="metric">
          <span class="label">summit</span>
          <span class="val">${Math.round(c.summitElev)}<small>m</small></span>
        </div>
        <div class="metric score">
          <span class="label">score</span>
          <span class="val">${(c.climbDistKm * c.avgGradient).toFixed(0)}</span>
        </div>
      </div>
    </div>
  `,
    )
    .join("");
}
