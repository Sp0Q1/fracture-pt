(function () {
  "use strict";

  var MAX_SIZE = 5 * 1024 * 1024;
  var ALLOWED = ["image/png","image/jpeg","image/gif","image/webp","image/svg+xml"];

  function init() {
    var areas = document.querySelectorAll("textarea[data-md-editor]");
    for (var i = 0; i < areas.length; i++) setup(areas[i]);
  }

  function setup(ta) {
    var wrap = document.createElement("div");
    wrap.className = "md-editor-wrap";
    ta.parentNode.insertBefore(wrap, ta);
    wrap.appendChild(buildToolbar(ta));
    wrap.appendChild(ta);
    var preview = document.createElement("div");
    preview.className = "md-preview";
    wrap.appendChild(preview);

    var timer = null;
    ta.addEventListener("input", function () {
      clearTimeout(timer);
      // nosemgrep: javascript.browser.security.insecure-document-method.insecure-document-method
      timer = setTimeout(function () { preview.innerHTML = renderMd(ta.value); }, 300);
    });
    // nosemgrep: javascript.browser.security.insecure-document-method.insecure-document-method
    if (ta.value) preview.innerHTML = renderMd(ta.value);

    ta.addEventListener("dragover", function (e) { e.preventDefault(); });
    ta.addEventListener("drop", function (e) {
      e.preventDefault();
      var files = e.dataTransfer && e.dataTransfer.files;
      if (files) handleFiles(ta, wrap, files);
    });
    ta.addEventListener("paste", function (e) {
      var items = e.clipboardData && e.clipboardData.items;
      if (!items) return;
      for (var i = 0; i < items.length; i++) {
        if (items[i].kind === "file") {
          var f = items[i].getAsFile();
          if (f) { e.preventDefault(); handleFiles(ta, wrap, [f]); return; }
        }
      }
    });
  }

  function buildToolbar(ta) {
    var bar = document.createElement("div");
    bar.className = "md-toolbar";
    var btns = [
      ["B", "Bold", function () { wrapSel(ta, "**", "**"); }],
      ["I", "Italic", function () { wrapSel(ta, "*", "*"); }],
      ["H", "Heading", function () { prefixLines(ta, "### "); }],
      ["\uD83D\uDD17", "Link", function () { insertLink(ta); }],
      ["</>", "Code", function () { insertCode(ta); }],
      ["\uD83D\uDCCE", "Image", function () { pickImage(ta, bar); }],
      ["\u2630", "List", function () { prefixLines(ta, "- "); }],
      ["\u275D", "Quote", function () { prefixLines(ta, "> "); }]
    ];
    for (var i = 0; i < btns.length; i++) {
      var b = document.createElement("button");
      b.type = "button";
      b.textContent = btns[i][0];
      b.title = btns[i][1];
      b.addEventListener("click", btns[i][2]);
      bar.appendChild(b);
    }
    return bar;
  }

  function getSel(ta) {
    return { s: ta.selectionStart, e: ta.selectionEnd, t: ta.value.substring(ta.selectionStart, ta.selectionEnd) };
  }

  function replaceRange(ta, s, e, text) {
    ta.value = ta.value.substring(0, s) + text + ta.value.substring(e);
    ta.selectionStart = ta.selectionEnd = s + text.length;
    ta.focus();
    ta.dispatchEvent(new Event("input", { bubbles: true }));
  }

  function wrapSel(ta, before, after) {
    var sel = getSel(ta);
    var text = sel.t || "text";
    replaceRange(ta, sel.s, sel.e, before + text + after);
    ta.selectionStart = sel.s + before.length;
    ta.selectionEnd = sel.s + before.length + text.length;
  }

  function prefixLines(ta, prefix) {
    var sel = getSel(ta);
    var start = ta.value.lastIndexOf("\n", sel.s - 1) + 1;
    var end = ta.value.indexOf("\n", sel.e);
    if (end === -1) end = ta.value.length;
    var block = ta.value.substring(start, end);
    var lines = block.split("\n").map(function (l) { return prefix + l; }).join("\n");
    replaceRange(ta, start, end, lines);
  }

  function insertLink(ta) {
    var sel = getSel(ta);
    var text = sel.t || "link text";
    var url = prompt("Enter URL:");
    if (url === null) return;
    replaceRange(ta, sel.s, sel.e, "[" + text + "](" + url + ")");
  }

  function insertCode(ta) {
    var sel = getSel(ta);
    if (sel.t.indexOf("\n") !== -1) {
      replaceRange(ta, sel.s, sel.e, "```\n" + sel.t + "\n```");
    } else {
      var text = sel.t || "code";
      wrapSel(ta, "`", "`");
    }
  }

  function pickImage(ta, bar) {
    var input = document.createElement("input");
    input.type = "file";
    input.accept = "image/*";
    input.addEventListener("change", function () {
      if (input.files && input.files.length) handleFiles(ta, bar.parentNode, [input.files[0]]);
    });
    input.click();
  }

  function handleFiles(ta, wrap, files) {
    for (var i = 0; i < files.length; i++) uploadFile(ta, wrap, files[i]);
  }

  function uploadFile(ta, wrap, file) {
    if (ALLOWED.indexOf(file.type) === -1) { showStatus(wrap, "Error: unsupported type " + file.type); return; }
    if (file.size > MAX_SIZE) { showStatus(wrap, "Error: file too large (max 5 MB)"); return; }
    showStatus(wrap, "Uploading\u2026");
    var fd = new FormData();
    fd.append("file", file);
    fetch("/api/uploads", { method: "POST", body: fd, credentials: "same-origin" })
      .then(function (r) {
        if (!r.ok) return r.text().then(function (t) { throw new Error(t || "HTTP " + r.status); });
        return r.json();
      })
      .then(function (data) {
        showStatus(wrap, "");
        var md = "![image](/api/uploads/" + data.pid + ")\n";
        var pos = ta.selectionStart;
        replaceRange(ta, pos, pos, md);
      })
      .catch(function (err) { showStatus(wrap, "Upload failed: " + err.message); });
  }

  function showStatus(wrap, msg) {
    var el = wrap.querySelector(".md-upload-status");
    if (!el) {
      el = document.createElement("span");
      el.className = "md-upload-status";
      el.setAttribute("role", "status");
      el.setAttribute("aria-live", "polite");
      var bar = wrap.querySelector(".md-toolbar");
      if (bar) bar.appendChild(el);
    }
    el.textContent = msg;
  }

  /* --- Minimal Markdown Preview Renderer --- */
  function esc(s) { return s.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;").replace(/"/g,"&quot;"); }

  function renderMd(src) {
    var lines = src.split("\n");
    var html = [], inCode = false, inList = false, inQuote = false, buf = [];

    function flushQuote() { if (inQuote) { html.push("<blockquote>" + buf.join("<br>") + "</blockquote>"); buf = []; inQuote = false; } }
    function flushList() { if (inList) { html.push("<ul>" + buf.join("") + "</ul>"); buf = []; inList = false; } }
    function flushBlock() { flushQuote(); flushList(); }

    for (var i = 0; i < lines.length; i++) {
      var line = lines[i];
      if (inCode) {
        if (line.match(/^```/)) { html.push("<pre><code>" + esc(buf.join("\n")) + "</code></pre>"); buf = []; inCode = false; }
        else buf.push(line);
        continue;
      }
      if (line.match(/^```/)) { flushBlock(); inCode = true; buf = []; continue; }

      var m;
      if ((m = line.match(/^(#{1,6})\s+(.*)/))) { flushBlock(); html.push("<h" + m[1].length + ">" + inline(esc(m[2])) + "</h" + m[1].length + ">"); continue; }
      if ((m = line.match(/^>\s?(.*)/))) { flushList(); if (!inQuote) { inQuote = true; buf = []; } buf.push(inline(esc(m[1]))); continue; }
      if ((m = line.match(/^[-*]\s+(.*)/))) { flushQuote(); if (!inList) { inList = true; buf = []; } buf.push("<li>" + inline(esc(m[1])) + "</li>"); continue; }

      flushBlock();
      if (line.trim() === "") { html.push(""); }
      else { html.push("<p>" + inline(esc(line)) + "</p>"); }
    }
    if (inCode) { html.push("<pre><code>" + esc(buf.join("\n")) + "</code></pre>"); }
    flushBlock();
    return html.join("\n");
  }

  function inline(s) {
    s = s.replace(/!\[([^\]]*)\]\(([^)]+)\)/g, '<img src="$2" alt="$1" style="max-width:100%">');
    s = s.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2">$1</a>');
    s = s.replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>");
    s = s.replace(/\*(.+?)\*/g, "<em>$1</em>");
    s = s.replace(/`([^`]+)`/g, "<code>$1</code>");
    return s;
  }

  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", init);
  else init();
})();
