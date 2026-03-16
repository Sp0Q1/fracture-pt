(function () {
  "use strict";

  var DAILY_RATE = 1400;
  var MIN_DAYS = 3;

  // Red-team is org-wide — flat base of 15 days + per-complexity scaling
  var REDTEAM_BASE_DAYS = 15;

  // Complexity levels map to internal effort multipliers (not shown to user)
  var COMPLEXITY = {
    1: { label: "Minimal", targets: 2, redteamExtra: 1 },
    2: { label: "Small", targets: 5, redteamExtra: 3 },
    3: { label: "Medium", targets: 10, redteamExtra: 5 },
    4: { label: "Large", targets: 20, redteamExtra: 8 },
    5: { label: "Enterprise", targets: 40, redteamExtra: 12 },
  };

  var BASE_DAYS_PER_TARGET = 0.8;

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

  // Duration is locked by default — auto-calculated from approach + complexity.
  var durationLocked = true;

  // Current values
  var currentApproach = "crystal";
  var currentComplexity = 3;
  var currentDuration = 8;

  // DOM refs
  var lockDuration = document.getElementById("lock-duration");
  var lockDurationText = document.getElementById("lock-duration-text");

  var scopeRange = document.getElementById("scope-range");
  var durationRange = document.getElementById("duration-range");
  var durationNumber = document.getElementById("duration-number");

  var scopeValueDisplay = document.getElementById("scope-value-display");
  var durationValueDisplay = document.getElementById("duration-value-display");
  var priceEl = document.getElementById("scope-price");
  var breakdownEl = document.getElementById("scope-breakdown");

  var controlDuration = document.getElementById("control-duration");

  var boxInner = document.getElementById("scope-box-inner");

  var formApproach = document.getElementById("form-approach");
  var formScope = document.getElementById("form-scope");
  var formDuration = document.getElementById("form-duration");
  var formEstimate = document.getElementById("form-estimate");

  var approachRadios = document.querySelectorAll('input[name="approach"]');
  var approachHint = document.getElementById("approach-hint");
  var durationWarning = document.getElementById("duration-warning");

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

  function calcDuration(approach, complexity) {
    var level = COMPLEXITY[complexity];
    var raw;
    if (approach === "redteam") {
      raw = REDTEAM_BASE_DAYS + level.redteamExtra;
    } else {
      raw = level.targets * BASE_DAYS_PER_TARGET * EFFICIENCY[approach];
    }
    return Math.max(getMinDays(approach), Math.ceil(raw));
  }

  function getCoverageRatio(approach, complexity, duration) {
    var idealDays = calcDuration(approach, complexity);
    return duration / idealDays;
  }

  function getApproachHint(approach, complexity, duration) {
    if (approach === "redteam") return "";

    var ratio = getCoverageRatio(approach, complexity, duration);
    var label = COMPLEXITY[complexity].label.toLowerCase();

    if (approach === "crystal" && ratio < 0.7) {
      return "With " + duration + " day" + (duration === 1 ? "" : "s") +
        " for a " + label + " environment, crystal-box may not achieve full coverage. Consider reducing complexity or adding more days \u2014 or switch to grey-box for faster reconnaissance.";
    }
    if (approach === "crystal" && ratio < 0.9) {
      return "Tight schedule for crystal-box \u2014 coverage will be good but not exhaustive. Adding a few more days would allow deeper analysis.";
    }
    if (approach === "black" && duration >= calcDuration("crystal", complexity)) {
      return "You have enough days for a crystal-box assessment, which would provide significantly deeper coverage at the same cost.";
    }
    if (approach === "grey" && duration >= calcDuration("crystal", complexity) * 1.2) {
      return "With this many days, crystal-box would provide the most thorough results \u2014 worth considering if you can share source access.";
    }
    return "";
  }

  function recalculate() {
    if (durationLocked) {
      currentDuration = calcDuration(currentApproach, currentComplexity);
      currentDuration = clamp(currentDuration, getMinDays(currentApproach), 40);
    }

    updateUI();
  }

  function updateUI() {
    // Sync slider
    scopeRange.value = currentComplexity;
    durationRange.value = currentDuration;
    durationNumber.value = currentDuration;

    // Sync approach radio
    for (var i = 0; i < approachRadios.length; i++) {
      approachRadios[i].checked = approachRadios[i].value === currentApproach;
    }

    // Value displays
    scopeValueDisplay.textContent = COMPLEXITY[currentComplexity].label;
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

    // Duration lock state
    controlDuration.classList.toggle("derived", durationLocked);
    durationRange.disabled = durationLocked;
    durationNumber.disabled = durationLocked;
    lockDurationText.textContent = durationLocked ? "Locked" : "Unlocked";

    // Approach inputs are always enabled
    for (var j = 0; j < approachRadios.length; j++) {
      approachRadios[j].disabled = false;
    }

    // Approach coverage hint — only when duration is manually set.
    // When locked, the system auto-calculates the right duration for the
    // chosen approach, so suggesting a different approach is circular.
    var hint = durationLocked ? "" : getApproachHint(currentApproach, currentComplexity, currentDuration);
    approachHint.textContent = hint;
    approachHint.classList.toggle("visible", !!hint);

    // Duration unlock warning
    if (!durationLocked) {
      var idealDuration = calcDuration(currentApproach, currentComplexity);
      if (currentDuration < idealDuration) {
        durationWarning.textContent =
          "The recommended duration for this configuration is " + idealDuration +
          " man-days. Reducing below this may limit the depth and completeness of the assessment.";
      } else if (idealDuration > 40) {
        durationWarning.textContent =
          "This combination of approach and complexity exceeds 40 man-days. " +
          "Please reduce complexity or select a more efficient approach for an accurate estimate.";
      } else {
        durationWarning.textContent =
          "Duration is manually set. The estimated duration for this configuration is " +
          idealDuration + " man-days.";
      }
      durationWarning.classList.add("visible");
    } else {
      durationWarning.textContent = "";
      durationWarning.classList.remove("visible");
    }

    // 3D box visual
    updateBox();

    // Hidden form fields
    formApproach.value = currentApproach;
    formScope.value = COMPLEXITY[currentComplexity].label;
    formDuration.value = currentDuration;
    formEstimate.value = finalPrice;
  }

  function updateBox() {
    var color = APPROACH_COLORS[currentApproach];
    var w = 30 + (currentComplexity / 5) * 90;
    var h = 30 + (currentDuration / 40) * 90;
    var d = 20 + (3.5 - EFFICIENCY[currentApproach]) * 20;

    boxInner.style.setProperty("--box-w", w + "px");
    boxInner.style.setProperty("--box-h", h + "px");
    boxInner.style.setProperty("--box-d", d + "px");
    boxInner.style.setProperty("--box-color", color);
  }

  // Duration lock toggle
  lockDuration.addEventListener("change", function () {
    durationLocked = this.checked;
    if (durationLocked) {
      currentDuration = calcDuration(currentApproach, currentComplexity);
      currentDuration = clamp(currentDuration, getMinDays(currentApproach), 40);
    }
    recalculate();
  });

  for (var i = 0; i < approachRadios.length; i++) {
    approachRadios[i].addEventListener("change", function () {
      currentApproach = this.value;
      recalculate();
    });
  }

  scopeRange.addEventListener("input", function () {
    currentComplexity = parseInt(this.value, 10) || 1;
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
