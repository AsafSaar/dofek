/* Dofek GUI — main application script.
   Extracted verbatim from the first inline <script> in index.html so the
   CSP can enforce script-src 'self'. Loaded at the same point in the
   document, so execution order relative to the markup is unchanged. */
/* ═══ HTML ESCAPING ══════════════════════════════════════
   Almost everything this UI renders is OS-controlled: process names come
   from whatever is running on the machine, and a process can name itself
   anything. Any such value that reaches an `innerHTML` template must go
   through `esc()` first — prefer `textContent`, and use `esc()` only where
   the surrounding markup genuinely has to be built as a string.

   The five characters below cover both element context (`<`, `>`, `&`) and
   attribute context (`"`, `'`), so the same helper is safe for
   `title="${esc(name)}"` as for `<span>${esc(name)}</span>`.

   `tests/frontend_no_raw_innerhtml.rs` enforces that every `innerHTML`
   assignment in this directory is either escaped or annotated `// SAFE:`. */
const _ESC = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' };
function esc(v) {
  return String(v == null ? '' : v).replace(/[&<>"']/g, c => _ESC[c]);
}

/* ═══ TAURI IPC ══════════════════════════════════════════ */
var invoke;
var tauriListen;

/* Capture `invoke` and `listen` into module-scope vars, then drop the
   `window.__TAURI__` convenience global.

   The global isn't always populated by the time this script parses, which is
   why acquisition is a function called again from `tick()` and
   `startSnapshotListener()` rather than a one-shot at boot. The global is
   deleted only once *both* handles are held, so a retry can never find it
   already gone.

   `withGlobalTauri` stays on in tauri.conf.json — this frontend has no
   bundler, so the global is how it reaches the API in the first place.
   Deleting it afterwards is defense in depth behind the real fix
   (script-src 'self'), not a barrier on its own: `__TAURI_INTERNALS__` is
   installed by the runtime and is what the dialog and shell plugins call
   through, so it has to stay reachable. */
function acquireTauriApi() {
  try {
    const g = window.__TAURI__;
    // Tauri 2: try the @tauri-apps/api path first, then raw internals.
    if (!invoke) {
      if (g && g.core) invoke = g.core.invoke;
      else if (window.__TAURI_INTERNALS__) invoke = window.__TAURI_INTERNALS__.invoke;
    }
    if (!tauriListen && g && g.event) tauriListen = g.event.listen;
    if (invoke && tauriListen) {
      try { delete window.__TAURI__; } catch(e) { /* non-configurable — harmless */ }
    }
  } catch(e) {
    console.error('Tauri IPC init failed:', e);
  }
  return !!invoke;
}
acquireTauriApi();

// Stamp the version into the topbar logo and About overlay, so they track
// Cargo.toml instead of being frozen at the literal we shipped with.
if (invoke) {
  invoke('get_app_version').then(v => {
    const tag = 'v' + v;
    const top = document.getElementById('app-version');
    const about = document.getElementById('about-version');
    if (top) top.textContent = tag;
    if (about) about.textContent = tag;
  }).catch(() => {});
}

/* Open the help/settings overlay and hydrate the toggles from current settings.
   Used by the `?` key and by the "Settings" tray menu item (which emits
   `dofek://open-settings` from the Rust side). */
function openSettingsOverlay() {
  const helpEl = document.getElementById('help-overlay');
  if (!helpEl.classList.contains('visible')) helpEl.classList.add('visible');
  if (invoke) {
    invoke('get_settings').then(s => {
      document.getElementById('telem-toggle-cb').checked = s.telemetry_enabled || false;
      document.getElementById('tray-enable-cb').checked = s.enable_tray !== false;
      document.getElementById('tray-close-cb').checked = s.close_to_tray !== false;
      // Resolve effective tray display mode: prefer the explicit 3-way
      // setting; fall back to the legacy boolean for users upgrading from <1.4.
      const mode = (s.tray_display_mode && ['chart','chart+text','text'].includes(s.tray_display_mode))
        ? s.tray_display_mode
        : (s.tray_show_text === false ? 'chart' : 'chart+text');
      document.getElementById('tray-mode-select').value = mode;
      document.getElementById('update-startup-cb').checked = !!s.check_updates_on_startup;
    }).catch(() => {});
    // Also hydrate the plugin list (defined later in this file).
    if (typeof refreshPluginsList === 'function') refreshPluginsList();
  }
}

if (tauriListen) {
  tauriListen('dofek://open-settings', () => openSettingsOverlay()).catch(err => {
    console.warn('Failed to register tray-settings listener:', err);
  });
}
const LEN = 60;
let dashOffset = 0; // animated threshold dash offset

/* GPU definitions — populated from first snapshot */
let GPUS = [];

/* Platform-specific empty-state labels for the GPU tile.
   On Apple Silicon this becomes the chip name + unified-memory hint. */
let PLATFORM_INFO = null;

/* ═══ STATE ══════════════════════════════════════════════ */
const S = {
  metric: 'cpu',
  chartMode: 'default', // 'default' or 'horizon'
  gpu: 'all',
  cat: 'all',
  sortBy: 'cpu', sortAsc: false,
  cpu: { hist:[], raw:[], cores:[], temp:null, power:null },
  gpus: [],
  mem: { hist:[], usedGB:0, totalGB:1, swapPct:0 },
  net: { dh:[], uh:[], down:0, up:0 },
  disk: { rh:[], wh:[], read:0, write:0 },
};

function push(arr,v) { arr.push(v); if(arr.length>LEN) arr.shift(); }

function makeCandle(mean, variance) {
  const v = Math.abs(variance);
  const min  = Math.max(0,  mean - v*0.7);
  const max  = Math.min(100, mean + v*0.6);
  const p25  = min  + (mean-min)*0.4;
  const p75  = mean + (max-mean)*0.4;
  return {mean, min, max, p25, p75};
}

/* ═══ LIVE PROCESS DATA ═════════════════════════════════ */
let PROCS = [];
let PLUGINS = [];              // snap.plugin_statuses — see upPlugins()
let searchQuery = '';
let selectedPid = null;        // PID of the selected process row
let searchVisible = false;
let pendingKill = null;        // {type:'single'|'batch', pids:[], names:[]}
let groupedView = false;       // tree view toggle
let expandedGroups = new Set(); // group names that are expanded

/* Map Rust category enum to frontend category string */
function mapCategory(proc) {
  if (proc.category === 'Watch') return 'watch';
  if (proc.category === 'Ai' || proc.is_ai_workload) return 'ai';
  if (proc.category === 'Dev') return 'dev';
  return null;
}

function mapAiState(proc) {
  if (proc.ai_state === 'Inferring') return 'inf';
  if (proc.ai_state === 'Idle') return 'ild';
  if (proc.ai_state === 'Loading') return 'inf';
  return null;
}

/* ═══ DATA FETCH ════════════════════════════════════════ */
/* tick() either uses the snapshot pushed in (fast path, via the
   dofek://snapshot Tauri event) or pulls one via invoke('get_snapshot') for
   the initial hydration before the event listener is wired. The push path
   removes the per-tick IPC round-trip and full-snapshot JSON parse — the
   dominant remaining WebKitGTK cost at 1 Hz. */
async function tick(snap) {
  // Retry Tauri IPC init — the global may not have existed at script parse.
  if (!acquireTauriApi()) return;
  if (!snap) {
    try {
      snap = await invoke('get_snapshot');
    } catch(e) {
      console.error('Failed to get snapshot:', e);
      return;
    }
  }

  if (!PLATFORM_INFO) {
    invoke('get_platform_info').then(p => { PLATFORM_INFO = p; }).catch(() => {});
  }

  /* Dismiss loading overlay on first real data */
  const lo = document.getElementById('loading-overlay');
  if (lo && !lo.classList.contains('hidden')) lo.classList.add('hidden');

  /* Set hostname from snapshot */
  if (snap.hostname) document.getElementById('hostname').textContent = snap.hostname;

  /* CPU — skip the first sysinfo refresh (always returns 100%) */
  const avg = snap.cpu.total_load || 0;
  if (avg >= 99 && S.cpu.hist.length === 0) return; // discard bogus first sample
  S.cpu.cores = snap.cpu.per_core_load || [];
  S.cpu.name = snap.cpu.name || 'CPU';
  S.cpu.temp = snap.cpu.temperature || null;
  S.cpu.power = snap.cpu.power || null;
  push(S.cpu.hist, makeCandle(avg, avg*0.3 + 2));
  push(S.cpu.raw, avg);

  /* GPUs */
  while (S.gpus.length < snap.gpus.length) {
    S.gpus.push({ hist:[], util:0, vram:0, temp:0, power:0 });
  }
  snap.gpus.forEach((g, i) => {
    S.gpus[i].util  = g.utilization;
    S.gpus[i].vram  = g.vram_used_mb;
    S.gpus[i].temp  = g.temperature;
    S.gpus[i].power = g.power_watts;
    push(S.gpus[i].hist, g.utilization);
  });

  /* Build GPUS definitions if not yet set */
  if (GPUS.length === 0 && snap.gpus.length > 0) {
    GPUS = snap.gpus.map(g => ({
      name: g.name.replace('NVIDIA ','').replace('GeForce ',''),
      vramMax: g.vram_total_mb,
      powerMax: Math.max(g.power_watts * 3, 250),
    }));
    /* Update GPU tab labels */
    const gpuTabs = document.querySelectorAll('.gpu-tab[data-g]');
    gpuTabs.forEach(tab => {
      const idx = parseInt(tab.dataset.g);
      if (!isNaN(idx) && GPUS[idx]) tab.textContent = GPUS[idx].name;
    });
    /* Show/hide GPU1 tab */
    if (GPUS.length < 2) {
      const tab1 = document.querySelector('.gpu-tab[data-g="1"]');
      if (tab1) tab1.style.display = 'none';
    }
  }

  /* Memory */
  S.mem.usedGB  = snap.memory.used_gb;
  S.mem.totalGB = snap.memory.total_gb;
  S.mem.swapPct = snap.memory.swap_used_percent;
  push(S.mem.hist, snap.memory.used_percent);

  /* Network — aggregate all interfaces for total throughput,
     display name of the busiest one */
  let totalDown=0, totalUp=0, bestName='', bestTraffic=0;
  (snap.network.interfaces||[]).forEach(iface=>{
    totalDown += iface.rx_bytes_per_sec;
    totalUp   += iface.tx_bytes_per_sec;
    const traffic = iface.rx_bytes_per_sec + iface.tx_bytes_per_sec;
    if(traffic > bestTraffic){ bestTraffic=traffic; bestName=iface.name; }
  });
  S.net.down = totalDown;
  S.net.up   = totalUp;
  if(bestName) S.net.ifName = bestName;
  else if(snap.network.interfaces.length) S.net.ifName = snap.network.interfaces[0].name;
  push(S.net.dh, S.net.down);
  push(S.net.uh, S.net.up);

  /* Disk */
  const disk = snap.disk || {total_read_bytes_per_sec:0, total_write_bytes_per_sec:0};
  S.disk.read  = disk.total_read_bytes_per_sec || 0;
  S.disk.write = disk.total_write_bytes_per_sec || 0;
  push(S.disk.rh, S.disk.read);
  push(S.disk.wh, S.disk.write);

  /* Processes */
  PROCS = snap.processes.map(p => ({
    name: p.name,
    pid:  p.pid,
    cpu:  p.cpu_percent,
    mem:  p.memory_bytes / (1024*1024), // bytes -> MB
    vram: p.vram_bytes ? p.vram_bytes / (1024*1024) : null, // bytes -> MB
    ai:   mapAiState(p),
    cat:  mapCategory(p),
    label: p.plugin_label || null,
  }));

  /* Plugins. Serialized since v1.7 — see DataSnapshot.plugin_statuses. */
  PLUGINS = Array.isArray(snap.plugin_statuses) ? snap.plugin_statuses : [];
}

/* ═══ CANVAS HELPERS ═════════════════════════════════════ */
/* Setting `canvas.width` reallocates the backing pixel buffer and resets all
   ctx state — doing it on every frame for 4-7 canvases burns real WebKit CPU.
   We cache the last applied (cssW, cssH, dpr) per canvas in a WeakMap and
   skip the reset when nothing changed; otherwise we still do the resize +
   ctx.scale(dpr) once. clearRect handles the per-frame wipe. */
const _canvasSize = new WeakMap();
function prep(c) {
  const dpr = devicePixelRatio || 1;
  const r = c.getBoundingClientRect();
  if (r.width < 1 || r.height < 1) return null;
  const last = _canvasSize.get(c);
  let ctx;
  if (!last || last.cssW !== r.width || last.cssH !== r.height || last.dpr !== dpr) {
    c.width = r.width * dpr;
    c.height = r.height * dpr;
    ctx = c.getContext('2d');
    ctx.scale(dpr, dpr);
    _canvasSize.set(c, { cssW: r.width, cssH: r.height, dpr });
  } else {
    ctx = c.getContext('2d');
  }
  return { ctx, W: r.width, H: r.height };
}

function bezier(ctx,pts,t=0.27) {
  for(let i=0;i<pts.length-1;i++){
    const p0=pts[Math.max(0,i-1)],p1=pts[i],p2=pts[i+1],p3=pts[Math.min(pts.length-1,i+2)];
    ctx.bezierCurveTo(p1.x+(p2.x-p0.x)*t,p1.y+(p2.y-p0.y)*t,p2.x-(p3.x-p1.x)*t,p2.y-(p3.y-p1.y)*t,p2.x,p2.y);
  }
}

/* CANDLESTICK chart for CPU */
function drawCandle(c, candleData, color, lo=0, hi=100, warn=null, crit=null) {
  const s=prep(c); if(!s||candleData.length<2) return;
  const {ctx,W,H}=s;
  const pl=6,pr=10,pt=14,pb=6;
  const dW=W-pl-pr, dH=H-pt-pb;
  ctx.clearRect(0,0,W,H);
  const toY = v => pt+dH*(1-Math.max(0,Math.min(1,(v-lo)/(hi-lo))));

  /* grid */
  ctx.strokeStyle='rgba(255,255,255,.03)'; ctx.lineWidth=1;
  for(let i=1;i<=3;i++){
    const y=pt+dH/4*i;
    ctx.beginPath(); ctx.moveTo(pl,y); ctx.lineTo(W-pr,y); ctx.stroke();
  }

  /* thresholds (animated dash march) */
  const thresh=(val,col,dash)=>{
    if(val===null) return;
    const y=toY(val);
    ctx.strokeStyle=col; ctx.lineWidth=1; ctx.setLineDash(dash); ctx.lineDashOffset=dashOffset;
    ctx.beginPath(); ctx.moveTo(pl,y); ctx.lineTo(W-pr,y); ctx.stroke();
    ctx.setLineDash([]); ctx.lineDashOffset=0;
  };
  thresh(warn,'rgba(251,191,36,.4)',[5,4]);
  thresh(crit,'rgba(248,113,113,.4)',[3,3]);

  const gap = dW/(LEN);
  const bodyW = Math.max(2, gap*0.55);

  candleData.forEach((d,i) => {
    const x = pl + i*gap + gap/2;
    const yMean = toY(d.mean);
    const yMin  = toY(d.min);
    const yMax  = toY(d.max);
    const yP25  = toY(d.p25);
    const yP75  = toY(d.p75);

    /* full range wick */
    ctx.strokeStyle = color+'45';
    ctx.lineWidth = 1;
    ctx.beginPath(); ctx.moveTo(x,yMax); ctx.lineTo(x,yMin); ctx.stroke();

    /* IQR body */
    const bodyTop    = Math.min(yP25,yP75);
    const bodyHeight = Math.max(2,Math.abs(yP25-yP75));
    ctx.fillStyle = i===candleData.length-1 ? color+'cc' : color+'60';
    ctx.fillRect(x-bodyW/2, bodyTop, bodyW, bodyHeight);

    /* mean tick (the "close" equivalent) */
    ctx.strokeStyle = i===candleData.length-1 ? color : color+'99';
    ctx.lineWidth = i===candleData.length-1 ? 2 : 1.5;
    ctx.beginPath();
    ctx.moveTo(x-bodyW/2, yMean);
    ctx.lineTo(x+bodyW/2, yMean);
    ctx.stroke();
  });

  /* current mean horizontal guide */
  const last = candleData[candleData.length-1];
  const yLast = toY(last.mean);
  ctx.strokeStyle = color+'50';
  ctx.lineWidth = 1;
  ctx.setLineDash([4,5]);
  ctx.beginPath(); ctx.moveTo(pl,yLast); ctx.lineTo(W-pr,yLast); ctx.stroke();
  ctx.setLineDash([]);

  /* live dot */
  ctx.beginPath(); ctx.arc(pl+dW, yLast, 3, 0, Math.PI*2);
  ctx.fillStyle = color; ctx.fill();
  ctx.beginPath(); ctx.arc(pl+dW, yLast, 5.5, 0, Math.PI*2);
  ctx.strokeStyle = color+'55'; ctx.lineWidth=1.5; ctx.stroke();
}

/* AREA chart for GPU/MEM/NET.
   `straight=true` swaps cubic-bezier smoothing for a polyline — used by the
   four small sp-* sparklines, where bezier through ~LEN points is the
   single biggest per-frame cost on WebKitGTK. The main chart still smooths. */
function drawArea(c, hist, color, lo=0, hi=100, warn=null, crit=null, secondary=null, straight=false) {
  const s=prep(c); if(!s||hist.length<2) return;
  const {ctx,W,H}=s;
  const pl=4,pr=8,pt=12,pb=6;
  const dW=W-pl-pr, dH=H-pt-pb;
  ctx.clearRect(0,0,W,H);
  const toY=v=>pt+dH*(1-Math.max(0,Math.min(1,(v-lo)/(hi-lo))));

  const trace=(pts)=>{
    if (straight) { for (let i=1;i<pts.length;i++) ctx.lineTo(pts[i].x, pts[i].y); }
    else { bezier(ctx, pts); }
  };

  ctx.strokeStyle='rgba(255,255,255,.03)'; ctx.lineWidth=1;
  for(let i=1;i<=3;i++){const y=pt+dH/4*i;ctx.beginPath();ctx.moveTo(pl,y);ctx.lineTo(W-pr,y);ctx.stroke();}

  const thresh=(val,col,dash)=>{
    if(val===null) return;
    const y=toY(val);
    ctx.strokeStyle=col;ctx.lineWidth=1;ctx.setLineDash(dash);ctx.lineDashOffset=dashOffset;
    ctx.beginPath();ctx.moveTo(pl,y);ctx.lineTo(W-pr,y);ctx.stroke();ctx.setLineDash([]);ctx.lineDashOffset=0;
  };
  thresh(warn,'rgba(251,191,36,.4)',[5,4]);
  thresh(crit,'rgba(248,113,113,.4)',[3,3]);

  if(secondary){
    const pts2=secondary.map((v,i)=>({x:pl+(i/(LEN-1))*dW,y:toY(v)}));
    ctx.globalAlpha=.55; ctx.beginPath(); ctx.moveTo(pts2[0].x,pts2[0].y); trace(pts2);
    ctx.strokeStyle=secondary.col||'#94a3b8'; ctx.lineWidth=1.2; ctx.stroke(); ctx.globalAlpha=1;
  }

  const pts=hist.map((v,i)=>({x:pl+(i/(LEN-1))*dW,y:toY(v)}));
  const g=ctx.createLinearGradient(0,pt,0,H);
  g.addColorStop(0,color+'40'); g.addColorStop(1,color+'00');
  ctx.beginPath();ctx.moveTo(pts[0].x,H-pb);ctx.lineTo(pts[0].x,pts[0].y);trace(pts);
  ctx.lineTo(pts[pts.length-1].x,H-pb);ctx.closePath();ctx.fillStyle=g;ctx.fill();
  ctx.beginPath();ctx.moveTo(pts[0].x,pts[0].y);trace(pts);
  ctx.strokeStyle=color;ctx.lineWidth=1.8;ctx.stroke();
  const lp=pts[pts.length-1];
  ctx.beginPath();ctx.arc(lp.x,lp.y,3,0,Math.PI*2);ctx.fillStyle=color;ctx.fill();
  ctx.beginPath();ctx.arc(lp.x,lp.y,5.5,0,Math.PI*2);ctx.strokeStyle=color+'55';ctx.lineWidth=1.5;ctx.stroke();
}

function drawDual(c,h1,h2,c1,c2) {
  const s=prep(c); if(!s) return;
  const {ctx,W,H}=s;
  const maxN=Math.max(1e4,...h1,...h2);
  ctx.clearRect(0,0,W,H);
  // Polyline (no bezier) — drawDual is sparkline-only (sp-net).
  [[h1,c1],[h2,c2]].forEach(([h,col])=>{
    if(h.length<2) return;
    const pts=h.map((v,i)=>({x:(i/(LEN-1))*W,y:H-2-(Math.max(0,Math.min(1,v/maxN))*(H-4))}));
    const g=ctx.createLinearGradient(0,0,0,H);
    g.addColorStop(0,col+'28');g.addColorStop(1,col+'00');
    ctx.beginPath();ctx.moveTo(pts[0].x,H);ctx.lineTo(pts[0].x,pts[0].y);
    for (let i=1;i<pts.length;i++) ctx.lineTo(pts[i].x, pts[i].y);
    ctx.lineTo(pts[pts.length-1].x,H);ctx.closePath();ctx.fillStyle=g;ctx.fill();
    ctx.beginPath();ctx.moveTo(pts[0].x,pts[0].y);
    for (let i=1;i<pts.length;i++) ctx.lineTo(pts[i].x, pts[i].y);
    ctx.strokeStyle=col;ctx.lineWidth=1.2;ctx.stroke();
  });
}

/* HORIZON chart — folds y-axis into layered color bands */
function drawHorizon(c, hist, color, lo=0, hi=100, warn=null, crit=null, numBands=3) {
  const s=prep(c); if(!s||hist.length<2) return;
  const {ctx,W,H}=s;
  const pl=6,pr=10,pt=14,pb=6;
  const dW=W-pl-pr, dH=H-pt-pb;
  ctx.clearRect(0,0,W,H);

  /* grid */
  ctx.strokeStyle='rgba(255,255,255,.03)'; ctx.lineWidth=1;
  for(let i=1;i<=3;i++){
    const y=pt+dH/4*i;
    ctx.beginPath(); ctx.moveTo(pl,y); ctx.lineTo(W-pr,y); ctx.stroke();
  }

  /* thresholds (animated dash march) */
  const thresh=(val,col,dash)=>{
    if(val===null) return;
    const frac=(val-lo)/(hi-lo);
    const y=pt+dH*(1-frac);
    ctx.strokeStyle=col;ctx.lineWidth=1;ctx.setLineDash(dash);ctx.lineDashOffset=dashOffset;
    ctx.beginPath();ctx.moveTo(pl,y);ctx.lineTo(W-pr,y);ctx.stroke();ctx.setLineDash([]);ctx.lineDashOffset=0;
  };
  thresh(warn,'rgba(251,191,36,.4)',[5,4]);
  thresh(crit,'rgba(248,113,113,.4)',[3,3]);

  /* parse base color into RGB */
  const parseHex=c=>{const m=c.match(/^#?([\da-f]{2})([\da-f]{2})([\da-f]{2})/i);return m?[parseInt(m[1],16),parseInt(m[2],16),parseInt(m[3],16)]:[56,189,248];};
  const [br,bg,bb]=parseHex(color);
  const bandColor=(band)=>{
    const factors=[0.25,0.55,1.0];
    const f=factors[Math.min(band,factors.length-1)];
    return `rgb(${Math.round(br*f)},${Math.round(bg*f)},${Math.round(bb*f)})`;
  };
  const bandAlpha=(band)=>[0.5,0.7,1.0][Math.min(band,2)];

  const range=hi-lo;
  const bandRange=range/numBands;
  const gap=dW/LEN;
  const barW=Math.max(1,gap-1);

  /* draw each column as stacked horizon bands */
  hist.forEach((rawVal,i)=>{
    const v=Math.max(lo,Math.min(hi,rawVal))-lo; // value relative to lo
    const x=pl+i*gap;
    const bandIdx=Math.min(Math.floor(v/bandRange),numBands-1);

    for(let b=0;b<=bandIdx;b++){
      const fillFrac=b<bandIdx?1.0:((v-b*bandRange)/bandRange);
      const fillH=fillFrac*dH;
      const yTop=pt+dH-fillH;

      ctx.globalAlpha=bandAlpha(b);
      ctx.fillStyle=bandColor(b);
      ctx.fillRect(x,yTop,barW,fillH);
    }
  });
  ctx.globalAlpha=1;

  /* bright edge line on top of highest band */
  ctx.beginPath();
  const pts=hist.map((rawVal,i)=>{
    const v=Math.max(lo,Math.min(hi,rawVal));
    const frac=(v-lo)/range;
    return {x:pl+i*gap+barW/2, y:pt+dH*(1-frac)};
  });
  ctx.moveTo(pts[0].x,pts[0].y);
  bezier(ctx,pts);
  ctx.strokeStyle=color;
  ctx.lineWidth=1.5;
  ctx.stroke();

  /* live dot */
  const lp=pts[pts.length-1];
  ctx.beginPath();ctx.arc(lp.x,lp.y,3,0,Math.PI*2);ctx.fillStyle=color;ctx.fill();
  ctx.beginPath();ctx.arc(lp.x,lp.y,5.5,0,Math.PI*2);ctx.strokeStyle=color+'55';ctx.lineWidth=1.5;ctx.stroke();
}

/* ═══ FORMATTERS ══════════════════════════════════════════ */
function fmtB(b){if(b<1e3)return b.toFixed(0)+' B/s';if(b<1e6)return (b/1e3).toFixed(1)+' KB/s';return (b/1e6).toFixed(1)+' MB/s';}
function fmtMB(m){return m>=1024?(m/1024).toFixed(1)+' GB':m.toFixed(0)+' MB';}
function setText(id, val) {
  const el = document.getElementById(id);
  if (!el || el.textContent === val) return;
  el.textContent = val;
  // Restart the .flash animation without forcing a synchronous layout. The
  // previous `void el.offsetWidth` trick caused 7+ layout reflows per frame
  // (every pill in the topbar uses setText). rAF lets the browser batch.
  el.classList.remove('flash');
  if (!el._flashScheduled) {
    el._flashScheduled = true;
    requestAnimationFrame(() => { el.classList.add('flash'); el._flashScheduled = false; });
  }
}

/* ═══ RENDER ═════════════════════════════════════════════ */
function upTopBar() {
  const avg=S.cpu.hist[S.cpu.hist.length-1]?.mean||0;
  const n=S.gpus.length;
  const combinedUtil=n?S.gpus.reduce((s,g)=>s+g.util,0)/n:0;
  const combinedVram=S.gpus.reduce((s,g)=>s+g.vram,0);
  const totalVram=GPUS.reduce((s,g)=>s+g.vramMax,0)||1;
  const maxTemp=n?Math.max(...S.gpus.map(g=>g.temp)):0;
  setText('t-cpu', avg.toFixed(1)+'%');
  setText('t-gpu', combinedUtil.toFixed(1)+'%');
  setText('t-vram', Math.round(combinedVram)+'/'+Math.round(totalVram)+'MB');
  setText('t-mem', ((S.mem.usedGB/S.mem.totalGB)*100).toFixed(1)+'%');
  setText('t-tmp', maxTemp.toFixed(1)+'°C');
  /* CPU temp pill (from LHM) */
  const cpuTempPill = document.getElementById('cpu-temp-pill');
  if (S.cpu.temp && S.cpu.temp > 0) {
    cpuTempPill.style.display = '';
    setText('t-cpu-tmp', S.cpu.temp.toFixed(0)+'°C');
  } else {
    cpuTempPill.style.display = 'none';
  }
  setText('t-dn','↓ '+fmtB(S.net.down));
  setText('t-up','↑ '+fmtB(S.net.up));

  /* AI badge — show the first inferring/loading AI process */
  const aiProc = PROCS.find(p => p.ai === 'inf' || p.ai === 'ild');
  const aiPill = document.getElementById('ai-pill');
  if (aiProc && aiPill) {
    aiPill.style.display = '';
    const label = aiProc.ai === 'inf' ? 'inferring' : 'idle';
    setText('t-ai', aiProc.name + ' — ' + label);
  } else if (aiPill) {
    aiPill.style.display = 'none';
  }
}

function upMainChart() {
  const mc=document.getElementById('mainCanvas');
  const m=S.metric;
  let valStr,hw;
  const badge=document.getElementById('chart-mode');
  const legend=document.getElementById('thresh-legend');

  const horizon = S.chartMode === 'horizon';

  if(m==='cpu'){
    const cd=S.cpu.hist;
    if(!cd.length){ valStr='—'; hw='Waiting...'; }
    else {
      const last=cd[cd.length-1]; const prev=cd[cd.length-2];
      valStr=last.mean.toFixed(1)+'%';
      upDelta(last.mean,prev?.mean||last.mean,'%');
      hw=S.cpu.name||'CPU';
    }
    if(horizon){
      badge.textContent='HORIZON'; badge.className='chart-mode-badge horizon';
      if(S.cpu.raw.length>=2) drawHorizon(mc, S.cpu.raw, '#38bdf8', 0, 100, 80, 90);
    } else {
      badge.textContent='CANDLE'; badge.className='chart-mode-badge candle';
      if(cd.length>=2) drawCandle(mc, cd, '#38bdf8', 0, 100, 80, 90);
    }
    legend.style.display='flex';
  } else if(m==='gpu'){
    const n=S.gpus.length;
    if(!n){ valStr='N/A'; hw=(PLATFORM_INFO&&PLATFORM_INFO.title)||'No GPU'; }
    else if(n>=2){
      const h0=S.gpus[0].hist, h1=S.gpus[1].hist;
      const combined=h0.map((v,i)=>(v+(h1[i]||0))/2);
      const cv=combined[combined.length-1]||0;
      valStr=cv.toFixed(1)+'%';
      upDelta(cv,combined[combined.length-2]||cv,'%');
      hw=GPUS.map(g=>g.name).join(' + ')+' (avg util)';
      if(horizon){
        badge.textContent='HORIZON'; badge.className='chart-mode-badge horizon';
        const gpuHi=Math.max(10,...h0,...h1);
        drawHorizon(mc,h0,'#a78bfa',0,gpuHi,gpuHi*0.85,gpuHi*0.95);
      } else {
        badge.textContent='AREA'; badge.className='chart-mode-badge';
        const sec=h1.slice(); sec.col='#db2777';
        const gpuHi=Math.max(10,...h0,...h1);
        drawArea(mc,h0,'#a78bfa',0,gpuHi,gpuHi*0.85,gpuHi*0.95,sec);
      }
    } else {
      const g0=S.gpus[0];
      valStr=g0.util.toFixed(1)+'%';
      const prev=g0.hist[g0.hist.length-2]||g0.util;
      upDelta(g0.util,prev,'%');
      hw=GPUS[0]?.name||'GPU';
      const gpuHi=Math.max(10,...g0.hist);
      if(horizon){
        badge.textContent='HORIZON'; badge.className='chart-mode-badge horizon';
        drawHorizon(mc,g0.hist,'#a78bfa',0,gpuHi,gpuHi*0.85,gpuHi*0.95);
      } else {
        badge.textContent='AREA'; badge.className='chart-mode-badge';
        drawArea(mc,g0.hist,'#a78bfa',0,gpuHi,gpuHi*0.85,gpuHi*0.95);
      }
    }
    legend.style.display='flex';
  } else if(m==='mem'){
    const v=(S.mem.usedGB/(S.mem.totalGB||1))*100;
    const prev=S.mem.hist[S.mem.hist.length-2]||v;
    valStr=v.toFixed(1)+'%';
    upDelta(v,prev,'%');
    hw=S.mem.usedGB.toFixed(1)+' / '+S.mem.totalGB+' GB';
    if(horizon){
      badge.textContent='HORIZON'; badge.className='chart-mode-badge horizon';
      drawHorizon(mc,S.mem.hist,'#34d399',0,100,80,90);
    } else {
      badge.textContent='AREA'; badge.className='chart-mode-badge';
      drawArea(mc,S.mem.hist,'#34d399',0,100,80,90);
    }
    legend.style.display='flex';
  } else if(m==='net'){
    const maxN=Math.max(1e4,...S.net.dh,...S.net.uh)*1.1;
    valStr=fmtB(S.net.down);
    document.getElementById('c-delta').textContent='↑ '+fmtB(S.net.up);
    document.getElementById('c-delta').className='c-delta flat';
    hw=S.net.ifName||'Network';
    if(horizon){
      badge.textContent='HORIZON'; badge.className='chart-mode-badge horizon';
      drawHorizon(mc,S.net.dh,'#38bdf8',0,maxN,null,null);
    } else {
      badge.textContent='AREA'; badge.className='chart-mode-badge';
      const sec=S.net.uh.slice(); sec.col='#fb923c';
      drawArea(mc,S.net.dh,'#38bdf8',0,maxN,null,null,sec);
    }
    legend.style.display='none';
  } else { // disk
    const maxD=Math.max(1e4,...S.disk.rh,...S.disk.wh)*1.1;
    valStr='↓'+fmtB(S.disk.read);
    document.getElementById('c-delta').textContent='↑ '+fmtB(S.disk.write);
    document.getElementById('c-delta').className='c-delta flat';
    hw='Disk I/O';
    if(horizon){
      badge.textContent='HORIZON'; badge.className='chart-mode-badge horizon';
      drawHorizon(mc,S.disk.rh,'#eab308',0,maxD,null,null);
    } else {
      badge.textContent='AREA'; badge.className='chart-mode-badge';
      const sec=S.disk.wh.slice(); sec.col='#fb923c';
      drawArea(mc,S.disk.rh,'#eab308',0,maxD,null,null,sec);
    }
    legend.style.display='none';
  }
  document.getElementById('c-val').textContent=valStr||'—';
  document.getElementById('c-val').className='c-val '+m;
  document.getElementById('c-hw').textContent=hw||'';
}

function upDelta(cur,prev,unit){
  const d=cur-prev,el=document.getElementById('c-delta');
  el.textContent=(d>=0?'+':'')+d.toFixed(1)+unit;
  el.className='c-delta '+(Math.abs(d)<0.05?'flat':d>0?'up':'dn');
}

function upCores(){
  if(S.cpu.name) document.getElementById('cpu-sub').textContent=S.cpu.name;
  const div = document.getElementById('cores');
  /* Build the per-core rows once (keyed by index) and only mutate width/color/
     pct text on subsequent frames. The previous innerHTML rebuild reparsed
     ~16-32 nested divs every 500ms — small N but adds up to real WebKit work. */
  const n = S.cpu.cores.length;
  while (div.children.length < n) {
    const i = div.children.length;
    const row = document.createElement('div');
    row.className = 'core';
    // SAFE: `i` is a loop index; the rest is a literal.
    row.innerHTML = `<div class="core-lbl">C${i}</div><div class="core-track"><div class="core-fill"></div></div><div class="core-pct">0%</div>`;
    div.appendChild(row);
  }
  while (div.children.length > n) div.lastChild.remove();
  for (let i = 0; i < n; i++) {
    const v = S.cpu.cores[i] || 0;
    const row = div.children[i];
    const fill = row.children[1].firstChild;
    const pct = row.children[2];
    const wPct = Math.min(v, 100) + '%';
    if (fill.style.width !== wPct) fill.style.width = wPct;
    const bg = v > 90 ? 'var(--crit)' : v > 75 ? 'var(--warn)' : 'var(--cpu)';
    if (fill.dataset.bg !== bg) { fill.style.background = bg; fill.dataset.bg = bg; }
    const pctTxt = v.toFixed(1) + '%';
    if (pct.textContent !== pctTxt) pct.textContent = pctTxt;
  }
  /* CPU temp/power from LHM */
  const sensorsEl = document.getElementById('cpu-sensors');
  if (S.cpu.temp || S.cpu.power) {
    sensorsEl.style.display = '';
    let sh = '';
    if (S.cpu.temp) {
      const tc = S.cpu.temp > 90 ? 'var(--crit)' : S.cpu.temp > 75 ? 'var(--warn)' : 'var(--warn)';
      const tp = Math.min(S.cpu.temp / 105 * 100, 100);
      sh += `<div style="display:flex;align-items:center;gap:4px;margin-bottom:2px;">
        <span style="color:var(--muted);font-size:9px;width:32px;">Temp</span>
        <div class="strack"><div class="sfill" data-sensor="temp"></div></div>
        <span style="color:var(--dim);font-size:10px;min-width:36px;text-align:right;">${S.cpu.temp.toFixed(0)}°C</span>
      </div>`;
    }
    if (S.cpu.power) {
      const pp = Math.min(S.cpu.power / 200 * 100, 100);
      sh += `<div style="display:flex;align-items:center;gap:4px;">
        <span style="color:var(--muted);font-size:9px;width:32px;">Powr</span>
        <div class="strack"><div class="sfill" data-sensor="power"></div></div>
        <span style="color:var(--dim);font-size:10px;min-width:36px;text-align:right;">${S.cpu.power.toFixed(0)}W</span>
      </div>`;
    }
    // SAFE: `sh` is assembled just above from literals and numbers
    // formatted with toFixed(); no external string reaches it.
    sensorsEl.innerHTML = sh;
    /* Apply widths via DOM */
    sensorsEl.querySelectorAll('.sfill').forEach(el => {
      if (el.dataset.sensor === 'temp') {
        const tp = Math.min(S.cpu.temp / 105 * 100, 100);
        el.style.width = tp + '%';
        el.style.background = S.cpu.temp > 90 ? 'var(--crit)' : S.cpu.temp > 75 ? 'var(--warn)' : 'var(--warn)';
      } else {
        const pp = Math.min(S.cpu.power / 200 * 100, 100);
        el.style.width = pp + '%';
        el.style.background = 'var(--net-up)';
      }
    });
  } else {
    sensorsEl.style.display = 'none';
  }
  drawArea(document.getElementById('sp-cpu'),S.cpu.hist.map(d=>d.mean),'#38bdf8',0,100,null,null,null,true);
}

function upGPU(){
  if(!S.gpus.length){
    /* No GPU detected — hide the placeholder tabs and stat rows, show an
       inline empty-state mirroring the TUI: chip-name title + unified-memory body.
       We re-render until PLATFORM_INFO actually arrives over IPC, so we don't
       lock in "No GPU" before the chip name is known. */
    const tabs    = document.querySelector('.gpu-tabs');
    const statsEl = document.getElementById('gpu-stats');
    const sparkEl = document.getElementById('sp-gpu');
    if (tabs)    tabs.style.display    = 'none';
    if (sparkEl) sparkEl.style.display = 'none';
    if (statsEl) {
      const haveInfo = !!(PLATFORM_INFO && PLATFORM_INFO.title);
      const title = haveInfo ? PLATFORM_INFO.title : 'No GPU';
      const body  = haveInfo ? PLATFORM_INFO.body  : 'No GPU detected';
      const stamp = haveInfo ? 'platform' : 'fallback';
      if (statsEl.dataset.emptyStamp !== stamp) {
        // SAFE: both values are escaped. They originate from the OS
        // (sysctl machdep.cpu.brand_string on macOS), not from a literal.
        statsEl.innerHTML =
          `<div class="gpu-empty">` +
            `<div class="title">${esc(title)}</div>` +
            `<div class="body">${esc(body)}</div>` +
          `</div>`;
        statsEl.dataset.emptyStamp = stamp;
      }
    }
    return;
  }
  if(!GPUS.length) return;
  const g=S.gpu;

  let util, vramPct, vramLabel, temp, tempMax, power, powerPct, hist;

  if(g==='all'){
    const n=S.gpus.length;
    util=S.gpus.reduce((s,g)=>s+g.util,0)/n;
    const totalVram=GPUS.reduce((s,g)=>s+g.vramMax,0)||1;
    const usedVram=S.gpus.reduce((s,g)=>s+g.vram,0);
    vramPct=(usedVram/totalVram)*100;
    vramLabel=fmtMB(usedVram);
    temp=Math.max(...S.gpus.map(g=>g.temp));
    tempMax=100;
    const totalPow=S.gpus.reduce((s,g)=>s+g.power,0);
    const maxPow=GPUS.reduce((s,g)=>s+g.powerMax,0)||1;
    power=totalPow;
    powerPct=(totalPow/maxPow)*100;
    if(n>=2){
      hist=S.gpus[0].hist.map((v,i)=>(v+(S.gpus[1].hist[i]||0))/2);
    } else {
      hist=S.gpus[0].hist;
    }
  } else {
    const gi=parseInt(g);
    const gd=S.gpus[gi]; if(!gd) return;
    const gdef=GPUS[gi]; if(!gdef) return;
    util=gd.util;
    vramPct=(gd.vram/gdef.vramMax)*100;
    vramLabel=fmtMB(gd.vram);
    temp=gd.temp;
    tempMax=100;
    power=gd.power;
    powerPct=(gd.power/gdef.powerMax)*100;
    hist=S.gpus[gi].hist;
  }

  document.getElementById('g-util').style.width=util+'%';
  document.getElementById('g-util-v').textContent=util.toFixed(1)+'%';
  document.getElementById('g-vram').style.width=vramPct+'%';
  document.getElementById('g-vram-v').textContent=vramLabel;
  document.getElementById('g-temp').style.width=(temp/tempMax*100)+'%';
  document.getElementById('g-temp-v').textContent=temp.toFixed(1)+'°C';
  document.getElementById('g-powr').style.width=powerPct+'%';
  document.getElementById('g-powr-v').textContent=power.toFixed(0)+'W';

  const gpuMax=Math.max(1,...hist);
  drawArea(document.getElementById('sp-gpu'),hist,'#a78bfa',0,gpuMax,null,null,null,true);
}

function upMEM(){
  const pct=(S.mem.usedGB/S.mem.totalGB)*100;
  document.getElementById('m-used').style.width=pct+'%';
  document.getElementById('m-used-v').textContent=pct.toFixed(1)+'%';
  document.getElementById('mem-sub').textContent=S.mem.usedGB.toFixed(1)+' / '+S.mem.totalGB+' GB';
  const swapRow=document.getElementById('swap-row');
  if(swapRow) swapRow.style.display=S.mem.swapPct>0.1?'':'none';
  drawArea(document.getElementById('sp-mem'),S.mem.hist,'#34d399',0,100,null,null,null,true);
}

function upNET(){
  document.getElementById('n-dn').textContent=fmtB(S.net.down);
  document.getElementById('n-up').textContent=fmtB(S.net.up);
  if(S.net.ifName) document.getElementById('net-iface').textContent=S.net.ifName;
  drawDual(document.getElementById('sp-net'),S.net.dh,S.net.uh,'#38bdf8','#fb923c');
}

let activeCat='all';
function getFilteredProcs() {
  const q = searchQuery.toLowerCase();
  return [...PROCS]
    .filter(p => activeCat === 'all' || p.cat === activeCat)
    .filter(p => !q || p.name.toLowerCase().includes(q))
    .sort((a, b) => {
      let cmp = 0;
      switch (S.sortBy) {
        case 'name': cmp = a.name.toLowerCase().localeCompare(b.name.toLowerCase()); break;
        case 'pid': cmp = a.pid - b.pid; break;
        case 'cpu': cmp = a.cpu - b.cpu; break;
        case 'vram': cmp = (a.vram || 0) - (b.vram || 0); break;
        default: cmp = a.mem - b.mem;
      }
      return S.sortAsc ? cmp : -cmp;
    });
}

/* Process-row reconciliation cache.
   Rebuilding the whole <tbody> via innerHTML='' twice a second was pegging
   WebKitWebProcess at ~100% CPU on Linux. Instead we keep a stable cache
   keyed by PID (or "g:<name>" for group headers) and only mutate the cells
   that actually changed. Click handling moved to a single delegated listener
   below. */
const _rowCache = new Map(); // key -> { tr, sig, kind }
const _rowDot = {ai:'●',dev:'■',watch:'★',null:'·'};
const _rowDotCls = {ai:'ai',dev:'dev',watch:'watch',null:'none'};

/* ═══ PLUGIN DOCK ════════════════════════════════════════ */
/* Every string rendered below originates in a plugin's stdout. The Rust side
   bounds it at ingest (plugin::sanitize — 8 panels, 16 entries/panel, 64/128
   char strings, control characters stripped), but it is still third-party
   text, so this renderer is built entirely from createElement + textContent.
   There is deliberately no innerHTML and no template interpolation anywhere in
   this section: a plugin that reports a name of `<img onerror=...>` renders as
   those literal characters. */

/* Whitelists, so a plugin can't put an arbitrary token into a class attribute
   even by accident. Unknown values fall through to the neutral style. */
const _PD_STATE_CLS = {healthy:'ok', starting:'idle', unhealthy:'warn', crashed:'crit'};
const _PD_ENTRY_CLS = {accent:'pd-accent', dim:'pd-dim', warn:'pd-warn', error:'pd-crit'};
let _pdSig = '';

function el(tag, cls, text) {
  const n = document.createElement(tag);
  if (cls) n.className = cls;
  if (text !== undefined) n.textContent = text;
  return n;
}

/* Cheap change detector. The dock repaints at most once a second and holds a
   handful of rows, but a plugin reporting a static panel is the common case —
   no reason to rebuild the subtree (and lose the user's scroll position) when
   nothing moved. */
function pdSig(list) {
  return list.map(s => {
    const panels = (s.response && s.response.panels) || [];
    return s.display_name + '' + s.state + '' + panels.map(p =>
      p.label + '' + (p.content || []).map(e => e.key + '=' + e.value + '#' + e.style).join('')
    ).join('');
  }).join('');
}

/* Entries for one panel, appended to `line`. Mirrors the TUI dock: dim key,
   styled value. */
function pdEntries(line, panel) {
  for (const e of (panel.content || [])) {
    const kv = el('span', 'pd-kv');
    if (e.key) kv.appendChild(el('span', 'pd-k', e.key));
    kv.appendChild(el('span', 'pd-v ' + (_PD_ENTRY_CLS[e.style] || ''), e.value));
    line.appendChild(kv);
  }
}

function pdItem(s) {
  const panels = (s.response && s.response.panels) || [];
  const item = el('div', 'pd-item');

  /* Head line: status dot, plugin name, first panel inline (same density
     trade-off the TUI dock makes), then the state word when it isn't
     healthy. */
  const head = el('div', 'pd-line');
  head.appendChild(el('span', 'pd-dot ' + (_PD_STATE_CLS[s.state] || 'idle'), '●'));
  head.appendChild(el('span', 'pd-name', (s.display_name || s.name || '').toUpperCase()));
  if (panels[0]) pdEntries(head, panels[0]);
  if (s.state !== 'healthy') {
    head.appendChild(el('span', 'pd-state ' + (_PD_STATE_CLS[s.state] || 'idle'), s.state));
  }
  item.appendChild(head);

  /* Remaining panels, one line each. */
  for (let i = 1; i < panels.length; i++) {
    const line = el('div', 'pd-line pd-sub');
    line.appendChild(el('span', 'pd-plabel', panels[i].label));
    pdEntries(line, panels[i]);
    item.appendChild(line);
  }
  return item;
}

function upPlugins() {
  const countEl = document.getElementById('plugin-count');
  const emptyEl = document.getElementById('plugin-empty');
  const liveEl  = document.getElementById('plugin-live');
  if (!countEl || !emptyEl || !liveEl) return;

  const list = PLUGINS;
  const healthy = list.filter(s => s.state === 'healthy').length;
  countEl.textContent = list.length === 0 ? '0 active' : healthy + '/' + list.length + ' active';

  emptyEl.hidden = list.length > 0;
  liveEl.hidden  = list.length === 0;
  if (list.length === 0) {
    if (_pdSig !== '') { liveEl.replaceChildren(); _pdSig = ''; }
    return;
  }

  const sig = pdSig(list);
  if (sig === _pdSig) return;
  _pdSig = sig;

  const frag = document.createDocumentFragment();
  for (const s of list) frag.appendChild(pdItem(s));
  liveEl.replaceChildren(frag);
}

/* END PLUGIN DOCK — the marker above and this one bound the region the
   `plugin_dock_renderer_has_no_html_sinks_at_all` guard scans. Keep dock
   rendering inside them. */

function _aiHtml(ai) {
  if (ai === 'inf') return '<span class="badge inf"><span class="bd">●</span> infer</span>';
  if (ai === 'ild') return '<span class="badge ild">○ idle</span>';
  return '<span style="color:var(--muted)">—</span>';
}
function _vramHtml(v) {
  return v ? `<span class="pvram">${fmtMB(v)}</span>` : '<span class="pvram none">—</span>';
}
function _cpuCls(cpu) { return cpu>20?'crit':cpu>12?'warn':''; }

function upProcs(){
  const sorted = getFilteredProcs();

  // Update tree toggle button
  const treeBtn = document.getElementById('tree-toggle-btn');
  treeBtn.classList.toggle('on', groupedView);

  // Update search count
  const countEl = document.getElementById('search-count');
  if (searchVisible) {
    countEl.textContent = sorted.length + ' match' + (sorted.length === 1 ? '' : 'es');
  }

  // Update kill bar
  const killBar = document.getElementById('kill-bar');
  const killInfo = document.getElementById('kill-info');
  if (selectedPid && sorted.find(p => p.pid === selectedPid)) {
    const sel = sorted.find(p => p.pid === selectedPid);
    killBar.classList.remove('hidden');
    // SAFE: sel.name is escaped; sel.pid is a number from the collector.
    killInfo.innerHTML = `<b>${esc(sel.name)}</b> (PID ${sel.pid})`;
    document.getElementById('kill-selected-btn').style.display = '';
    document.getElementById('kill-all-btn').style.display =
      (searchQuery || activeCat !== 'all') && sorted.length > 1 ? '' : 'none';
    document.getElementById('kill-all-btn').textContent = `Kill All (${sorted.length})`;
  } else {
    if ((searchQuery || activeCat !== 'all') && sorted.length > 0) {
      killBar.classList.remove('hidden');
      // SAFE: array length, a number.
      killInfo.innerHTML = `<b>${sorted.length}</b> processes match`;
      document.getElementById('kill-selected-btn').style.display = 'none';
      document.getElementById('kill-all-btn').style.display = '';
      document.getElementById('kill-all-btn').textContent = `Kill All (${sorted.length})`;
    } else {
      killBar.classList.add('hidden');
    }
    if (selectedPid) { selectedPid = null; }
  }

  const tb = document.getElementById('proctbody');
  const descriptors = groupedView ? buildGroupDescriptors(sorted) : sorted.map(p => ({key:'p:'+p.pid, kind:'proc', p, isChild:false}));
  reconcileRows(tb, descriptors);
}

function buildGroupDescriptors(sorted) {
  const groups = [];
  const groupMap = {};
  sorted.forEach(p => {
    if (groupMap[p.name] !== undefined) {
      groups[groupMap[p.name]].procs.push(p);
    } else {
      groupMap[p.name] = groups.length;
      groups.push({ name: p.name, procs: [p] });
    }
  });
  groups.sort((a, b) => {
    let cmp = 0;
    switch (S.sortBy) {
      case 'name': cmp = a.name.toLowerCase().localeCompare(b.name.toLowerCase()); break;
      case 'pid':  cmp = a.procs[0].pid - b.procs[0].pid; break;
      case 'cpu':  cmp = a.procs.reduce((s,p)=>s+p.cpu,0) - b.procs.reduce((s,p)=>s+p.cpu,0); break;
      case 'vram': cmp = a.procs.reduce((s,p)=>s+(p.vram||0),0) - b.procs.reduce((s,p)=>s+(p.vram||0),0); break;
      default:     cmp = a.procs.reduce((s,p)=>s+p.mem,0) - b.procs.reduce((s,p)=>s+p.mem,0);
    }
    return S.sortAsc ? cmp : -cmp;
  });
  const out = [];
  const catPriority = {watch:0, ai:1, dev:2, null:3};
  groups.forEach(g => {
    if (g.procs.length === 1) {
      out.push({key:'p:'+g.procs[0].pid, kind:'proc', p:g.procs[0], isChild:false});
      return;
    }
    const cpuTotal = g.procs.reduce((s,p) => s+p.cpu, 0);
    const memTotal = g.procs.reduce((s,p) => s+p.mem, 0);
    const vramTotal = g.procs.reduce((s,p) => s+(p.vram||0), 0);
    const bestCat = g.procs.reduce((best, p) => {
      const pc = catPriority[p.cat||'null']||3;
      const bc = catPriority[best||'null']||3;
      return pc < bc ? p.cat : best;
    }, null);
    out.push({key:'g:'+g.name, kind:'group', g, cpuTotal, memTotal, vramTotal, bestCat, expanded:expandedGroups.has(g.name)});
    if (expandedGroups.has(g.name)) {
      g.procs.forEach(p => out.push({key:'p:'+p.pid+':c', kind:'proc', p, isChild:true}));
    }
  });
  return out;
}

function reconcileRows(tb, descriptors) {
  const seen = new Set();
  descriptors.forEach((d, idx) => {
    seen.add(d.key);
    let entry = _rowCache.get(d.key);
    const sig = sigFor(d);
    if (!entry || entry.kind !== d.kind) {
      if (entry) entry.tr.remove();
      entry = createRow(d);
      _rowCache.set(d.key, entry);
    } else if (entry.sig !== sig) {
      updateRow(entry, d);
      entry.sig = sig;
    }
    if (tb.children[idx] !== entry.tr) {
      tb.insertBefore(entry.tr, tb.children[idx] || null);
    }
  });
  for (const [key, entry] of _rowCache) {
    if (!seen.has(key)) { entry.tr.remove(); _rowCache.delete(key); }
  }
}

function sigFor(d) {
  if (d.kind === 'group') {
    return `g|${d.bestCat||''}|${d.expanded?1:0}|${d.cpuTotal.toFixed(1)}|${d.memTotal.toFixed(0)}|${d.vramTotal.toFixed(0)}|${d.g.procs.length}`;
  }
  const p = d.p;
  return `p|${p.cat||''}|${p.ai||''}|${p.cpu.toFixed(1)}|${p.mem.toFixed(0)}|${p.vram||0}|${p.pid===selectedPid?1:0}|${d.isChild?1:0}|${p.name}|${p.label||''}`;
}

function createRow(d) {
  if (d.kind === 'group') return createGroupRow(d);
  return createProcRow(d);
}

function createProcRow(d) {
  const p = d.p;
  const cat = p.cat || 'null';
  const tr = document.createElement('tr');
  tr.dataset.key = d.key;
  tr.dataset.pid = p.pid;
  if (d.isChild) tr.dataset.child = '1';
  tr.className = 'proc-row '+(p.cat?'cat-'+p.cat:'default') + (p.pid===selectedPid?' selected':'') + (d.isChild?' group-child':'');
  const dotHtml = d.isChild ? '' : `<span class="cat-dot ${_rowDotCls[cat]}">${_rowDot[cat]}</span>`;
  // SAFE: every interpolation is a lookup into a fixed table (_rowDotCls,
  // _rowDot, _cpuCls) or a number formatted by toFixed/fmtMB. The one
  // OS-controlled value — p.name — is deliberately left out and written as
  // textContent below.
  tr.innerHTML =
    `<td>${dotHtml}<span class="pname"></span><span class="plabel"></span></td>` +
    `<td class="ppid">${p.pid}</td>` +
    `<td class="pcpu ${_cpuCls(p.cpu)}">${p.cpu.toFixed(1)}</td>` +
    `<td class="pmem">${fmtMB(p.mem)}</td>` +
    `<td class="pvramcell">${_vramHtml(p.vram)}</td>` +
    `<td class="paicell">${_aiHtml(p.ai)}<button class="row-kill" title="Kill process">✕</button></td>`;
  // A process can name itself anything, including markup — and a plugin label
  // is third-party text too. Both go in as textContent, never innerHTML.
  tr.querySelector('.pname').textContent = p.name;
  if (p.label) tr.querySelector('.plabel').textContent = p.label;
  return { tr, sig: sigFor(d), kind: 'proc' };
}

function updateProcRow(entry, d) {
  const p = d.p;
  const tr = entry.tr;
  const cells = tr.children;
  const prev = entry.prev || (entry.prev = {});
  const wantCls = 'proc-row '+(p.cat?'cat-'+p.cat:'default') + (p.pid===selectedPid?' selected':'') + (d.isChild?' group-child':'');
  if (prev.cls !== wantCls) { tr.className = wantCls; prev.cls = wantCls; }
  // CPU% changes essentially every frame on a busy box — its cell does need
  // to update each call. The other cells (mem, vram, ai badge) change far
  // less often; gate them with `prev.*` to avoid blowing away DOM (and
  // re-parsing innerHTML for ~500 rows) when the data is unchanged.
  if (prev.cpu !== p.cpu) {
    const wantCpuCls = 'pcpu ' + _cpuCls(p.cpu);
    if (prev.cpuCls !== wantCpuCls) { cells[2].className = wantCpuCls; prev.cpuCls = wantCpuCls; }
    cells[2].textContent = p.cpu.toFixed(1);
    prev.cpu = p.cpu;
  }
  if (prev.mem !== p.mem) { cells[3].textContent = fmtMB(p.mem); prev.mem = p.mem; }
  // A plugin can attach, change, or drop a label between polls (an Ollama
  // model unloading, say), so the badge has to be updatable, not create-only.
  const labelKey = p.label || '';
  if (prev.label !== labelKey) {
    const badge = cells[0].querySelector('.plabel');
    if (badge) badge.textContent = labelKey;
    prev.label = labelKey;
  }
  const vramKey = p.vram || 0;
    // SAFE: _vramHtml interpolates only fmtMB() of a number.
  if (prev.vram !== vramKey) { cells[4].innerHTML = _vramHtml(p.vram); prev.vram = vramKey; }
  const aiKey = p.ai || '';
  if (prev.ai !== aiKey) {
    const aiCell = cells[5];
    const killBtn = aiCell.querySelector('.row-kill');
    // SAFE: _aiHtml returns one of three string literals.
    aiCell.innerHTML = _aiHtml(p.ai);
    if (killBtn) aiCell.appendChild(killBtn);
    // SAFE: string literal, no interpolation.
    else aiCell.insertAdjacentHTML('beforeend', '<button class="row-kill" title="Kill process">✕</button>');
    prev.ai = aiKey;
  }
}

function createGroupRow(d) {
  const g = d.g;
  const cat = d.bestCat || 'null';
  const catCls = d.bestCat ? 'cat-'+d.bestCat : 'default';
  const arrow = d.expanded ? '▾' : '▸';
  const tr = document.createElement('tr');
  tr.dataset.key = d.key;
  tr.dataset.group = g.name;
  tr.className = `proc-row group-header ${catCls}`;
  // SAFE: fixed-table lookups and numbers only. The group name is
  // OS-controlled and appears in two places here — as text and inside a
  // title="" attribute — so both are filled in after the parse, below.
  tr.innerHTML =
    `<td><span class="group-arrow">${arrow}</span><span class="cat-dot ${_rowDotCls[cat]}">${_rowDot[cat]}</span><span class="pname"><span class="group-count">(${g.procs.length})</span></span></td>` +
    `<td class="ppid"></td>` +
    `<td class="pcpu ${_cpuCls(d.cpuTotal)}">${d.cpuTotal.toFixed(1)}</td>` +
    `<td class="pmem">${fmtMB(d.memTotal)}</td>` +
    `<td class="pvramcell">${d.vramTotal>0?`<span class="pvram">${fmtMB(d.vramTotal)}</span>`:'<span class="pvram none">—</span>'}</td>` +
    `<td class="paicell"><button class="row-kill">✕</button></td>`;
  // Text node before the count span, reproducing the original DOM exactly:
  // <span class="pname">NAME<span class="group-count">(n)</span></span>.
  const pnameEl = tr.querySelector('.pname');
  pnameEl.insertBefore(document.createTextNode(g.name), pnameEl.firstChild);
  // Attribute context: a name containing a double quote would otherwise close
  // title="" and open an event-handler attribute. setAttribute takes the value
  // literally, so no quoting question arises.
  tr.querySelector('.row-kill').setAttribute('title', `Kill all ${g.name}`);
  return { tr, sig: sigFor(d), kind: 'group' };
}

function updateGroupRow(entry, d) {
  const g = d.g;
  const tr = entry.tr;
  const cells = tr.children;
  const prev = entry.prev || (entry.prev = {});
  const catCls = d.bestCat ? 'cat-'+d.bestCat : 'default';
  const wantCls = `proc-row group-header ${catCls}`;
  if (prev.cls !== wantCls) { tr.className = wantCls; prev.cls = wantCls; }
  if (prev.expanded !== d.expanded) {
    const arrowEl = cells[0].querySelector('.group-arrow');
    if (arrowEl) arrowEl.textContent = d.expanded ? '▾' : '▸';
    prev.expanded = d.expanded;
  }
  if (prev.count !== g.procs.length) {
    const countEl = cells[0].querySelector('.group-count');
    if (countEl) countEl.textContent = `(${g.procs.length})`;
    prev.count = g.procs.length;
  }
  if (prev.cpu !== d.cpuTotal) {
    const wantCpuCls = 'pcpu ' + _cpuCls(d.cpuTotal);
    if (prev.cpuCls !== wantCpuCls) { cells[2].className = wantCpuCls; prev.cpuCls = wantCpuCls; }
    cells[2].textContent = d.cpuTotal.toFixed(1);
    prev.cpu = d.cpuTotal;
  }
  if (prev.mem !== d.memTotal) { cells[3].textContent = fmtMB(d.memTotal); prev.mem = d.memTotal; }
  if (prev.vram !== d.vramTotal) {
    // SAFE: fmtMB() of a number, or a literal.
    cells[4].innerHTML = d.vramTotal > 0 ? `<span class="pvram">${fmtMB(d.vramTotal)}</span>` : '<span class="pvram none">—</span>';
    prev.vram = d.vramTotal;
  }
}

function updateRow(entry, d) {
  if (d.kind === 'group') updateGroupRow(entry, d);
  else updateProcRow(entry, d);
}

/* Single delegated click listener for the whole table — replaces ~2N
   per-row listeners that were re-bound on every render. */
document.getElementById('proctbody').addEventListener('click', (e) => {
  const tr = e.target.closest('tr.proc-row');
  if (!tr) return;
  const isKill = e.target.classList.contains('row-kill');
  if (tr.classList.contains('group-header')) {
    const name = tr.dataset.group;
    if (!name) return;
    if (isKill) {
      e.stopPropagation();
      const procs = PROCS.filter(p => p.name === name);
      showKillConfirm('batch', procs.map(p => ({pid:p.pid, name:p.name})));
      return;
    }
    if (expandedGroups.has(name)) expandedGroups.delete(name);
    else expandedGroups.add(name);
    upProcs();
    return;
  }
  const pid = parseInt(tr.dataset.pid);
  if (!pid) return;
  if (isKill) {
    e.stopPropagation();
    const proc = PROCS.find(p => p.pid === pid);
    if (proc) showKillConfirm('single', [{pid:proc.pid, name:proc.name}]);
    return;
  }
  selectedPid = (selectedPid === pid) ? null : pid;
  document.getElementById('kill-selected-btn').style.display = '';
  upProcs();
});

/* ═══ CONTROLS ═══════════════════════════════════════════ */
document.querySelectorAll('.tab').forEach(btn=>{
  btn.addEventListener('click',()=>{
    document.querySelectorAll('.tab').forEach(b=>b.classList.remove('on'));
    btn.classList.add('on'); S.metric=btn.dataset.m; upMainChart(); saveSettings();
  });
});

/* Sort — shared helper */
function setSort(col){
  if(S.sortBy===col) S.sortAsc=!S.sortAsc;
  else { S.sortBy=col; S.sortAsc=(col==='name'||col==='pid'); }
  document.querySelectorAll('.sort-btn').forEach(b=>b.classList.toggle('on',b.dataset.sort===S.sortBy));
  document.querySelectorAll('.th-sort').forEach(th=>{
    th.classList.toggle('active',th.dataset.sort===S.sortBy);
    th.textContent=th.dataset.sort===S.sortBy
      ? th.dataset.sort.toUpperCase()+(th.dataset.sort==='cpu'?'%':'')+(S.sortAsc?' ▲':' ▼')
      : ({name:'NAME',pid:'PID',cpu:'CPU%',mem:'MEM',vram:'VRAM'}[th.dataset.sort]);
  });
  upProcs();
  saveSettings();
}

/* Sort buttons */
document.querySelectorAll('.sort-btn').forEach(btn=>{
  btn.addEventListener('click',()=>setSort(btn.dataset.sort));
});

/* Clickable column headers */
document.querySelectorAll('.th-sort').forEach(th=>{
  th.addEventListener('click',()=>setSort(th.dataset.sort));
});

/* Category filter tabs */
document.querySelectorAll('.cat-tab').forEach(btn=>{
  btn.addEventListener('click',()=>{
    activeCat=btn.dataset.cat;
    document.querySelectorAll('.cat-tab').forEach(b=>b.classList.remove('on'));
    btn.classList.add('on'); upProcs(); saveSettings();
  });
});

/* ═══ SEARCH ════════════════════════════════════════════ */
const searchBar = document.getElementById('proc-search');
const searchInput = document.getElementById('search-input');

function openSearch() {
  searchVisible = true;
  searchBar.classList.remove('hidden');
  searchInput.focus();
}
function closeSearch() {
  searchVisible = false;
  searchQuery = '';
  searchInput.value = '';
  searchBar.classList.add('hidden');
  selectedPid = null;
  upProcs();
}

searchInput.addEventListener('input', () => {
  searchQuery = searchInput.value;
  selectedPid = null;
  upProcs();
});
searchInput.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') { closeSearch(); e.preventDefault(); }
  if (e.key === 'Enter') { searchInput.blur(); e.preventDefault(); }
  e.stopPropagation(); // prevent global shortcuts while typing
});
document.getElementById('search-clear').addEventListener('click', closeSearch);

/* ═══ KILL ACTIONS ══════════════════════════════════════ */
document.getElementById('kill-selected-btn').addEventListener('click', () => {
  if (!selectedPid) return;
  const p = PROCS.find(p => p.pid === selectedPid);
  if (p) showKillConfirm('single', [{pid:p.pid, name:p.name}]);
});
document.getElementById('kill-all-btn').addEventListener('click', () => {
  const filtered = getFilteredProcs();
  if (filtered.length === 0) return;
  showKillConfirm('batch', filtered.map(p => ({pid:p.pid, name:p.name})));
});

function showKillConfirm(type, targets) {
  pendingKill = {type, targets};
  const modal = document.getElementById('kill-modal');
  const title = document.getElementById('kill-modal-title');
  const msg = document.getElementById('kill-modal-msg');
  if (type === 'single') {
    title.textContent = 'CONFIRM KILL';
    // SAFE: name is escaped; pid is a number.
    msg.innerHTML = `Kill <b>${esc(targets[0].name)}</b> (PID ${targets[0].pid})?`;
  } else {
    title.textContent = 'CONFIRM KILL ALL';
    const unique = [...new Set(targets.map(t => t.name))];
    const preview = unique.slice(0,4).join(', ') + (unique.length > 4 ? ', ...' : '');
    // SAFE: targets.length is a number; the name preview is escaped here at
    // the sink, where it is visible, rather than upstream where it isn't.
    msg.innerHTML = `Kill <b>${targets.length}</b> processes (${esc(preview)})?`;
  }
  modal.classList.add('visible');
}

document.getElementById('kill-modal-yes').addEventListener('click', async () => {
  document.getElementById('kill-modal').classList.remove('visible');
  if (!pendingKill || !invoke) return;
  try {
    if (pendingKill.type === 'single') {
      const result = await invoke('kill_process', {pid: pendingKill.targets[0].pid});
      showToast(result);
    } else {
      const pids = pendingKill.targets.map(t => t.pid);
      const result = await invoke('kill_processes', {pids});
      showToast(result);
    }
  } catch(e) {
    showToast(String(e));
  }
  pendingKill = null;
  selectedPid = null;
  upProcs();
});
document.getElementById('kill-modal-no').addEventListener('click', () => {
  document.getElementById('kill-modal').classList.remove('visible');
  pendingKill = null;
});
document.getElementById('kill-modal').addEventListener('click', (e) => {
  if (e.target.id === 'kill-modal') {
    e.target.classList.remove('visible');
    pendingKill = null;
  }
});

/* Tree toggle */
document.getElementById('tree-toggle-btn').addEventListener('click', () => {
  groupedView = !groupedView;
  upProcs();
});

/* GPU panel tabs */
document.querySelectorAll('.gpu-tab').forEach(btn=>{
  btn.addEventListener('click',()=>{
    S.gpu=btn.dataset.g;
    document.querySelectorAll('.gpu-tab').forEach(b=>b.classList.remove('on'));
    btn.classList.add('on'); upGPU();
  });
});

/* ═══ KEYBOARD SHORTCUTS ═════════════════════════════════ */
document.addEventListener('keydown', e => {
  const k = e.key.toLowerCase();

  // If kill modal is open, y confirms, anything else cancels
  const killModal = document.getElementById('kill-modal');
  if (killModal.classList.contains('visible')) {
    if (k === 'y') document.getElementById('kill-modal-yes').click();
    else document.getElementById('kill-modal-no').click();
    e.preventDefault();
    return;
  }

  // If any overlay is open, any key closes it
  const helpEl = document.getElementById('help-overlay');
  const aboutEl = document.getElementById('about-overlay');
  if (helpEl.classList.contains('visible')) {
    helpEl.classList.remove('visible');
    e.preventDefault();
    return;
  }
  if (aboutEl.classList.contains('visible')) {
    aboutEl.classList.remove('visible');
    e.preventDefault();
    return;
  }

  // Search: / opens, Escape clears
  if (e.key === '/' && document.activeElement !== searchInput) {
    e.preventDefault();
    openSearch();
    return;
  }
  if (e.key === 'Escape' && searchVisible) {
    closeSearch();
    e.preventDefault();
    return;
  }

  // Kill selected: x or Delete
  if ((k === 'x' && !e.shiftKey) || e.key === 'Delete') {
    if (selectedPid) {
      const p = PROCS.find(p => p.pid === selectedPid);
      if (p) showKillConfirm('single', [{pid:p.pid, name:p.name}]);
    }
    return;
  }

  // Kill all matching: X (shift+x)
  if (e.key === 'X' && e.shiftKey) {
    const filtered = getFilteredProcs();
    if (filtered.length > 0 && (searchQuery || activeCat !== 'all')) {
      showKillConfirm('batch', filtered.map(p => ({pid:p.pid, name:p.name})));
    }
    return;
  }

  // Toggle tree/grouped view: t
  if (k === 't') {
    groupedView = !groupedView;
    upProcs();
    return;
  }

  // Toggle horizon chart mode: h
  if (k === 'h') {
    S.chartMode = S.chartMode === 'default' ? 'horizon' : 'default';
    upMainChart(); saveSettings();
    return;
  }

  // Chart tab switching: c/g/m/n
  const chartKeys = {c:'cpu', g:'gpu', m:'mem', n:'net', d:'disk'};
  if (chartKeys[k]) {
    S.metric = chartKeys[k];
    document.querySelectorAll('.tab').forEach(b => {
      b.classList.toggle('on', b.dataset.m === chartKeys[k]);
    });
    upMainChart(); saveSettings();
    return;
  }

  // Category filter: 1-4
  const catKeys = {'1':'all', '2':'ai', '3':'dev', '4':'watch'};
  if (catKeys[k]) {
    activeCat = catKeys[k];
    document.querySelectorAll('.cat-tab').forEach(b => {
      b.classList.toggle('on', b.dataset.cat === catKeys[k]);
    });
    upProcs(); saveSettings();
    return;
  }

  // Sort cycling: tab
  if (k === 'tab') {
    e.preventDefault();
    const order = ['name','pid','cpu','mem','vram'];
    const idx = order.indexOf(S.sortBy);
    setSort(order[(idx + 1) % order.length]);
    return;
  }

  // Resize panes: [ / ]
  const wl = document.getElementById('watchlist-panel');
  if (wl && (k === '[' || k === ']')) {
    const cur = wl.offsetWidth;
    const mainW = document.querySelector('.main').offsetWidth;
    const delta = k === ']' ? -30 : 30;
    wl.style.width = Math.max(200, Math.min(mainW - 200, cur + delta)) + 'px';
    saveSettings();
    return;
  }

  // Save snapshot: s
  if (k === 's' && invoke) {
    invoke('get_snapshot').then(snap => {
      const ts = new Date().toISOString().replace(/[:.]/g,'-');
      const txt = `Dofek snapshot — ${ts}\n\nCPU: ${snap.cpu.name} — ${snap.cpu.total_load.toFixed(1)}%\nMemory: ${snap.memory.used_gb.toFixed(1)} / ${snap.memory.total_gb.toFixed(1)} GB (${snap.memory.used_percent.toFixed(1)}%)\nGPU: ${snap.gpus.map(g=>g.name+' '+g.utilization.toFixed(1)+'%').join(', ')||'N/A'}\nProcesses: ${snap.processes.length}`;
      const blob = new Blob([txt], {type:'text/plain'});
      const a = document.createElement('a');
      a.href = URL.createObjectURL(blob);
      a.download = `dofek-snapshot-${ts}.txt`;
      a.click();
      URL.revokeObjectURL(a.href);
      showToast('Snapshot saved');
    });
    return;
  }

  // Toggle help: ?
  if (k === '?' || (e.shiftKey && e.key === '?')) {
    const helpEl = document.getElementById('help-overlay');
    if (helpEl.classList.contains('visible')) {
      helpEl.classList.remove('visible');
    } else {
      openSettingsOverlay();
    }
    return;
  }

  // Toggle about: a
  if (k === 'a') {
    document.getElementById('about-overlay').classList.toggle('visible');
    return;
  }

});

/* ═══ RESIZE ═════════════════════════════════════════════ */
const ro=new ResizeObserver(()=>{upMainChart();upCores();upGPU();upMEM();upNET();});
['mainCanvas','sp-cpu','sp-gpu','sp-mem','sp-net'].forEach(id=>{
  const el=document.getElementById(id); if(el) ro.observe(el);
});

/* ═══ PANEL RESIZE DRAG ══════════════════════════════════ */
{
  const handle=document.getElementById('resize-handle');
  const wl=document.getElementById('watchlist-panel');
  const main=document.querySelector('.main');
  let dragging=false, startX=0, startW=0;

  handle.addEventListener('mousedown',e=>{
    dragging=true; startX=e.clientX; startW=wl.offsetWidth;
    handle.classList.add('active');
    document.body.style.cursor='col-resize';
    document.body.style.userSelect='none';
    e.preventDefault();
  });

  document.addEventListener('mousemove',e=>{
    if(!dragging) return;
    const dx=startX-e.clientX;
    const newW=Math.max(200, Math.min(main.offsetWidth-200, startW+dx));
    wl.style.width=newW+'px';
  });

  document.addEventListener('mouseup',()=>{
    if(!dragging) return;
    dragging=false;
    handle.classList.remove('active');
    document.body.style.cursor='';
    document.body.style.userSelect='';
    saveSettings();
  });
}

/* ═══ SETTINGS PERSISTENCE ═══════════════════════════════ */
let saveTimer = null;
function saveSettings() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(async () => {
    if (!invoke) return;
    try {
      const wl = document.getElementById('watchlist-panel');
      const main = document.querySelector('.main');
      const splitPct = (main && wl) ? Math.round((1 - wl.offsetWidth / main.offsetWidth) * 100) : 58;
      await invoke('save_settings', { settings: {
        chart_tab: S.metric,
        chart_mode: S.chartMode,
        sort_column: S.sortBy,
        sort_ascending: S.sortAsc,
        category_filter: activeCat,
        split_pct: splitPct,
        refresh_ms: 500,
      }});
    } catch(e) { console.warn('Settings save failed:', e); }
  }, 2000);
}

async function loadSettings() {
  if (!invoke) return;
  try {
    const s = await invoke('get_settings');
    if (s.chart_tab) S.metric = s.chart_tab;
    if (s.chart_mode) S.chartMode = s.chart_mode;
    if (s.sort_column) S.sortBy = s.sort_column;
    if (typeof s.sort_ascending === 'boolean') S.sortAsc = s.sort_ascending;
    if (s.category_filter) { S.cat = s.category_filter; activeCat = s.category_filter; }

    // Update tab buttons
    document.querySelectorAll('.tab').forEach(b => b.classList.toggle('on', b.dataset.m === S.metric));
    document.querySelectorAll('.cat-tab').forEach(b => b.classList.toggle('on', b.dataset.cat === S.cat));

    // Update sort header highlights
    document.querySelectorAll('.sh').forEach(b => {
      b.classList.toggle('active', b.dataset.s === S.sortBy);
    });

    // Apply split_pct to watchlist panel width
    if (s.split_pct && s.split_pct >= 25 && s.split_pct <= 85) {
      const wl = document.getElementById('watchlist-panel');
      const main = document.querySelector('.main');
      if (wl && main) {
        const mainW = main.offsetWidth;
        wl.style.width = Math.round(mainW * (1 - s.split_pct / 100)) + 'px';
      }
    }
  } catch(e) { console.warn('Settings load failed, using defaults:', e); }
}

/* ═══ MAIN LOOP ══════════════════════════════════════════ */
async function frame(snap){
  // When the window is hidden (close-to-tray, minimised, fully covered)
  // there's no point repainting Canvas/DOM — WebKitGTK still spends real CPU
  // on those operations. The snapshot ring buffers in S keep getting pushed
  // even while hidden (the sample arrives via tick before this gate would
  // matter), but skipping the render dominates the savings.
  if (document.visibilityState === 'hidden') return;
  await tick(snap);
  dashOffset -= 0.6; // animate threshold dashes
  const t=new Date().toTimeString().slice(0,8).split(':');
  // SAFE: t is the digit groups of a formatted Date, split on ':'.
  document.getElementById('clock').innerHTML=t[0]+'<span class="clock-sep">:</span>'+t[1]+'<span class="clock-sep">:</span>'+t[2];
  const fns=[upTopBar, upMainChart, upCores, upGPU, upMEM, upNET, upProcs, upPlugins];
  for(const fn of fns){ try{ fn(); }catch(e){ console.error(fn.name+':',e); } }
}
async function checkTelemetryPrompt() {
  if (!invoke) return;
  try {
    const prompted = await invoke('get_telemetry_prompted');
    if (prompted) return;
    const modal = document.getElementById('telem-modal');
    modal.classList.add('visible');
    await new Promise(resolve => {
      document.getElementById('telem-yes').addEventListener('click', async () => {
        await invoke('set_telemetry_choice', { enabled: true });
        modal.classList.remove('visible');
        resolve();
      });
      document.getElementById('telem-no').addEventListener('click', async () => {
        await invoke('set_telemetry_choice', { enabled: false });
        modal.classList.remove('visible');
        resolve();
      });
    });
  } catch(e) { console.error('Telemetry prompt error:', e); }
}
// Backend pushes a snapshot via the `dofek://snapshot` Tauri event whenever
// the collector produces one. We do one initial frame() (which falls through
// to invoke('get_snapshot')) so the UI hydrates immediately, then hand control
// to the listener — no polling, no duplicate JSON parses.
function startSnapshotListener() {
  if (!tauriListen) acquireTauriApi();
  if (!tauriListen) {
    // Tauri event API not available yet — fall back to polling so the UI
    // still updates. Retried only at startup; once the listener is wired
    // there's no further polling.
    console.warn('Tauri event API not available, falling back to setInterval polling');
    setInterval(() => frame(), 1000);
    return;
  }
  tauriListen('dofek://snapshot', e => { frame(e.payload); }).catch(err => {
    console.error('listen(dofek://snapshot) failed, falling back to polling:', err);
    setInterval(() => frame(), 1000);
  });
}
loadSettings().then(() => checkTelemetryPrompt()).then(() => { frame(); startSnapshotListener(); });
/* ═══ TOAST ══════════════════════════════════════════════ */
function showToast(msg){
  const t=document.getElementById('toast');
  t.textContent=msg; t.classList.add('show');
  setTimeout(()=>t.classList.remove('show'),2000);
}

