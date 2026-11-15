# BountyVault

On-chain escrow for bug bounties on Tari Ootle.

## The problem

Bug bounties run on trust, and trust breaks both ways: sponsors ghost hunters after a valid report, and hunters fear doing the work and never getting paid. BountyVault removes the trust from the payment. A sponsor locks the bounty in TARI when the bounty is created. A hunter submits only a hash of their report. The sponsor approves — the whole vault pays the hunter — or rejects and the bounty reopens. A hunter who disagrees can dispute, and a referee named at creation settles it. A bounty nobody claims refunds in full once its deadline epoch passes.

Only hashes go on chain: the scope hash and the report hash. The scope text and the report itself stay off-chain. A public bounty board should not leak what was found, or where.

## How it works

One component per bounty. States: `Open → Submitted → Paid`, `Open → Submitted → Open` (reject), `Open → Submitted → Disputed → Paid | Refunded`, `Open → Refunded` (cancel or timeout).

| Method | Who | What it does |
| --- | --- | --- |
| `create(bucket, scope_hash, deadline_epoch, referee, sponsor_account)` | anyone (caller becomes sponsor) | Locks the TARI bucket in the vault. State: Open |
| `submit(report_hash, hunter_account)` | anyone, while Open and before the deadline | Records the caller as hunter + the report hash. State: Submitted |
| `approve()` | sponsor only | Pays the **entire** vault to the recorded hunter. State: Paid |
| `reject()` | sponsor only | Clears the submission, reopens the bounty. Funds stay locked. State: Open |
| `dispute()` | the submitting hunter only | State: Disputed. Funds stay locked |
| `resolve(pay_hunter)` | referee only | `true` pays the whole vault to the hunter (Paid); `false` refunds the whole vault to the sponsor (Refunded) |
| `cancel()` | sponsor only, while Open | Refunds the whole vault to the sponsor. State: Refunded |
| `timeout()` | anyone, once the deadline epoch is reached | Refunds the whole vault to the sponsor. State: Refunded |
| `get_state()` / `get_details()` | anyone | Read-only views |

Rules the template enforces (not just the UI):

- Every payout moves the **whole** vault — no partial amounts, no dust left behind.
- `approve` can only ever pay the recorded hunter. The sponsor can never pay themselves through it.
- The referee is fixed at creation and can never be changed.
- Caller checks use the engine's transaction signer public key, compared against the sponsor / hunter / referee keys stored in the component.
- The deadline is the chain epoch (`Consensus::current_epoch()`), not wall-clock time.

## The demo

`web/` is a static, walletless simulation of the exact same state machine — no build step, no runtime dependencies. Open `web/index.html` in a browser (or serve the folder) and walk through create → submit → approve, or the dispute path. Every action shows the Ootle call it maps to, and the scope/report text is hashed in the browser so you can see exactly what would go on chain (only the hash).

The demo has a "Live (Sapient / Esmeralda)" tab. It is a clearly-labeled placeholder: the template has not been published to any network yet, and the tab says so. Nothing in the demo moves real funds, and nothing pretends to.

## Build and test

Requires Rust (stable) with the `wasm32-unknown-unknown` target:

```sh
rustup target add wasm32-unknown-unknown
cd template
cargo test            # engine tests (tari_template_test_tooling)
cargo build --release --target wasm32-unknown-unknown
```

## Honest limits

- Not audited. The template is small and every transition is covered by engine tests, but do not lock meaningful amounts in it until it has been reviewed.
- Simulation-first: the web demo mirrors the template logic, but it is not the template. The template has not been published to Esmeralda or to mainnet yet, so there is no live path to try today.
- The hunter supplies their own payout account at submit time, and the sponsor supplies their refund account at creation. Each is the only party hurt by supplying a wrong one, but the template cannot verify ownership of an account address — only of the signing key.
- The referee is trusted to judge honestly. That is inherent to any referee model; the chain holds the money and follows the call, it cannot see whether a report is valid.

## License

MIT. See [LICENSE](LICENSE).
