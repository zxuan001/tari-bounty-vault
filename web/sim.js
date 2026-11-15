// BountyVault demo simulation — pure state machine, no DOM.
// Mirrors the on-chain template's state machine exactly:
//   Open -> Submitted -> Paid        (approve)
//   Open -> Submitted -> Open        (reject, submission cleared)
//   Open -> Submitted -> Disputed -> Paid | Refunded   (dispute, resolve)
//   Open -> Refunded                (cancel by sponsor, or timeout after deadline)
// Payouts always move the WHOLE vault: no partial amounts, no dust left behind.
// Only hashes go on chain: scope_hash and report_hash. Scope text and report text
// stay off-chain (shown here only so the demo can hash them for you).

(function (root) {
  "use strict";

  var STATES = ["Open", "Submitted", "Disputed", "Paid", "Refunded"];

  // --- Minimal SHA-256 (FIPS 180-4), sync, no dependencies -----------------
  // Standard construction; written here so the demo works from file:// too.
  var K = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
  ];
  function rotr(x, n) { return (x >>> n) | (x << (32 - n)); }
  function sha256Hex(ascii) {
    var bytes = [];
    for (var i = 0; i < ascii.length; i++) {
      var c = ascii.charCodeAt(i);
      if (c < 0x80) bytes.push(c);
      else if (c < 0x800) { bytes.push(0xc0 | (c >> 6), 0x80 | (c & 0x3f)); }
      else if (c < 0x10000) { bytes.push(0xe0 | (c >> 12), 0x80 | ((c >> 6) & 0x3f), 0x80 | (c & 0x3f)); }
      else { bytes.push(0xf0 | (c >> 18), 0x80 | ((c >> 12) & 0x3f), 0x80 | ((c >> 6) & 0x3f), 0x80 | (c & 0x3f)); }
    }
    var bitLen = bytes.length * 8;
    bytes.push(0x80);
    while (bytes.length % 64 !== 56) bytes.push(0);
    for (var b = 7; b >= 0; b--) bytes.push((bitLen / Math.pow(2, b * 8)) & 0xff);
    var h = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    for (var off = 0; off < bytes.length; off += 64) {
      var w = new Array(64);
      for (var t = 0; t < 16; t++) {
        w[t] = (bytes[off + t * 4] << 24) | (bytes[off + t * 4 + 1] << 16) | (bytes[off + t * 4 + 2] << 8) | bytes[off + t * 4 + 3];
      }
      for (var t2 = 16; t2 < 64; t2++) {
        var s0 = rotr(w[t2 - 15], 7) ^ rotr(w[t2 - 15], 18) ^ (w[t2 - 15] >>> 3);
        var s1 = rotr(w[t2 - 2], 17) ^ rotr(w[t2 - 2], 19) ^ (w[t2 - 2] >>> 10);
        w[t2] = (w[t2 - 16] + s0 + w[t2 - 7] + s1) | 0;
      }
      var a = h[0], bb = h[1], c2 = h[2], d = h[3], e = h[4], f = h[5], g = h[6], hh = h[7];
      for (var t3 = 0; t3 < 64; t3++) {
        var S1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25);
        var ch = (e & f) ^ (~e & g);
        var temp1 = (hh + S1 + ch + K[t3] + w[t3]) | 0;
        var S0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22);
        var maj = (a & bb) ^ (a & c2) ^ (bb & c2);
        var temp2 = (S0 + maj) | 0;
        hh = g; g = f; f = e; e = (d + temp1) | 0; d = c2; c2 = bb; bb = a; a = (temp1 + temp2) | 0;
      }
      h[0] = (h[0] + a) | 0; h[1] = (h[1] + bb) | 0; h[2] = (h[2] + c2) | 0; h[3] = (h[3] + d) | 0;
      h[4] = (h[4] + e) | 0; h[5] = (h[5] + f) | 0; h[6] = (h[6] + g) | 0; h[7] = (h[7] + hh) | 0;
    }
    var out = "";
    for (var i2 = 0; i2 < 8; i2++) out += ("00000000" + (h[i2] >>> 0).toString(16)).slice(-8);
    return out;
  }

  // --- State machine -------------------------------------------------------
  function BountyVaultSim() {
    this.reset();
  }

  BountyVaultSim.prototype.reset = function () {
    this.exists = false;
    this.state = null;
    this.amount = 0;
    this.vault = 0;
    this.scopeHash = null;
    this.deadlineEpoch = 0;
    this.sponsor = null;
    this.referee = null;
    this.submission = null; // { hunter, reportHash, epoch } | null
    this.events = [];
    this.currentEpoch = 1000; // simulated chain epoch; the demo can advance it
  };

  BountyVaultSim.prototype.log = function (event, ootleCall, detail) {
    this.events.push({ event: event, call: ootleCall, detail: detail || "" });
  };

  BountyVaultSim.prototype.fail = function (msg) {
    throw new Error(msg);
  };

  // create(amount, scopeHash, deadlineEpoch, referee, sponsor)
  BountyVaultSim.prototype.create = function (amount, scopeHash, deadlineEpoch, referee, sponsor) {
    if (this.exists) this.fail("a bounty already exists in this demo (reload to start over)");
    if (!(amount > 0)) this.fail("amount must be more than zero");
    if (deadlineEpoch <= this.currentEpoch) this.fail("deadline must be in the future");
    if (!referee) this.fail("a referee is required");
    if (referee === sponsor) this.fail("referee must differ from the sponsor");
    this.exists = true;
    this.state = "Open";
    this.amount = amount;
    this.vault = amount;
    this.scopeHash = scopeHash;
    this.deadlineEpoch = deadlineEpoch;
    this.sponsor = sponsor;
    this.referee = referee;
    this.submission = null;
    this.log("Created", "create(amount=" + amount + " µT, scope_hash=0x" + scopeHash.slice(0, 12) + "…, deadline_epoch=" + deadlineEpoch + ", referee=" + referee + ")", "vault locked: " + amount + " µT");
    return true;
  };

  // submit(reportHash, hunter, epoch)
  BountyVaultSim.prototype.submit = function (reportHash, hunter, epoch) {
    if (!this.exists) this.fail("no bounty yet — create one first");
    if (this.state !== "Open") this.fail("submit needs state Open (now " + this.state + ")");
    if (epoch >= this.deadlineEpoch) this.fail("deadline has passed — submissions are closed");
    if (this.submission) this.fail("a submission is already waiting for review");
    this.submission = { hunter: hunter, reportHash: reportHash, epoch: epoch };
    this.state = "Submitted";
    this.log("Submitted", "submit(report_hash=0x" + reportHash.slice(0, 12) + "…)", "hunter: " + hunter);
    return true;
  };

  // approve(caller) — sponsor only, pays the whole vault to the hunter
  BountyVaultSim.prototype.approve = function (caller) {
    if (this.state !== "Submitted") this.fail("approve needs state Submitted (now " + this.state + ")");
    if (caller !== this.sponsor) this.fail("only the sponsor can approve");
    if (!this.submission) this.fail("no submission to approve");
    var paid = this.vault;
    var hunter = this.submission.hunter;
    this.vault = 0;
    this.state = "Paid";
    this.log("Approved", "approve()", "paid " + paid + " µT to " + hunter + " — vault now empty");
    return { paid: paid, to: hunter };
  };

  // reject(caller) — sponsor only, clears the submission, reopens
  BountyVaultSim.prototype.reject = function (caller) {
    if (this.state !== "Submitted") this.fail("reject needs state Submitted (now " + this.state + ")");
    if (caller !== this.sponsor) this.fail("only the sponsor can reject");
    this.submission = null;
    this.state = "Open";
    this.log("Rejected", "reject()", "submission cleared — bounty is Open again, funds stay locked");
    return true;
  };

  // dispute(caller) — only the submitting hunter
  BountyVaultSim.prototype.dispute = function (caller) {
    if (this.state !== "Submitted") this.fail("dispute needs state Submitted (now " + this.state + ")");
    if (!this.submission || caller !== this.submission.hunter) this.fail("only the hunter who submitted can dispute");
    this.state = "Disputed";
    this.log("Disputed", "dispute()", "funds stay locked until the referee resolves");
    return true;
  };

  // resolve(caller, payHunter) — referee only
  BountyVaultSim.prototype.resolve = function (caller, payHunter) {
    if (this.state !== "Disputed") this.fail("resolve needs state Disputed (now " + this.state + ")");
    if (caller !== this.referee) this.fail("only the referee can resolve");
    var paid = this.vault;
    var to = payHunter ? this.submission.hunter : this.sponsor;
    this.vault = 0;
    this.state = payHunter ? "Paid" : "Refunded";
    this.log("Resolved", "resolve(pay_hunter=" + payHunter + ")", (payHunter ? "paid " : "refunded ") + paid + " µT to " + to + " — vault now empty");
    return { paid: paid, to: to };
  };

  // cancel(caller) — sponsor only, only while Open with no submission
  BountyVaultSim.prototype.cancel = function (caller) {
    if (this.state !== "Open") this.fail("cancel needs state Open (now " + this.state + ")");
    if (caller !== this.sponsor) this.fail("only the sponsor can cancel");
    if (this.submission) this.fail("cannot cancel while a submission is waiting");
    var refunded = this.vault;
    this.vault = 0;
    this.state = "Refunded";
    this.log("Cancelled", "cancel()", "refunded " + refunded + " µT to the sponsor — vault now empty");
    return { refunded: refunded };
  };

  // timeout(epoch) — anyone, only while Open and epoch >= deadline
  BountyVaultSim.prototype.timeout = function (epoch) {
    if (this.state !== "Open") this.fail("timeout needs state Open (now " + this.state + ")");
    if (epoch < this.deadlineEpoch) this.fail("deadline not reached yet (epoch " + epoch + " < " + this.deadlineEpoch + ")");
    var refunded = this.vault;
    this.vault = 0;
    this.state = "Refunded";
    this.log("Refunded", "timeout()", "deadline passed with no accepted report — refunded " + refunded + " µT to the sponsor");
    return { refunded: refunded };
  };

  BountyVaultSim.prototype.details = function () {
    return {
      state: this.state,
      amount: this.amount,
      vault: this.vault,
      scopeHash: this.scopeHash,
      deadlineEpoch: this.deadlineEpoch,
      sponsor: this.sponsor,
      referee: this.referee,
      submission: this.submission ? { hunter: this.submission.hunter, reportHash: this.submission.reportHash, epoch: this.submission.epoch } : null,
      currentEpoch: this.currentEpoch,
    };
  };

  var api = { BountyVaultSim: BountyVaultSim, STATES: STATES, sha256Hex: sha256Hex };
  if (typeof module !== "undefined" && module.exports) module.exports = api;
  root.BountyVault = api;
})(typeof window !== "undefined" ? window : globalThis);
