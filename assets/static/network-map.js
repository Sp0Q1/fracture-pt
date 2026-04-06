/* Network Topology Map - force-directed SVG graph. Vanilla JS, CSP-compliant. */
(function () {
  "use strict";
  var SEV_COLORS = {extreme:"#dc2626",high:"#f97316",elevated:"#eab308",moderate:"#3b82f6",low:"#22c55e",none:"#475569"};
  var SEV_ORDER = ["extreme","high","elevated","moderate","low"];
  var FPS_INTERVAL = 1000/30, MAX_NODES = 50;
  var DAMPING = 0.85, REPULSION = 1800, ATTRACTION = 0.008, CENTER_PULL = 0.012;

  var container = document.getElementById("network-map");
  if (!container) return;
  var raw = container.getAttribute("data-targets");
  if (!raw) return;
  var targets;
  try { targets = JSON.parse(raw); } catch(e) { return; }
  if (!Array.isArray(targets) || !targets.length) return;

  /* Build node/edge graph */
  var nodes = [], edges = [], nodeMap = {};
  function addNode(id, label, sev, size, pid, data) {
    if (nodeMap[id]) return nodeMap[id];
    var n = {id:id,label:label,severity:sev,r:size,pid:pid,data:data,x:0,y:0,vx:0,vy:0,isDomain:!!pid};
    nodeMap[id] = n; nodes.push(n); return n;
  }
  targets.forEach(function(t) {
    var r = 18 + Math.min((t.findings_total||0)*2, 20);
    var dom = addNode("t:"+t.hostname, t.hostname, t.highest_severity||"none", r, t.pid, t);
    var subs = (t.subdomains_list||[]).slice(0, MAX_NODES);
    subs.forEach(function(sub) {
      var sn = addNode("s:"+sub.name, sub.name, sub.resolved?"low":"none", 6+(sub.resolved?3:0), null, sub);
      edges.push({source:dom, target:sn});
    });
  });
  if (!nodes.length) return;

  /* Layout init */
  var W = container.clientWidth||600, H = Math.max(400, Math.min(W*0.55, 600));
  var cx = W/2, cy = H/2;
  nodes.forEach(function(n) {
    var spread = n.isDomain ? 60 : W*0.7;
    n.x = cx + (Math.random()-0.5)*spread;
    n.y = cy + (Math.random()-0.5)*(n.isDomain?60:H*0.7);
  });

  /* Create SVG */
  var NS = "http://www.w3.org/2000/svg";
  var svg = document.createElementNS(NS,"svg");
  svg.setAttribute("viewBox","0 0 "+W+" "+H);
  svg.setAttribute("preserveAspectRatio","xMidYMid meet");
  container.appendChild(svg);
  var lineGroup = document.createElementNS(NS,"g");
  svg.appendChild(lineGroup);
  var edgeEls = edges.map(function() {
    var line = document.createElementNS(NS,"line");
    line.setAttribute("stroke","rgba(255,255,255,.12)");
    line.setAttribute("stroke-width","1");
    lineGroup.appendChild(line);
    return line;
  });
  var nodeGroup = document.createElementNS(NS,"g");
  svg.appendChild(nodeGroup);
  var circleEls = [];
  nodes.forEach(function(n) {
    var g = document.createElementNS(NS,"g");
    g.setAttribute("class","nm-node");
    var c = document.createElementNS(NS,"circle");
    var col = SEV_COLORS[n.severity]||SEV_COLORS.none;
    c.setAttribute("r",n.r); c.setAttribute("fill",col);
    c.setAttribute("fill-opacity",n.isDomain?"0.85":"0.55");
    c.setAttribute("stroke",col); c.setAttribute("stroke-width",n.isDomain?"2.5":"1");
    c.setAttribute("stroke-opacity","0.6"); g.appendChild(c);
    if (n.isDomain) {
      var text = document.createElementNS(NS,"text");
      text.setAttribute("text-anchor","middle"); text.setAttribute("dy",n.r+14);
      text.setAttribute("fill","#e2e8f0"); text.setAttribute("font-size","11");
      text.setAttribute("font-weight","600");
      text.textContent = n.label.length>24 ? n.label.slice(0,22)+".." : n.label;
      g.appendChild(text);
    }
    if (n.pid) { g.setAttribute("data-href","/targets/"+n.pid); c.setAttribute("class","nm-clickable"); }
    nodeGroup.appendChild(g);
    circleEls.push({g:g,circle:c,node:n});
  });

  /* Tooltip */
  var tooltip = document.createElement("div");
  tooltip.className = "nm-tooltip"; tooltip.setAttribute("hidden","");
  container.appendChild(tooltip);
  function addLine(parent, text, cls) {
    var el = document.createElement("div");
    if (cls) el.className = cls;
    el.textContent = text;
    parent.appendChild(el);
  }
  function showTooltip(n, evt) {
    while (tooltip.firstChild) tooltip.removeChild(tooltip.firstChild);
    var title = document.createElement("strong");
    title.textContent = n.label;
    tooltip.appendChild(title);
    if (n.data && n.isDomain) {
      addLine(tooltip, "Subdomains: "+(n.data.subdomains||0));
      addLine(tooltip, "Open ports: "+(n.data.open_ports||0));
      if (n.data.findings) {
        var f=n.data.findings;
        SEV_ORDER.forEach(function(s) {
          if(f[s]>0) addLine(tooltip, s+": "+f[s], "nm-sev-"+s);
        });
      }
    } else if (n.data) { addLine(tooltip, n.data.resolved?"Resolved":"Unresolved"); }
    tooltip.removeAttribute("hidden");
    var rect = container.getBoundingClientRect();
    var x = evt.clientX-rect.left+12, y = evt.clientY-rect.top-10;
    if (x+280>rect.width) x = evt.clientX-rect.left-290;
    if (y<0) y = 4;
    tooltip.style.left = x+"px"; tooltip.style.top = y+"px";
  }
  function hideTooltip() { tooltip.setAttribute("hidden",""); }

  /* SVG coordinate helpers */
  function svgPoint(evt) {
    var rect = svg.getBoundingClientRect();
    return {x:(evt.clientX-rect.left)/rect.width*W, y:(evt.clientY-rect.top)/rect.height*H};
  }
  function hitTest(evt) {
    var pt = svgPoint(evt);
    for (var i=nodes.length-1; i>=0; i--) {
      var dx=pt.x-nodes[i].x, dy=pt.y-nodes[i].y;
      if (dx*dx+dy*dy < (nodes[i].r+4)*(nodes[i].r+4)) return i;
    }
    return -1;
  }

  /* Interaction: hover, click, drag */
  var hoveredIdx = -1, dragIdx = -1, dragOffX = 0, dragOffY = 0;
  function resetHover() {
    if (hoveredIdx<0) return;
    var n = nodes[hoveredIdx];
    circleEls[hoveredIdx].circle.setAttribute("stroke-opacity","0.6");
    circleEls[hoveredIdx].circle.setAttribute("stroke-width",n.isDomain?"2.5":"1");
    hoveredIdx = -1;
  }
  svg.addEventListener("mousemove", function(evt) {
    if (dragIdx>=0) {
      var pt=svgPoint(evt); nodes[dragIdx].x=pt.x-dragOffX; nodes[dragIdx].y=pt.y-dragOffY;
      nodes[dragIdx].vx=0; nodes[dragIdx].vy=0; return;
    }
    var hit = hitTest(evt);
    if (hit>=0) {
      if (hoveredIdx!==hit) { resetHover(); hoveredIdx=hit;
        circleEls[hit].circle.setAttribute("stroke-opacity","1");
        circleEls[hit].circle.setAttribute("stroke-width","3");
        svg.style.cursor = nodes[hit].pid?"pointer":"default";
      }
      showTooltip(nodes[hit], evt);
    } else { resetHover(); svg.style.cursor="default"; hideTooltip(); }
  });
  svg.addEventListener("mousedown", function(evt) {
    var hit=hitTest(evt);
    if (hit>=0) { evt.preventDefault(); dragIdx=hit; var pt=svgPoint(evt); dragOffX=pt.x-nodes[hit].x; dragOffY=pt.y-nodes[hit].y; }
  });
  document.addEventListener("mouseup", function() { dragIdx=-1; });
  svg.addEventListener("click", function(evt) {
    var hit=hitTest(evt); if (hit>=0 && nodes[hit].pid) window.location.href="/targets/"+nodes[hit].pid;
  });
  svg.addEventListener("mouseleave", function() { hideTooltip(); resetHover(); });

  /* Force simulation */
  function tick() {
    var i,j,n,m,dx,dy,dist,force;
    for (i=0; i<nodes.length; i++) { n=nodes[i];
      for (j=i+1; j<nodes.length; j++) { m=nodes[j];
        dx=n.x-m.x; dy=n.y-m.y; dist=Math.sqrt(dx*dx+dy*dy)||1;
        force=REPULSION/(dist*dist); var fx=dx/dist*force, fy=dy/dist*force;
        n.vx+=fx; n.vy+=fy; m.vx-=fx; m.vy-=fy;
      }
    }
    for (i=0; i<edges.length; i++) {
      var s=edges[i].source, t=edges[i].target;
      dx=t.x-s.x; dy=t.y-s.y; dist=Math.sqrt(dx*dx+dy*dy)||1;
      force=(dist-80)*ATTRACTION;
      s.vx+=dx/dist*force; s.vy+=dy/dist*force;
      t.vx-=dx/dist*force; t.vy-=dy/dist*force;
    }
    for (i=0; i<nodes.length; i++) { n=nodes[i];
      n.vx+=(cx-n.x)*CENTER_PULL; n.vy+=(cy-n.y)*CENTER_PULL;
    }
    for (i=0; i<nodes.length; i++) { n=nodes[i];
      if (i===dragIdx) continue;
      n.vx*=DAMPING; n.vy*=DAMPING; n.x+=n.vx; n.y+=n.vy;
      n.x=Math.max(n.r,Math.min(W-n.r,n.x)); n.y=Math.max(n.r,Math.min(H-n.r,n.y));
    }
  }

  /* Render loop */
  var lastFrame=0, settled=0;
  function render(ts) {
    if (ts-lastFrame>=FPS_INTERVAL) { lastFrame=ts; tick();
      for (var i=0; i<edges.length; i++) {
        edgeEls[i].setAttribute("x1",edges[i].source.x); edgeEls[i].setAttribute("y1",edges[i].source.y);
        edgeEls[i].setAttribute("x2",edges[i].target.x); edgeEls[i].setAttribute("y2",edges[i].target.y);
      }
      for (var j=0; j<nodes.length; j++)
        circleEls[j].g.setAttribute("transform","translate("+nodes[j].x+","+nodes[j].y+")");
      var energy=0;
      for (var k=0; k<nodes.length; k++) energy+=nodes[k].vx*nodes[k].vx+nodes[k].vy*nodes[k].vy;
      if (energy<0.01 && dragIdx<0) { settled++; if (settled>120) return; } else settled=0;
    }
    requestAnimationFrame(render);
  }
  requestAnimationFrame(render);

  /* Resize handler */
  var resizeTimer;
  window.addEventListener("resize", function() {
    clearTimeout(resizeTimer);
    resizeTimer = setTimeout(function() {
      W=container.clientWidth||600; H=Math.max(400,Math.min(W*0.55,600));
      cx=W/2; cy=H/2; svg.setAttribute("viewBox","0 0 "+W+" "+H);
      settled=0; requestAnimationFrame(render);
    }, 200);
  });
})();
