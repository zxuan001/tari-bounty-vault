// BountyVault demo wiring — DOM only. The state machine lives in sim.js.
(function () {
  "use strict";
  var vault = new BountyVault.BountyVaultSim();
  var role = "sponsor";

  function $(id) { return document.getElementById(id); }
  function shortHash(h) { return h ? "0x" + h.slice(0, 16) + "…" : "—"; }

  function refreshHashes() {
    $("scope-hash").textContent = "0x" + BountyVault.sha256Hex($("scope").value);
    $("report-hash").textContent = "0x" + BountyVault.sha256Hex($("report").value);
  }

  function render() {
    var d = vault.details();
    $("epoch").textContent = d.currentEpoch;
    $("who").textContent = role;
    $("st-state").textContent = d.state || "— no bounty yet —";
    $("st-vault").textContent = d.exists === false && !vault.exists ? "—" : d.vault + " µT";
    $("st-sponsor").textContent = d.sponsor || "—";
    $("st-referee").textContent = d.referee || "—";
    $("st-deadline").textContent = d.deadlineEpoch || "—";
    $("st-scope").textContent = shortHash(d.scopeHash);
    $("st-sub").textContent = d.submission
      ? d.submission.hunter + " · " + shortHash(d.submission.reportHash) + " · epoch " + d.submission.epoch
      : "— none —";
    var ev = $("events");
    ev.innerHTML = "";
    if (!vault.events.length) { ev.innerHTML = "<li>No events yet.</li>"; return; }
    vault.events.forEach(function (e) {
      var li = document.createElement("li");
      li.innerHTML = "<strong>" + e.event + "</strong> — <code>" + e.call + "</code><br>" + e.detail;
      ev.appendChild(li);
    });
  }

  function act(fn, callText) {
    $("act-error").textContent = "";
    $("act-call").textContent = "";
    try {
      fn();
      $("act-call").textContent = "Ootle call: " + callText;
    } catch (e) {
      $("act-error").textContent = "Rejected: " + e.message;
    }
    render();
  }

  // Role switcher
  document.querySelectorAll(".role").forEach(function (b) {
    b.addEventListener("click", function () {
      document.querySelectorAll(".role").forEach(function (x) { x.classList.remove("active"); });
      b.classList.add("active");
      role = b.getAttribute("data-role");
      render();
    });
  });

  // Epoch stepper
  [["epoch-plus", 1], ["epoch-plus10", 10], ["epoch-plus100", 100]].forEach(function (pair) {
    $(pair[0]).addEventListener("click", function () {
      vault.currentEpoch += pair[1];
      render();
    });
  });

  $("scope").addEventListener("input", refreshHashes);
  $("report").addEventListener("input", refreshHashes);

  $("create-btn").addEventListener("click", function () {
    var amount = parseInt($("amount").value, 10);
    var deadline = parseInt($("deadline").value, 10);
    var scopeHash = BountyVault.sha256Hex($("scope").value);
    $("create-call").textContent = "";
    try {
      vault.create(amount, scopeHash, deadline, "referee", "sponsor");
      $("create-call").textContent = "Ootle call: create(amount=" + amount + " µT, scope_hash=0x" + scopeHash.slice(0, 12) + "…, deadline_epoch=" + deadline + ", referee=referee)";
    } catch (e) {
      $("create-call").textContent = "Rejected: " + e.message;
    }
    render();
  });

  $("submit-btn").addEventListener("click", function () {
    var reportHash = BountyVault.sha256Hex($("report").value);
    act(
      function () { vault.submit(reportHash, role === "anyone" ? "hunter" : role, vault.currentEpoch); },
      "submit(report_hash=0x" + reportHash.slice(0, 12) + "…)"
    );
  });
  $("approve-btn").addEventListener("click", function () {
    act(function () { vault.approve(role); }, "approve()");
  });
  $("reject-btn").addEventListener("click", function () {
    act(function () { vault.reject(role); }, "reject()");
  });
  $("dispute-btn").addEventListener("click", function () {
    act(function () { vault.dispute(role); }, "dispute()");
  });
  $("resolve-pay-btn").addEventListener("click", function () {
    act(function () { vault.resolve(role, true); }, "resolve(pay_hunter=true)");
  });
  $("resolve-refund-btn").addEventListener("click", function () {
    act(function () { vault.resolve(role, false); }, "resolve(pay_hunter=false)");
  });
  $("cancel-btn").addEventListener("click", function () {
    act(function () { vault.cancel(role); }, "cancel()");
  });
  $("timeout-btn").addEventListener("click", function () {
    act(function () { vault.timeout(vault.currentEpoch); }, "timeout()");
  });

  // Tabs
  $("tab-sim").addEventListener("click", function () {
    $("tab-sim").classList.add("active");
    $("tab-live").classList.remove("active");
    $("panel-sim").hidden = false;
    $("panel-live").hidden = true;
  });
  $("tab-live").addEventListener("click", function () {
    $("tab-live").classList.add("active");
    $("tab-sim").classList.remove("active");
    $("panel-live").hidden = false;
    $("panel-sim").hidden = true;
  });

  refreshHashes();
  render();
})();
