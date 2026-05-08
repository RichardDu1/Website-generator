# Live Test Checklist

Pre-flight for the first real-money Gengar run. Work top-down — each item
guards against a known failure mode from the development history.

---

## 1. Environment

- [ ] Rust toolchain: `rustc --version` shows ≥ 1.65 (we use modern-ish syntax)
- [ ] On Windows: `bash` available for git operations
- [ ] Release build compiles cleanly: `cargo build --release -p v3-bot`
- [ ] All tests pass: `cargo test --workspace` → **114 passed, 0 failed**

## 2. Wallet & Funding

- [ ] `.env` file in project root with:
  - `PRIVATE_KEY=0x...` (EOA wallet, Polygon-mainnet, funded with USDC.e)
  - `DRY_RUN=false`
  - `BANKROLL_USDC=<your cap>` (acts as an upper bound; actual bankroll is
    refreshed every 60s from on-chain USDC.e balance)
- [ ] Wallet has **≥ 30 USDC.e** on Polygon (≥ max_bet_usdc + headroom)
- [ ] Wallet has **≥ 0.1 MATIC** for gas on order-signing
- [ ] Wallet has been approved for Polymarket CLOB trading (one-time, see
      Polymarket docs — `gnosis safe` / EOA distinction matters here)
- [ ] Double-check you're on **Polygon mainnet** (`CHAIN_ID=137`), not Amoy testnet

## 3. Optional: Telegram notifier

