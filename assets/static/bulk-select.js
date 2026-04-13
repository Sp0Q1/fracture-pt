/* bulk-select.js - Select-all checkbox + enable/disable submit button (CSP-compliant) */
(function () {
  "use strict";

  function init() {
    var selectAll = document.getElementById("select-all");
    var btn = document.getElementById("bulk-delete-btn");
    if (!selectAll || !btn) return;

    var boxes = document.querySelectorAll(".finding-checkbox");

    function updateBtn() {
      var any = false;
      for (var i = 0; i < boxes.length; i++) {
        if (boxes[i].checked) { any = true; break; }
      }
      btn.disabled = !any;
    }

    selectAll.addEventListener("change", function () {
      for (var i = 0; i < boxes.length; i++) {
        boxes[i].checked = selectAll.checked;
      }
      updateBtn();
    });

    for (var i = 0; i < boxes.length; i++) {
      boxes[i].addEventListener("change", updateBtn);
    }

    /* Prevent row-click navigation on .no-navigate cells */
    var cells = document.querySelectorAll(".no-navigate");
    for (var j = 0; j < cells.length; j++) {
      cells[j].addEventListener("click", function (e) {
        e.stopPropagation();
      });
    }
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
