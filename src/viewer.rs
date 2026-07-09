use std::fs;
use std::io;
use std::path::Path;

pub fn write_assets(out_dir: &Path) -> io::Result<()> {
    fs::create_dir_all(out_dir)?;
    fs::write(out_dir.join("index.html"), INDEX_HTML)?;
    fs::write(out_dir.join("map-viewer.js"), MAP_VIEWER_JS)?;
    Ok(())
}

const INDEX_HTML: &str = r####"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no">
<title>世界地图</title>
<style>
  * { box-sizing: border-box; margin: 0; padding: 0; }
  html, body { width: 100%; height: 100%; overflow: hidden; }
  body {
    background-color: #101010;
    background-image: conic-gradient(#151515 0.25turn, #101010 0turn 0.5turn, #151515 0turn 0.75turn, #101010 0turn);
    background-size: 24px 24px;
    color: #e0e0e0;
    font-family: "Segoe UI", "PingFang SC", "Microsoft YaHei", system-ui, sans-serif;
    font-size: 13px;
    user-select: none;
  }
  .mono { font-family: ui-monospace, "Cascadia Mono", Consolas, monospace; }

  #map {
    width: 100dvw !important; height: 100dvh !important;
    position: absolute; inset: 0;
    cursor: grab; touch-action: none;
    display: block;
  }
  #map.dragging { cursor: grabbing; }

  /* ── Minecraft 风格面板与按钮 ── */
  .mcpanel {
    position: absolute; z-index: 10;
    background: #2b2b2b;
    border: 2px solid;
    border-color: #4a4a4a #141414 #141414 #4a4a4a;
    box-shadow: 3px 3px 0 rgba(0,0,0,.45);
  }
  .mcbtn {
    font-family: inherit; font-size: 13px;
    color: #e0e0e0; cursor: pointer;
    background: #3a3a3a;
    padding: 4px 12px;
    border: 2px solid;
    border-color: #5a5a5a #171717 #171717 #5a5a5a;
  }
  .mcbtn:hover:not([disabled]) { background: #454545; color: #ffd75e; }
  .mcbtn:active:not([disabled]) {
    border-color: #171717 #5a5a5a #5a5a5a #171717;
  }
  .mcbtn.on {
    border-color: #171717 #5a5a5a #5a5a5a #171717;
    background: #1f2b1f; color: #55ff55;
  }
  .mcbtn[disabled] { color: #6a6a6a; cursor: not-allowed; background: #2c2c2c; }

  #topbar {
    top: 8px; left: 8px; right: 8px;
    display: flex; align-items: center; gap: 8px; flex-wrap: wrap;
    padding: 6px 10px;
  }
  #worldname {
    font-weight: 700; font-size: 15px;
    color: #ffd75e; text-shadow: 2px 2px 0 #3a3000;
    letter-spacing: 1px; margin-right: 4px;
  }
  #dims { display: flex; gap: 6px; }
  .spacer { flex: 1; }
  #genat { color: #8f8f8f; font-size: 12px; }

  #status {
    bottom: 8px; left: 8px;
    display: flex; align-items: center;
    padding: 5px 0;
  }
  #status .cell {
    padding: 2px 12px;
    border-right: 1px solid #171717;
    box-shadow: 1px 0 0 #3d3d3d;
    color: #9f9f9f; white-space: nowrap;
    display: flex; align-items: center; gap: 6px;
  }
  #status .cell:last-child { border-right: none; box-shadow: none; }
  #status b { color: #e0e0e0; font-weight: 600; min-width: 3.5em; display: inline-block; }
  #scaleline {
    display: inline-block; height: 7px;
    border: 2px solid #d8d8d8; border-top: none;
    vertical-align: middle;
  }
  #scaletxt { color: #e0e0e0; }

  #zoomctl {
    position: absolute; z-index: 10;
    bottom: 8px; right: 8px;
    display: flex; flex-direction: column; gap: 4px;
  }
  #zoomctl .mcbtn {
    width: 34px; height: 32px; padding: 0;
    font-size: 16px; line-height: 1;
    box-shadow: 2px 2px 0 rgba(0,0,0,.45);
  }

  #hint {
    position: absolute; inset: 0; z-index: 5;
    display: none; align-items: center; justify-content: center;
    pointer-events: none;
  }
  #hint .mcpanel { position: static; padding: 20px 30px; text-align: center; color: #9f9f9f; }
  #hint b { color: #ffd75e; display: block; margin-bottom: 8px; font-size: 15px; }

  @media (max-width: 640px) {
    #genat { display: none !important; }
    #status .cell { padding: 2px 8px; }
  }
