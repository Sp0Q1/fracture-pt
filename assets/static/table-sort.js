/* table-sort.js - Minimal client-side table sorter (CSP-compliant, no dependencies) */
(function () {
  "use strict";

  function parseValue(text) {
    var s = text.trim();
    if (s === "" || s === "\u2014" || s === "-") return null;
    /* Date: YYYY-MM-DD */
    if (/^\d{4}-\d{2}-\d{2}$/.test(s)) return { type: "date", v: s };
    /* Number (with optional comma thousands, currency symbols) */
    var n = parseFloat(s.replace(/[^0-9.\-]/g, ""));
    if (!isNaN(n) && /\d/.test(s)) return { type: "number", v: n };
    /* String */
    return { type: "string", v: s.toLowerCase() };
  }

  function compare(a, b, asc) {
    var pa = parseValue(a);
    var pb = parseValue(b);
    /* Nulls always sort last */
    if (pa === null && pb === null) return 0;
    if (pa === null) return 1;
    if (pb === null) return -1;
    var result = 0;
    if (pa.type === "number" && pb.type === "number") {
      result = pa.v - pb.v;
    } else {
      var sa = String(pa.v);
      var sb = String(pb.v);
      result = sa < sb ? -1 : sa > sb ? 1 : 0;
    }
    return asc ? result : -result;
  }

  function sortTable(table, colIndex, asc) {
    var tbody = table.tBodies[0];
    if (!tbody) return;
    var rows = Array.prototype.slice.call(tbody.rows);
    /* Separate section-header rows (keep in place) and sortable rows */
    var sortable = [];
    var positions = [];
    for (var i = 0; i < rows.length; i++) {
      if (rows[i].classList.contains("section-header")) continue;
      sortable.push(rows[i]);
      positions.push(i);
    }
    sortable.sort(function (a, b) {
      var aText = a.cells[colIndex] ? a.cells[colIndex].textContent : "";
      var bText = b.cells[colIndex] ? b.cells[colIndex].textContent : "";
      return compare(aText, bText, asc);
    });
    /* Re-insert sorted rows, preserving section headers */
    var sIdx = 0;
    for (var j = 0; j < rows.length; j++) {
      if (rows[j].classList.contains("section-header")) {
        tbody.appendChild(rows[j]);
      } else {
        tbody.appendChild(sortable[sIdx++]);
      }
    }
  }

  function initTable(table) {
    var headers = table.querySelectorAll("thead th");
    if (headers.length === 0) return;
    /* Skip comparison tables (pricing) - they have section-header rows and fixed layout */
    if (table.classList.contains("comparison-table")) return;

    for (var i = 0; i < headers.length; i++) {
      (function (th, idx) {
        th.setAttribute("data-sortable", "");
        th.setAttribute("role", "columnheader");
        th.setAttribute("aria-sort", "none");
        th.setAttribute("tabindex", "0");
        th.style.cursor = "pointer";

        function handleSort() {
          var isAsc = th.classList.contains("asc");
          /* Clear all sort indicators in this table */
          var allTh = table.querySelectorAll("thead th[data-sortable]");
          for (var k = 0; k < allTh.length; k++) {
            allTh[k].classList.remove("asc", "desc");
            allTh[k].setAttribute("aria-sort", "none");
          }
          var newAsc = !isAsc;
          th.classList.add(newAsc ? "asc" : "desc");
          th.setAttribute("aria-sort", newAsc ? "ascending" : "descending");
          sortTable(table, idx, newAsc);
        }

        th.addEventListener("click", handleSort);
        th.addEventListener("keydown", function (e) {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            handleSort();
          }
        });
      })(headers[i], i);
    }
  }

  /* Initialize all tables on DOMContentLoaded */
  function init() {
    var tables = document.querySelectorAll("table");
    for (var i = 0; i < tables.length; i++) {
      if (tables[i].querySelector("thead")) {
        initTable(tables[i]);
      }
    }
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
