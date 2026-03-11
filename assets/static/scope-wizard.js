(function () {
  "use strict";

  var DAILY_RATE = 1400;
  var BASE_DAYS_PER_TARGET = 0.8;
  var MIN_DAYS = 3;

  // Red-team is org-wide — flat base of 15 days + 0.5 per target for extra scope
  var REDTEAM_BASE_DAYS = 15;
  var REDTEAM_PER_TARGET = 0.5;

  var EFFICIENCY = {
    crystal: 1.0,
    grey: 1.3,
    black: 1.8,
    redteam: 2.5,
  };

  var APPROACH_COLORS = {
    crystal: "#22c55e",
    grey: "#3b82f6",
    black: "#a855f7",
    redteam: "#ef4444",
  };

  var APPROACH_ORDER = ["crystal", "grey", "black", "redteam"];

  // Lock state: exactly 2 must be locked at all times
  var locks = { approach: true, scope: true, duration: false };
  var lockHistory = ["approach", "scope"]; // most recent last

  // Current values
  var currentApproach = "crystal";
  var currentScope = 5;
  var currentDuration = 4;

  // DOM refs
  var lockApproach = document.getElementById("lock-approach");
  var lockScope = document.getElementById("lock-scope");
  var lockDuration = document.getElementById("lock-duration");

  var scopeRange = document.getElementById("scope-range");
  var scopeNumber = document.getElementById("scope-number");
  var durationRange = document.getElementById("duration-range");
  var durationNumber = document.getElementById("duration-number");

  var scopeValueDisplay = document.getElementById("scope-value-display");
  var durationValueDisplay = document.getElementById("duration-value-display");
  var priceEl = document.getElementById("scope-price");
  var breakdownEl = document.getElementById("scope-breakdown");

  var controlApproach = document.getElementById("control-approach");
  var controlScope = document.getElementById("control-scope");
  var controlDuration = document.getElementById("control-duration");

  var boxInner = document.getElementById("scope-box-inner");

  var formApproach = document.getElementById("form-approach");
  var formScope = document.getElementById("form-scope");
  var formDuration = document.getElementById("form-duration");
  var formEstimate = document.getElementById("form-estimate");

  var approachRadios = document.querySelectorAll('input[name="approach"]');

  function clamp(val, min, max) {
    return Math.max(min, Math.min(max, val));
  }

  function getVolumeDiscount(days) {
    if (days >= 31) return 0.15;
    if (days >= 16) return 0.1;
    if (days >= 6) return 0.05;
    return 0;
  }

  function formatPrice(cents) {
    var euros = Math.round(cents);
    return (
      "\u20ac" + euros.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ",")
    );
  }

  function getMinDays(approach) {
    return approach === "redteam" ? REDTEAM_BASE_DAYS : MIN_DAYS;
  }

  function calcDuration(approach, scope) {
    var raw;
    if (approach === "redteam") {
      raw = REDTEAM_BASE_DAYS + scope * REDTEAM_PER_TARGET;
    } else {
      raw = scope * BASE_DAYS_PER_TARGET * EFFICIENCY[approach];
    }
    return Math.max(getMinDays(approach), Math.ceil(raw));
  }

  function calcScope(approach, duration) {
    var effectiveDuration = Math.max(duration, getMinDays(approach));
    if (approach === "redteam") {
      return Math.max(1, Math.floor((effectiveDuration - REDTEAM_BASE_DAYS) / REDTEAM_PER_TARGET));
    }
    return Math.max(1, Math.floor(effectiveDuration / (BASE_DAYS_PER_TARGET * EFFICIENCY[approach])));
  }

  function bestFitApproach(scope, duration) {
    // Find the most thorough approach that fits the time budget
    var best = "crystal";
    for (var i = 0; i < APPROACH_ORDER.length; i++) {
      var a = APPROACH_ORDER[i];
      var needed = calcDuration(a, scope);
      if (needed <= duration && duration >= getMinDays(a)) {
        best = a;
      }
    }
    return best;
  }

  function recalculate() {
    var derived = getDerived();

    if (derived === "duration") {
      currentDuration = calcDuration(currentApproach, currentScope);
      currentDuration = clamp(currentDuration, getMinDays(currentApproach), 40);
    } else if (derived === "scope") {
      currentScope = calcScope(currentApproach, currentDuration);
      currentScope = clamp(currentScope, 1, 50);
    } else if (derived === "approach") {
      currentApproach = bestFitApproach(currentScope, currentDuration);
    }

    updateUI();
  }

  function getDerived() {
    if (!locks.approach) return "approach";
    if (!locks.scope) return "scope";
    return "duration";
  }

  function updateUI() {
    // Sync sliders and number inputs
    scopeRange.value = currentScope;
    scopeNumber.value = currentScope;
    durationRange.value = currentDuration;
    durationNumber.value = currentDuration;

    // Sync approach radio
    for (var i = 0; i < approachRadios.length; i++) {
      approachRadios[i].checked = approachRadios[i].value === currentApproach;
    }

    // Value displays
    scopeValueDisplay.textContent =
      currentScope + (currentScope === 1 ? " target" : " targets");
    durationValueDisplay.textContent =
      currentDuration + (currentDuration === 1 ? " man-day" : " man-days");

    // Price calculation
    var discount = getVolumeDiscount(currentDuration);
    var rawPrice = currentDuration * DAILY_RATE;
    var finalPrice = Math.round(rawPrice * (1 - discount));

    priceEl.textContent = formatPrice(finalPrice);

    var breakdownText =
      currentDuration + " man-day" + (currentDuration === 1 ? "" : "s") +
      " \u00d7 \u20ac1,400/day";
    if (discount > 0) {
      breakdownText += " \u2212 " + (discount * 100) + "% volume discount";
    }
    breakdownEl.textContent = breakdownText;

    // Derived styling
    var derived = getDerived();
    controlApproach.classList.toggle("derived", derived === "approach");
    controlScope.classList.toggle("derived", derived === "scope");
    controlDuration.classList.toggle("derived", derived === "duration");

    // Show/hide derived badges
    var badges = document.querySelectorAll(".scope-derived-badge");
    for (var b = 0; b < badges.length; b++) {
      badges[b].style.display = "none";
    }
    var derivedControl = document.getElementById("control-" + derived);
    var derivedBadge = derivedControl
      ? derivedControl.querySelector(".scope-derived-badge")
      : null;
    if (derivedBadge) derivedBadge.style.display = "";

    // Add derived badge to approach control if needed
    if (derived === "approach" && !controlApproach.querySelector(".scope-derived-badge")) {
      var badge = document.createElement("span");
      badge.className = "scope-derived-badge";
      badge.textContent = "auto-calculated";
      controlApproach.appendChild(badge);
    }

    // Disable inputs for derived variable
    for (var j = 0; j < approachRadios.length; j++) {
      approachRadios[j].disabled = derived === "approach";
    }
    scopeRange.disabled = derived === "scope";
    scopeNumber.disabled = derived === "scope";
    durationRange.disabled = derived === "duration";
    durationNumber.disabled = derived === "duration";

    // 3D box visual
    updateBox();

    // Hidden form fields
    formApproach.value = currentApproach;
    formScope.value = currentScope;
    formDuration.value = currentDuration;
    formEstimate.value = finalPrice;
  }

  function updateBox() {
    var color = APPROACH_COLORS[currentApproach];
    var w = 30 + (currentScope / 50) * 90; // 30-120px
    var h = 30 + (currentDuration / 40) * 90; // 30-120px
    var d = 20 + (3.5 - EFFICIENCY[currentApproach]) * 20; // crystal=70px, redteam=40px

    boxInner.style.setProperty("--box-w", w + "px");
    boxInner.style.setProperty("--box-h", h + "px");
    boxInner.style.setProperty("--box-d", d + "px");
    boxInner.style.setProperty("--box-color", color);
  }

  // Lock management
  function handleLockChange(which) {
    var checkbox =
      which === "approach"
        ? lockApproach
        : which === "scope"
        ? lockScope
        : lockDuration;

    if (checkbox.checked) {
      // Locking this one — if already 2 locked, unlock the oldest
      locks[which] = true;
      lockHistory.push(which);

      var lockedCount = 0;
      var lockedKeys = [];
      for (var k in locks) {
        if (locks[k]) {
          lockedCount++;
          lockedKeys.push(k);
        }
      }

      if (lockedCount > 2) {
        // Find the oldest locked that isn't the one we just locked
        for (var i = 0; i < lockHistory.length; i++) {
          var candidate = lockHistory[i];
          if (candidate !== which && locks[candidate]) {
            locks[candidate] = false;
            var cb =
              candidate === "approach"
                ? lockApproach
                : candidate === "scope"
                ? lockScope
                : lockDuration;
            cb.checked = false;
            lockHistory.splice(i, 1);
            break;
          }
        }
      }
    } else {
      // Trying to unlock — ensure at least 2 remain locked
      var remaining = 0;
      for (var k2 in locks) {
        if (k2 !== which && locks[k2]) remaining++;
      }
      if (remaining < 2) {
        checkbox.checked = true;
        return;
      }
      locks[which] = false;
      // Remove from history
      for (var h = lockHistory.length - 1; h >= 0; h--) {
        if (lockHistory[h] === which) {
          lockHistory.splice(h, 1);
          break;
        }
      }
    }

    recalculate();
  }

  // Event listeners
  lockApproach.addEventListener("change", function () {
    handleLockChange("approach");
  });
  lockScope.addEventListener("change", function () {
    handleLockChange("scope");
  });
  lockDuration.addEventListener("change", function () {
    handleLockChange("duration");
  });

  for (var i = 0; i < approachRadios.length; i++) {
    approachRadios[i].addEventListener("change", function () {
      currentApproach = this.value;
      recalculate();
    });
  }

  scopeRange.addEventListener("input", function () {
    currentScope = parseInt(this.value, 10) || 1;
    recalculate();
  });
  scopeNumber.addEventListener("input", function () {
    currentScope = clamp(parseInt(this.value, 10) || 1, 1, 50);
    recalculate();
  });

  durationRange.addEventListener("input", function () {
    currentDuration = parseInt(this.value, 10) || 1;
    recalculate();
  });
  durationNumber.addEventListener("input", function () {
    currentDuration = clamp(parseInt(this.value, 10) || 1, 1, 40);
    recalculate();
  });

  // Initial calculation
  recalculate();
})();