</style>
</head>
<body>
<canvas id="map"></canvas>

<div class="mcpanel" id="topbar">
  <span id="worldname">世界地图</span>
  <div id="dims"></div>
  <button class="mcbtn on" id="gridbtn" title="显示/隐藏坐标网格">网格</button>
  <button class="mcbtn" id="spawnbtn" title="显示/隐藏世界出生点">出生点</button>
  <span class="spacer"></span>
  <span id="genat"></span>
</div>

<div class="mcpanel" id="status">
  <span class="cell">X <b class="mono" id="cxv">-</b></span>
  <span class="cell">Z <b class="mono" id="czv">-</b></span>
  <span class="cell">缩放 <b class="mono" id="zoomtxt">-</b></span>
  <span class="cell"><span id="scaleline"></span><span class="mono" id="scaletxt">-</span></span>
</div>

<div id="zoomctl">
  <button class="mcbtn" id="zin" title="放大">+</button>
  <button class="mcbtn" id="zout" title="缩小">−</button>
  <button class="mcbtn" id="zfit" title="适应视图">⊡</button>
</div>

<div id="hint"><div class="mcpanel"><b>暂无地图数据</b>请先运行 Atlas 生成瓦片</div></div>

<script src="map-viewer.js"></script>
</body>
</html>
"####;