- [ ] Create a bot via [@BotFather](https://t.me/BotFather), grab the token
- [ ] Get your `chat_id` by sending any message to the bot, then:
      `curl https://api.telegram.org/bot<TOKEN>/getUpdates`
- [ ] Add to `.env`:
  - `TELEGRAM_BOT_TOKEN=...`
  - `TELEGRAM_CHAT_ID=...`
- [ ] Verify: on bot startup you should receive `🚀 Gengar bot started` message

## 4. Config review (`config/bot.toml`)

Defaults are conservative. Review against the checklist:

| Knob | Default | Check |
|---|---|---|
| `dry_run` | **must be `false` for live** | 🔴 blocking |
| `tick_interval_ms` | 500 | keep as-is |
| `strategies.gengar.enabled` | true | |
| `min_edge` | 0.05 | 5% — keep as-is for first test |
| `min_prob` | 0.80 | |
| `max_bet_usdc` | 25 | **lower to 10 for first live run** |
| `profit_lock_ratio` | 0.30 | |
| `max_position_hold_secs` | 900 | |
| `preemptive_cancel_prob` | 0.55 | |
| `risk.max_daily_loss` | 10.0 | **set to no more than you can afford to lose** |

**Action**: open `config/bot.toml`, reduce `max_bet_usdc` to 10 and
`max_daily_loss` to your comfort level before first live run.

## 5. Dry-run smoke test (5 min)

Before live, always re-verify the Gengar path works:

```bash
DRY_RUN=true BANKROLL_USDC=100 \
  RUST_LOG=info \
  ./target/release/v3-bot.exe 2>&1 | tee /tmp/dry.log
# Let run 5 minutes, then Ctrl-C
```

Expected log evidence:
- [ ] `📥 Loaded window M5: btc-updown-5m-*` (and M15, and H4)
- [ ] `⚓ Anchored opening price` for each window shortly after
- [ ] If BTC is moving: either `✅ Gengar entry` **or** skip lines with reasons
- [ ] **NO** references to `Momentum`, `MarketMaking`, `SpreadCapture`,
      `Gabagool`, `pair_leg`, `EnterPair` (those strategies were removed)
- [ ] `./logs/signals.csv` and `./logs/trades.csv` are created with headers

If any window never loads: check your network can reach
`gamma-api.polymarket.com` (some VPS regions are blocked).

## 6. First live run protocol

### Start
```bash
# Copy bot.toml knobs you want (max_bet_usdc, max_daily_loss)
DRY_RUN=false BANKROLL_USDC=30 \
  RUST_LOG=info,v3_mm_exchange=warn \
  ./target/release/v3-bot.exe 2>&1 | tee -a logs/live.log &
```

### Watch for
- [ ] Telegram `🚀 Gengar bot started` with **mode=LIVE**
- [ ] First `💰 Bankroll refresh` message within 60s showing your actual wallet
- [ ] First entry (may take minutes to hours depending on BTC volatility):
      Telegram `✅ ENTRY` + `logs/trades.csv` row
- [ ] First exit: `🔒 EXIT profit-lock` **or** `⚡ EXIT preemptive-cancel`
      **or** `⏱️ FORCE-SELL` with PnL

### Stop conditions (bail immediately, don't let the bot keep running)
- [ ] Multiple `💥 Force-sell FAILED` messages → wallet/network broken
- [ ] `max_daily_loss` tripped (Telegram stops; check `logs/trades.csv`)
- [ ] Unexpected `logs/signals.csv` entries you can't classify
- [ ] Any sustained reconnect storm >10 min (CLOB WS keeps failing)

### Stop gracefully
```bash
# find the pid
kill -SIGINT <pid>   # or Ctrl-C in foreground
```

## 7. Post-run analysis (first 24h)

Export the CSVs somewhere safe, open in any tool:

```python
import pandas as pd
s = pd.read_csv("logs/signals.csv")
t = pd.read_csv("logs/trades.csv")

# Skip reason breakdown: where are we bleeding opportunities?
print(s[s.action == "skip"].skip_reason.value_counts())

# Entry → exit PnL
entries = t[t.event == "entry"]
exits = t[t.event.str.startswith("exit-")]
print(f"Entries: {len(entries)}, Exits: {len(exits)}")
print(f"Total realized PnL: {exits.realized_pnl.sum():.2f}")
print(f"Win rate: {(exits.realized_pnl > 0).mean()*100:.1f}%")
print(f"Avg winner: {exits[exits.realized_pnl > 0].realized_pnl.mean():.2f}")
print(f"Avg loser:  {exits[exits.realized_pnl < 0].realized_pnl.mean():.2f}")
```

Key questions for first 24h:
1. Did any Gengar entry actually trigger? If zero → tune `min_edge` down
   or `min_btc_delta_pct` down (but **only** after reviewing signals.csv
   skip reasons — don't guess)
2. What % of exits were preemptive-cancel vs profit-lock vs force-sell?
   Too many force-sells → tick loop is stalling (check for network issues)
3. Did realized vol stabilize after 12 windows (1h for 5m, 3h for 15m,
   12h for H4)? Compare to 0.12 fallback

## 8. Known gotchas from dev history

- **H1 / 1h market doesn't exist on Polymarket** — we use 4h instead (verified
  via Gamma API on 2026-05-08). If you see `btc-updown-1h-*` slug in logs,
  something's wrong.
- **State file isolation**: `state/daily_pnl.json` is process-local. Don't
  run two bot instances from the same directory — they'll corrupt each
  other's daily loss tracking.
- **Post-only fallback**: executor tries post-only first (0% maker fee),
  falls back to GTC (~2% taker fee) only if the order would cross. This
  is deliberate — missing a fill costs more than the fee on Gengar entries.
- **CLOB WS reconnects are noisy**: they get demoted to `warn!` with
  `RUST_LOG=info,v3_mm_exchange=warn`. Recommended for production logs.

## 9. If things go wrong

1. **Stop the bot first**: `Ctrl-C` / `kill -SIGINT <pid>`
2. **Check positions manually**: Polymarket web UI → your portfolio. If
   positions exist that the bot didn't track in `logs/trades.csv`, close
   them manually (they're orphaned due to `force-sell FAILED` paths)
3. **Inspect `logs/trades.csv`** for the last entry and last exit
4. **Inspect `logs/live.log`** tail for the panic or error
5. **File an issue** with those three artifacts + bot version (`git rev-parse HEAD`)

## 10. Graduation criteria for scaling up

Before raising `max_bet_usdc` past $10, demand:

- [ ] 100+ entries logged across ≥ 1 week
- [ ] Win rate ≥ 55% (otherwise the edge is not real or vol is
      mis-calibrated)
- [ ] Realized PnL > fees paid (`(sum of non-zero maker) + 2% × (sum of
      taker fills)` — parse from `order_id` status via CLOB API if needed)
- [ ] At least one 4h window trade (H4 windows only enter a few times per day)
- [ ] Observed a `preemptive-cancel` fire at least once (means the adverse-move
      protection is actually working, not just dead code)