const MAP_VIEWER_JS: &str = r####"(function(){
  "use strict";
  var $ = function(id){ return document.getElementById(id); };
  var canvas = $("map"), ctx = canvas.getContext("2d");
  var dimsEl = $("dims"), worldnameEl = $("worldname"), genatEl = $("genat");
  var cxv = $("cxv"), czv = $("czv"), zoomtxt = $("zoomtxt");
  var scaleline = $("scaleline"), scaletxt = $("scaletxt"), hintEl = $("hint");
  var gridbtn = $("gridbtn"), spawnbtn = $("spawnbtn");

  var manifest = null, dimsById = {}, currentDim = null;
  var tileSize = 512, spawn = null;
  var cam = { x: 0, z: 0, scale: 1 };
  var players = [], tileCache = {};
  var showGrid = true, showSpawn = false;
  var userMoved = false, didFit = false, need = true;
  var dpr = Math.max(1, window.devicePixelRatio || 1);
  var pollMs = 1000;

  function requestDraw(){ need = true; }
  function cw(){ return canvas.width; }
  function ch(){ return canvas.height; }

  function resize(){
    var r = canvas.getBoundingClientRect();
    dpr = Math.max(1, window.devicePixelRatio || 1);
    canvas.width = Math.max(1, Math.round(r.width * dpr));
    canvas.height = Math.max(1, Math.round(r.height * dpr));
    requestDraw();
  }
  window.addEventListener("resize", resize);

  function sx(wx){ return (wx - cam.x) * cam.scale * dpr + cw()/2; }
  function sz(wz){ return (wz - cam.z) * cam.scale * dpr + ch()/2; }
  function wxAt(px){ return (px - cw()/2) / (cam.scale*dpr) + cam.x; }
  function wzAt(pz){ return (pz - ch()/2) / (cam.scale*dpr) + cam.z; }

  function dimName(d){ return d===1 ? "nether" : (d===2 ? "end" : "overworld"); }
  function esc(s){ return String(s).replace(/[&<>"]/g, function(c){
    return ({ "&":"&amp;", "<":"&lt;", ">":"&gt;", '"':"&quot;" })[c];
  }); }

  function pickDim(){
    if (currentDim && dimsById[currentDim]) return;
    var order = ["overworld","nether","end"];
    for (var i=0;i<order.length;i++){ if (dimsById[order[i]]){ currentDim = order[i]; return; } }
    var keys = Object.keys(dimsById);
    currentDim = keys.length ? keys[0] : "overworld";
  }

  function renderDimButtons(){
    var ids = manifest ? (manifest.dimensions||[]).map(function(d){ return d.id; }) : [];
    dimsEl.innerHTML = "";
    var label = { overworld:"主世界", nether:"下界", end:"末地" };
    ids.forEach(function(id){
      var b = document.createElement("button");
      b.className = "mcbtn" + (id === currentDim ? " on" : "");
      b.textContent = label[id] || id;
      b.onclick = function(){
        currentDim = id; didFit = false; userMoved = false;
        renderDimButtons(); fitView(); requestDraw();
      };
      dimsEl.appendChild(b);
    });
  }

  function fitView(){
    var d = dimsById[currentDim];
    if (!d){ return; }
    var wB = (d.maxTx - d.minTx + 1) * tileSize;
    var hB = (d.maxTz - d.minTz + 1) * tileSize;
    if (wB <= 0 || hB <= 0){ didFit = true; return; }
    cam.x = (d.minTx * tileSize + (d.maxTx + 1) * tileSize) / 2;
    cam.z = (d.minTz * tileSize + (d.maxTz + 1) * tileSize) / 2;
    var vw = cw()/dpr, vh = ch()/dpr;
    var s = Math.min(vw / wB, vh / hB) * 0.9;
    if (!isFinite(s) || s <= 0) s = 1;
    cam.scale = Math.min(Math.max(s, 0.02), 16);
    didFit = true;
    requestDraw();
  }

  function applyManifest(m){
    if (!m) return;
    manifest = m;
    tileSize = m.tileSize || 512;
    if (typeof m.pollMs === "number") pollMs = m.pollMs;
    spawn = (m.spawn && m.spawn.length === 3) ? m.spawn : null;
    dimsById = {};
    (m.dimensions||[]).forEach(function(d){ dimsById[d.id] = d; });
    if (m.name) worldnameEl.textContent = m.name;
    genatEl.textContent = m.generated ? ("生成于 " + m.generated) : "";
    var hasData = (m.dimensions||[]).length > 0;
    hintEl.style.display = hasData ? "none" : "flex";
    spawnbtn.disabled = !spawn;
    pickDim();
    renderDimButtons();
    if (!didFit && !userMoved) fitView();
    requestDraw();
  }

  function setPlayers(obj){
    players = (obj && obj.players) || [];
    requestDraw();
  }

  // ── transport: WebSocket (control = JSON text, tiles = binary) ──
  function connectWS(){
    var proto = location.protocol === "https:" ? "wss" : "ws";
    var ws;
    try { ws = new WebSocket(proto + "://" + location.host + "/ws"); }
    catch (e) { scheduleReconnect(); return; }
    ws.binaryType = "arraybuffer";
    ws.onmessage = function(ev){
      if (typeof ev.data === "string"){
        var msg; try { msg = JSON.parse(ev.data); } catch (e) { return; }
        if (msg.type === "map") applyManifest(msg.data);
        else if (msg.type === "players") setPlayers(msg);
      } else {
        handleTile(ev.data);
      }
    };
    ws.onclose = function(){ scheduleReconnect(); };
    ws.onerror = function(){ try { ws.close(); } catch (e) {} };
  }
  var reconnectT = null;
  function scheduleReconnect(){
    if (reconnectT) return;
    reconnectT = setTimeout(function(){ reconnectT = null; connectWS(); }, 1500);
  }

  // ── tiles: decoded from binary WS frames (16-byte header + PNG) ──
  function handleTile(buf){
    if (buf.byteLength < 16) return;
    var dv = new DataView(buf);
    var dim = dv.getInt32(0), z = dv.getInt32(4), tx = dv.getInt32(8), tz = dv.getInt32(12);
    var ck = dimName(dim) + "|" + z + "/" + tx + "_" + tz;
    var blob = new Blob([buf.slice(16)], { type: "image/png" });
    if (window.createImageBitmap){
      createImageBitmap(blob).then(function(bmp){
        var prev = tileCache[ck];
        if (prev && prev.close) prev.close();
        tileCache[ck] = bmp;
        requestDraw();
      }).catch(function(){});
    } else {
      var url = URL.createObjectURL(blob);
      var img = new Image();
      img.onload = function(){ tileCache[ck] = img; requestDraw(); URL.revokeObjectURL(url); };
      img.onerror = function(){ URL.revokeObjectURL(url); };
      img.src = url;
    }
  }

  function getTile(dname, z, tx, tz){
    return tileCache[dname + "|" + z + "/" + tx + "_" + tz] || null;
  }

  function drawBest(dname, z, tx, tz, maxZoom, left, top, right, bot){
    var exact = getTile(dname, z, tx, tz);
    if (exact){
      ctx.drawImage(exact, 0, 0, tileSize, tileSize, left, top, right-left, bot-top);
      return true;
    }
    for (var pz = z+1; pz <= maxZoom; pz++){
      var d2 = pz - z, ax = tx >> d2, az = tz >> d2;
      var anc = getTile(dname, pz, ax, az);
      if (anc){
        var sub = tileSize / Math.pow(2, d2);
        var ox = tx - (ax << d2), oz = tz - (az << d2);
        ctx.drawImage(anc, ox*sub, oz*sub, sub, sub, left, top, right-left, bot-top);
        return true;
      }
    }
    return false;
  }

  function pickZoom(d){
    var maxZ = d.maxZoom || 0, inv = 1 / cam.scale, z = 0;
    while (z < maxZ && Math.pow(2, z+1) <= inv) z++;
    return z;
  }

  function draw(){
    need = false;
    var W = cw(), H = ch();
    ctx.setTransform(1,0,0,1,0,0);
    ctx.clearRect(0,0,W,H);
    var d = dimsById[currentDim];
    if (d){
      var z = pickZoom(d), span = tileSize * Math.pow(2, z);
      var maxZoom = d.maxZoom || 0;
      ctx.imageSmoothingEnabled = cam.scale < 1;
      var tx0 = Math.floor(wxAt(0)/span), tx1 = Math.floor(wxAt(W)/span);
      var tz0 = Math.floor(wzAt(0)/span), tz1 = Math.floor(wzAt(H)/span);
      for (var tx = tx0; tx <= tx1; tx++){
        for (var tz = tz0; tz <= tz1; tz++){
          var left = Math.round(sx(tx*span)), right = Math.round(sx((tx+1)*span));
          var top = Math.round(sz(tz*span)), bot = Math.round(sz((tz+1)*span));
          drawBest(currentDim, z, tx, tz, maxZoom, left, top, right, bot);
        }
      }
      if (showGrid) drawGrid(W, H);
      if (showSpawn && spawn && currentDim === "overworld") drawSpawn();
      drawPlayers();
    }
    updateScaleBar();
  }

  function drawGrid(W, H){
    var stepPx = 16 * cam.scale * dpr, step = 16;
    if (stepPx < 6){ step = 512; stepPx = 512 * cam.scale * dpr; }
    if (stepPx < 6) return;
    ctx.lineWidth = 1;
    ctx.strokeStyle = (step === 512) ? "rgba(255,255,255,0.14)" : "rgba(255,255,255,0.06)";
    var wx1 = wxAt(W), wz1 = wzAt(H);
    var gx0 = Math.floor(wxAt(0)/step)*step;
    for (var x = gx0; x <= wx1; x += step){ var px = Math.round(sx(x))+0.5; ctx.beginPath(); ctx.moveTo(px,0); ctx.lineTo(px,H); ctx.stroke(); }
    var gz0 = Math.floor(wzAt(0)/step)*step;
    for (var zz = gz0; zz <= wz1; zz += step){ var pz = Math.round(sz(zz))+0.5; ctx.beginPath(); ctx.moveTo(0,pz); ctx.lineTo(W,pz); ctx.stroke(); }
  }

  function drawSpawn(){
    var px = sx(spawn[0] + 0.5), pz = sz(spawn[2] + 0.5), r = 7 * dpr;
    ctx.save();
    ctx.translate(px, pz);
    ctx.lineWidth = 2 * dpr;
    ctx.strokeStyle = "rgba(0,0,0,0.85)";
    ctx.fillStyle = "#ff5555";
    ctx.beginPath();
    ctx.moveTo(0,-r); ctx.lineTo(r,0); ctx.lineTo(0,r); ctx.lineTo(-r,0); ctx.closePath();
    ctx.fill(); ctx.stroke();
    ctx.fillStyle = "#e0e0e0";
    ctx.font = (11*dpr) + "px sans-serif";
    ctx.textBaseline = "bottom";
    ctx.fillText("出生点", r + 3*dpr, -r*0.2);
    ctx.restore();
  }

  function drawPlayers(){
    var r = 5 * dpr;
    ctx.textBaseline = "bottom";
    players.forEach(function(p){
      if (dimName(p.dim) !== currentDim) return;
      var px = sx(p.x), pz = sz(p.z);
      if (px < -40 || px > cw()+40 || pz < -40 || pz > ch()+40) return;
      ctx.beginPath(); ctx.arc(px, pz, r, 0, Math.PI*2);
      ctx.fillStyle = "#ffd75e"; ctx.fill();
      ctx.lineWidth = 2*dpr; ctx.strokeStyle = "rgba(0,0,0,0.85)"; ctx.stroke();
      var name = String(p.name);
      ctx.font = (12*dpr) + "px sans-serif";
      var w = ctx.measureText(name).width;
      ctx.fillStyle = "rgba(16,16,16,0.82)";
      ctx.fillRect(px + r + 3*dpr, pz - r - 16*dpr, w + 8*dpr, 15*dpr);
      ctx.fillStyle = "#e0e0e0";
      ctx.fillText(name, px + r + 7*dpr, pz - r - 3*dpr);
    });
  }

  var NICE = [1,2,5,10,20,50,100,200,500,1000,2000,5000];
  function updateScaleBar(){
    zoomtxt.textContent = cam.scale >= 1 ? cam.scale.toFixed(2)+"×" : (cam.scale).toFixed(3)+"×";
    var target = 90, blocks = NICE[0];
    for (var i=0;i<NICE.length;i++){ if (NICE[i]*cam.scale <= target) blocks = NICE[i]; }
    scaleline.style.width = Math.max(8, Math.round(blocks*cam.scale)) + "px";
    scaletxt.textContent = blocks + " 格";
  }

  // ── interaction ──
  canvas.addEventListener("mousedown", function(e){
    var dragging = true; userMoved = true;
    canvas.classList.add("dragging");
    var lx = e.clientX, ly = e.clientY;
    function mv(ev){
      if (!dragging) return;
      cam.x -= (ev.clientX - lx) / cam.scale;
      cam.z -= (ev.clientY - ly) / cam.scale;
      lx = ev.clientX; ly = ev.clientY;
      requestDraw();
    }
    function up(){ dragging = false; canvas.classList.remove("dragging");
      window.removeEventListener("mousemove", mv); window.removeEventListener("mouseup", up); }
    window.addEventListener("mousemove", mv);
    window.addEventListener("mouseup", up);
  });

  canvas.addEventListener("mousemove", function(e){
    var r = canvas.getBoundingClientRect();
    var px = (e.clientX - r.left) * dpr, pz = (e.clientY - r.top) * dpr;
    cxv.textContent = Math.floor(wxAt(px));
    czv.textContent = Math.floor(wzAt(pz));
  });

  function zoomAt(px, pz, factor){
    userMoved = true;
    var wx = wxAt(px), wz = wzAt(pz);
    cam.scale = Math.min(Math.max(cam.scale * factor, 0.01), 32);
    cam.x = wx - (px - cw()/2) / (cam.scale*dpr);
    cam.z = wz - (pz - ch()/2) / (cam.scale*dpr);
    requestDraw();
  }
  canvas.addEventListener("wheel", function(e){
    e.preventDefault();
    var r = canvas.getBoundingClientRect();
    zoomAt((e.clientX - r.left)*dpr, (e.clientY - r.top)*dpr, Math.pow(1.0015, -e.deltaY));
  }, { passive:false });

  $("zin").onclick = function(){ zoomAt(cw()/2, ch()/2, 1.3); };
  $("zout").onclick = function(){ zoomAt(cw()/2, ch()/2, 1/1.3); };
  $("zfit").onclick = function(){ userMoved = false; didFit = false; fitView(); };

  gridbtn.onclick = function(){ showGrid = !showGrid; gridbtn.classList.toggle("on", showGrid); requestDraw(); };
  spawnbtn.onclick = function(){ if (spawnbtn.disabled) return; showSpawn = !showSpawn; spawnbtn.classList.toggle("on", showSpawn); requestDraw(); };

  // ── touch (pan + pinch) ──
  var touchState = null;
  canvas.addEventListener("touchstart", function(e){
    userMoved = true;
    if (e.touches.length === 1){ touchState = { mode:"pan", x:e.touches[0].clientX, y:e.touches[0].clientY }; }
    else if (e.touches.length === 2){
      var dx = e.touches[0].clientX - e.touches[1].clientX, dy = e.touches[0].clientY - e.touches[1].clientY;
      touchState = { mode:"pinch", dist:Math.hypot(dx,dy) };
    }
  }, { passive:false });
  canvas.addEventListener("touchmove", function(e){
    e.preventDefault();
    if (!touchState) return;
    if (touchState.mode === "pan" && e.touches.length === 1){
      cam.x -= (e.touches[0].clientX - touchState.x) / cam.scale;
      cam.z -= (e.touches[0].clientY - touchState.y) / cam.scale;
      touchState.x = e.touches[0].clientX; touchState.y = e.touches[0].clientY;
      requestDraw();
    } else if (touchState.mode === "pinch" && e.touches.length === 2){
      var dx = e.touches[0].clientX - e.touches[1].clientX, dy = e.touches[0].clientY - e.touches[1].clientY;
      var dist = Math.hypot(dx,dy);
      var r = canvas.getBoundingClientRect();
      var mx = ((e.touches[0].clientX + e.touches[1].clientX)/2 - r.left) * dpr;
      var my = ((e.touches[0].clientY + e.touches[1].clientY)/2 - r.top) * dpr;
      zoomAt(mx, my, dist / touchState.dist);
      touchState.dist = dist;
    }
  }, { passive:false });
  canvas.addEventListener("touchend", function(e){ if (e.touches.length === 0) touchState = null; }, { passive:false });

  function frame(){ if (need) draw(); requestAnimationFrame(frame); }

  resize();
  connectWS();
  requestAnimationFrame(frame);
})();
"####;
