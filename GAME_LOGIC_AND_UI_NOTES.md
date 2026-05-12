# MineBTC Game Logic & UI Asset Design Notes

> Canonical game truth compiled from `programs/mineBTC/ECONOMY.md`, `claude.md`, `codex-index/`, and frontend inspection.
> Use this as the single source of truth when designing CTAs, copy, animations, and custom assets.

---

## 1. THE GAME IN ONE SENTENCE

**"Pick your country, bet SOL, win claims, your HashBeast evolves, your country climbs, you earn degenBTC."**

- **degenBTC** = the mineable token. Fair-launched, mineable-only, no pre-mine. Emissions rebalance every 4h economy cycle based on price movement.
- **SOL** = the betting currency. Every bet is SOL-denominated.
- **HashBeasts** = in-game operator NFTs that boost bets, earn XP, trigger mutations (Evolution / Power / Trait), and push your country up the leaderboard.
- **Faction War** = 4h competitive cycle where own-country gameplay support determines country rankings and degenBTC reward distribution.

---

## 2. CORE ECONOMY MECHANICS (What Users Actually Earn)

### 2.1 Betting & Round Rewards (60-second rounds)

| What user does | What happens |
|---------------|-------------|
| Bets SOL on country + direction (Up/Flat/Down) | 15% protocol fee taken; 85% goes to prize pot |
| Exact country+direction wins | Gets SOL prize pot share + degenBTC round emission share (pro-rata by weighted points) |
| Same country, wrong direction | Consolation degenBTC reward (21% of round emission per wrong direction) |
| Jackpot hit (1/625 chance) | Extra degenBTC if exact winner hits the selected faction |

**Weighted points** = raw bet size × active HashBeast multiplier (capped at 4.2x).

**SOL fee split:**
- 20% of protocol fee → staker SOL reward vault
- 80% of protocol fee → SOL treasury → 80% buybacks + 3% NFT market making + ~17% dev

### 2.2 Faction War Cycle (4h cycles tied to LP burn)

- Cycle auto-starts on first bet after previous settlement
- Own-country SOL bets with active gameplay HashBeast add **gameplay score** to your country's leaderboard
- Cycle settles when economy LP burn completes
- Rankings computed from gameplay scores → rank changes resolve directions (Up/Flat/Down)
- Reward pools:
  - **Base pool** (70%): anyone who bet any country's final direction correctly
  - **Loyalty pool** (20%): own-country correct-direction supporters
  - **MVP pool** (5%): top contributors
  - **HashBeast pool** (5%): eligible gameplay HashBeasts get accumulated_val bonus

### 2.3 HODL Tax (The "Diamond Hands" Mechanic)

- When withdrawing degenBTC rewards: **10% HODL tax** charged
- Tax is **redistributed** to remaining unclaimed degenBTC balances via global `hodl_tax_index`
- **Key insight for users:** The longer you hold without withdrawing, the more you earn from other users' exits
- Fast withdrawal = immediate liquidity but pay tax
- Slower withdrawal = earn part of others' HODL taxes
- Current APR shown in UI (1D / 7D toggle)

**Withdraw math for user:**
```
gross_pending
- HODL tax (10%)
+ referral bonus (+1% if has referrer)
- transfer tax (0.1%)
= estimated receive
```

### 2.4 Referral Program (Affiliate / Growth Engine)

- **Referrer earns tiered SOL from recruit game spend** (taken from protocol fee):
  - **0.5%** of gross bet / mint price if recruit is from a **different country**
  - **1.0%** of gross bet / mint price if recruit is from the **same country**
- **Recruit gets flat +1% degenBTC bonus on withdraw** regardless of country
- Referrer SOL cap: 100,000 SOL lifetime per referrer
- Key growth loop: "bring people to your flag" — same-country recruits pay the referrer 2x more SOL

**Referral reward account tracks:**
- Total referrals, same-faction referrals
- Pending SOL rewards, total SOL earned
- Per-faction recruit counts
- Leaderboard rank (currently shows "Indexing")

### 2.5 Economy Cycle (Price → Emissions → POL)

```
Every 30 min: snapshot_price()  (8x per 4h cycle)
  → swaps 10% SOL → degenBTC (price discovery)
  → earmarks 10% SOL for POL
After 8 snapshots: update_rate()
  → if price changed ≥3%: adjust emission rate
  → price up: +1% emission
  → price down: -3% emission (asymmetric deflation)
After rate update: add_lp_and_burn()
  → deposits SOL + degenBTC into Raydium pool
  → burns ALL LP tokens permanently
```

**Token tax (0.1% on all transfers):**
- 50% burn
- 25% faction treasury
- 25% recycle to mining vault

### 2.6 Staking (Passive Income)

- **degenBTC staking**: lock up degenBTC for hashpower, earn SOL + degenBTC rewards
- **LP staking**: lock LP tokens, earn SOL + degenBTC rewards
- **Lockup multiplier**: 1x to 3x based on lock duration
- **Passive HashBeast multiplier**: up to 3x (any faction, doesn't affect gameplay)
- **Combined max boost**: 9x total

### 2.7 HashBeast Lifecycle & Multi-Species Rebirth

```
1. Mint Genesis HashBeast (1 SOL base, bonding curve, 36k cap)
2. use_hashbeast_for_gameplay() → lock NFT, cache stats
3. Play rounds → bets trigger story events, XP accumulates
4. request_hashbeast_gameplay_unlock() → wait for next faction war
5. withdraw_hashbeast_from_gameplay() → sync DNA/XP/multiplier back

Story events (claim-time mutation rolls):
- Evolution (~10%): +50 base multiplier, DNA upgrades, XP resets
- Power (~30%): +25 base multiplier, power trait upgrade
- Trait (~60%): +5 base multiplier, visual trait upgrade

Rebirth (max 7 per asset):
- Claims accumulated_val degenBTC
- Resets multiplier, XP, breed count, DNA
- **Species changes on rebirth** — each rebirth_count maps to a new species:
  - rebirth 0 (genesis) = Doge/Canine
  - rebirth 1 = Pepe
  - rebirth 2 = Monkey/Ape
  - rebirth 3 = Cat
  - rebirth 4+ = TBD (alien, robot, dragon, etc.)
- Asset goes to country lootbox queue or burns
- New owner who wins the lootbox roll gets the **reborn species**

Breeding (post-genesis sellout):
- Same-country, same-rebirth-generation parents
- Price: max(curve_price, 1.5× floor anchor)
- 50% SOL + 50% degenBTC payment
```

**DNA Structure (256 bits / 32 bytes):**
- Faction (4 bits, offset 0): country ID
- Evolution Stage (3 bits, offset 4): 0-7
- Appearance Genes (105 bits, offset 7): 7 groups × 3 traits × 5 bits
- Combat Genes (60 bits, offset 112): 5 groups × 3 traits × 4 bits
- Breed/Body Type (2 bits, offset 172): body variant within faction
- **Rebirth Count (3 bits, offset 174): rebirth generation (0-7) — drives species mapping**
- Reserved (79 bits)

**Event-Driven Asset Generation Pipeline:**
The contract emits on-chain events that the backend indexer listens to, triggering automated asset generation:

| Event | Trigger | Asset Generated |
|-------|---------|----------------|
| `StoryEventTriggered` | Winning claim + mutation roll | New portrait/animation for the HashBeast reflecting mutation type (Evolution/Power/Trait) |
| `HashBeastEvolution` | Evolution event fires | Full transformation sequence: base → evolved form video/animation |
| `HashBeastPowerMutation` | Power trait upgrade | Power aura overlay, strength visual update |
| `HashBeastVisualMutation` | Visual trait upgrade | New accessory/appearance element generated |
| `HashBeastRebirthBurned` | Rebirth completes | New species reveal video: egg crack → species emergence |

**Backend flow:** Indexer catches event → reads `rebirth_count` + `new_dna` → calls image/video generation API → updates NFT metadata URI → pushes to frontend via socket.

This means every rebirth and every mutation produces **unique generative art** tied to the on-chain event. The NFT is a living asset that evolves visually as it progresses through the game.

---

## 3. CURRENT UI STATE ANALYSIS

### 3.1 REFERRAL DROPDOWN (TopBar)

**Current states:**

| State | What's shown |
|-------|-------------|
| **Not registered** | "Open your recruit desk." + "Earn 1% SOL from recruit game spend. Recruits get +1% degenBTC on withdraw." + "Register to refer" button |
| **Registered, no claimables** | Affiliate desk online. Claimable SOL: 0. Total earned. Recruits count. Leaderboard rank: "Indexing". Recruit code + copy. "No SOL ready yet. Share your code and earn when recruits play." |
| **Registered, has claimables** | Same + "Claim referral SOL" button |

**Current animation:** CSS-only tower/signal/coin animation in the hero of the popover.

**Problems:**
- "Indexing" for leaderboard rank feels broken/untrustworthy
- No visual excitement for earning SOL — just dry metric rows
- No social proof ("X users referred this week", "Top referrer earned Y SOL")
- No clear CTA for sharing beyond a copy button
- The "tower" animation is generic — doesn't tie to the game's Bitcoin/ninja/degen branding

### 3.2 HODL TAX DROPDOWN (TopBar)

**Current states:**

| State | What's shown |
|-------|-------------|
| **Not registered** | "Mine degenBTC, then hold the bag." + "Register and mine degenBTC before the vault starts tracking yield." + "Register to mine" button |
| **Registered, 0 balance, 0 withdrawable** | "Diamond hands earn from exits." + "Withdrawers pay HODL tax. Remaining miners collect the redistributed degenBTC." |
| **Registered, has dBTC balance, 0 withdrawable** | "Diamond hands earn from exits." |
| **Registered, has withdrawable** | "Rewards mined. Exit or compound?" + withdraw preview breakdown |

**Current animation:** CSS bag + coins + yield sparkles.

**Problems:**
- The copy is too tame for a degen product
- No dopamine hit when showing earned amounts
- Withdraw preview is a dry accounting table — feels like a tax form, not a casino payout
- No celebration animation when HODL APR is high
- Missing: "You earned X dBTC from others' paper hands this cycle"
- The bag animation is generic, not Bitcoin-branded

### 3.3 JOIN ARENA / USER CONSOLE (Right Panel)

**Current states:**

| State | What's shown |
|-------|-------------|
| **No wallet** | "Join the Arena." + "Pick your permanent country. Mine degenBTC in 60-second rounds and push your faction through the 4h cycle." + static image of HashBeast mining + "Pick country" button |
| **Wallet connected, not registered** | Same as above |
| **Registered** | Changes to betting console (PickConsole) |

**Current asset:** `/game-assets/user-console/hashbeast-mining-operator.png` — static PNG of a HashBeast with mining gear.

**Problems:**
- The static image is boring — no animation, no pulse
- Copy is functional but not exciting
- Doesn't explain WHY they should register (what do they get?)
- Missing: "Fair-launched degenBTC. Bitcoin but more degen."
- Missing: social proof / live stats ("X players mining", "Y SOL paid out today")
- The "Read more" expandable section has generic CSS animations, not compelling

### 3.4 HASHBEAST NFT SALE (Bottom Panel)

**Current states:**

| State | What's shown |
|-------|-------------|
| **Not registered** | "Unlock your in-game operator." + "Register your {country} callsign to mint a matching HashBeast, boost Arena runs, and help your country climb." + Corgi portrait + benefit tags (GAMEPLAY OPERATOR, XP + MUTATIONS, COUNTRY SCORE BOOST, BONUS TICKETS, GENESIS MINT) + mint price chart + "Register country to mint" button |
| **Registered, no HashBeast** | "Mint a HashBeast operator." + "A gameplay HashBeast adds XP, multiplier, mutations, and identity to your runs." |
| **Registered, wrong faction HashBeast** | "Need a {country} HashBeast." + "You own X HashBeasts, but none match your registered country." |
| **Registered, ready to lock** | "Lock for gameplay." + "{HashBeast} can enter custody now and start earning XP from bets." |
| **Locked live** | "Operator mining." + "{HashBeast} is active while autominer runs." |
| **Locked idle** | "Operator locked." + "Ready for the next run." |
| **Unlock pending** | "Unlock queued." + "Withdrawal opens after the current compute cycle. About Xm remaining." |
| **Withdraw ready** | "Withdraw ready." + "Return your HashBeast to wallet custody." |

**Current asset:** Static faction portraits (Corgi, etc.) + bonding curve chart.

**Problems:**
- Copy doesn't communicate the **in-game asset utility** well enough
- Missing: "Help your country climb the leaderboard" → should show LIVE leaderboard position
- Missing: "Earn degenBTC from your HashBeast" → accumulated_val mechanic not surfaced
- Missing: "Genesis mint = forever scarce" → 36k cap, post-genesis only via breeding
- Missing: "Your HashBeast powers the game's compute budget" → gameplay scores directly affect emissions
- The bonding curve chart is dry — needs to feel like a mint event, not a stock chart
- No urgency / FOMO on the genesis mint

---

## 4. RECOMMENDED UI CHANGES

### 4.1 REFERRAL DROPDOWN — Make It a Growth Engine

**0-state (not registered):**
- Headline: "Recruit degens. Earn SOL." 
- Sub: "Every player you bring to your flag pays you 1% of their game spend. Forever."
- CTA: "Register to unlock your recruit code"
- Asset: Animated "recruit beacon" — a Bitcoin-ninja signal tower pulsing with your country's flag color

**Active state (registered):**
- Hero: Big animated number of **lifetime SOL earned** with coin-rain animation
- Metrics: 
  - "This cycle: X SOL" (not just total)
  - "Recruits: Y" with country flags
  - "Your rank: #Z" (need backend for this)
- CTA: "Copy recruit link" → should generate a shareable URL, not just a code
- Social proof bar: "Top 10 referrers earned X SOL this week" (if data available)
- Claim button: "Claim X SOL" with a satisfying confetti animation on click

**New addition:**
- "Same-country bonus" callout: "Recruit to your flag → earn 5% instead of 3%"
- "Referral leaderboard" mini-list inside the dropdown

### 4.2 HODL TAX DROPDOWN — Make It a Casino Payout Experience

**0-state (not registered):**
- Headline: "Diamond hands get paid."
- Sub: "Mine degenBTC. Hold the bag. Collect 10% from every paper-hand exit."
- CTA: "Register + start mining"
- Asset: Empty degenBTC vault / bag with "0" — but the vault pulses with potential

**Accumulating state (has dBTC, not withdrawing):**
- Hero: Animated bag filling with golden degenBTC coins
- Big number: "You earned X dBTC from HODL tax this cycle" ← THIS IS THE DOPAMINE HIT
- Sub: "Y users paper-handed. You collected Z% of their tax."
- APR badge: "Live HODL APR: X%" with a flame/spark animation when APR > 50%
- CTA: "Hold longer → earn more"

**Withdraw state (has claimable):**
- Don't show a tax form. Show a **slot machine / reveal** animation:
  - "Gross: X dBTC"
  - "HODL tax: -Y dBTC (paid to diamond hands)"
  - "Transfer tax: -Z dBTC (50% burned, 25% treasury, 25% recycled)"
  - "You receive: W dBTC" ← big reveal with animation
- CTA: "Withdraw" or "Compound into stake"
- Asset: A lever-pull or vault-unlock animation

**New addition:**
- "Tax flywheel" explainer: "Every withdrawal makes the bag heavier for holders."
- Cycle countdown: "Next redistribution in X:XX" (ties to 4h cycle)

### 4.3 JOIN ARENA / USER CONSOLE — Make It an Onboarding Event

**0-state (no wallet / not registered):**
- Headline: "Mine degenBTC. Bitcoin, but degen."
- Sub: "Fair-launched. Mineable-only. Emissions rebalance every 4h based on price. Pick your country and start mining in 60-second rounds."
- Live stats banner: "X players mining | Y SOL paid out today | Z dBTC mined this cycle"
- CTA: "Pick your country →"
- Asset: **Animated degenBTC mining rig** — a HashBeast operator running a Bitcoin ASIC miner, with degenBTC coins flying out, sparks, and a live countdown to next round
- Secondary CTA: "Watch a round" (spectate mode — show last round results)

**Registered but not betting:**
- Show: "Your country: {flag} {name}"
- "Next round starts in Xs"
- "Your HashBeast: {name} | Multiplier: X.Xx"
- Quick-bet buttons: "Bet 0.1 SOL on Up" etc.

### 4.4 HASHBEAST NFT SALE — Make It a Mint Event

**Not registered state:**
- Headline: "HashBeasts power the Arena."
- Sub: "Genesis operators boost your bets, earn XP, mutate DNA, and push your country up the leaderboard. Only 36,000 will ever exist."
- Benefit list (with icons/animations):
  1. **Gameplay multiplier** — up to 4.2x on bets
  2. **XP & mutations** — Evolution, Power, Trait events on wins
  3. **Country score boost** — your bets directly help your faction climb
  4. **Bonus tickets** — free bets for joining
  5. **degenBTC earnings** — HashBeasts accumulate dBTC from story events
  6. **Genesis scarcity** — 36k cap. Post-genesis only via breeding.
- Mint price: "Current price: X SOL" with bonding curve visualization
- Scarcity bar: "Y / 36,000 minted" with a progress bar
- CTA: "Register country to mint →"
- Asset: **Animated HashBeast portrait** — the specific one for their selected/country with idle animation, plus a "mint card" flip animation on hover

**Registered, ready to mint:**
- Headline: "Mint your {country} operator."
- Show the specific HashBeast for their country (not generic Corgi)
- "Mint price: X SOL" + "You'll get: {N} bonus tickets"
- Scarcity countdown feel: "Z mints remaining at this price"
- CTA: "Mint HashBeast →"

**Registered, has HashBeast:**
- Show operator status panel with:
  - Portrait (animated)
  - Multiplier: X.Xx
  - XP: Y / next threshold
  - Accumulated dBTC: Z
  - Status: "Locked in gameplay" / "Ready to lock"
- CTA: "Lock for gameplay" / "Request unlock"

---

## 5. CUSTOM ASSET IDEAS & GENERATION PROMPTS

### 5.1 Referral System Assets

**Asset: Referral Beacon (0-state hero)**
```
A glowing signal tower made of Bitcoin blocks and golden SOL coins, with a ninja eye-mask motif on top. The tower pulses with cyan signal waves. Scattered around the base are small HashBeast silhouettes looking up at the beacon. Dark void background with warm amber/cyan glow. Style: pixel art meets crypto degen aesthetic. Bitcoin branded but with playful HashBeast energy.
```

**Asset: Recruit Coin Rain (active state background)**
```
Golden SOL coins falling like rain against a dark background, with a subtle grid pattern. Some coins have the HashBeast logo. Cyan and amber light streaks. Casino/chip-fall energy but Bitcoin-crypto themed. Looping animation frames.
```

**Asset: Referral Leaderboard Trophy**
```
A golden trophy shaped like a Bitcoin with a ninja mask, sitting on a pedestal made of stacked HashBeast paws. Small flags of different countries around the base. Glowing cyan accents. Style: premium game asset, not cartoony.
```

### 5.2 HODL Tax Assets

**Asset: Empty degenBTC Vault (0-state)**
```
An open vault door with dark interior, a single dust mote floating inside. The vault is shaped like a Bitcoin with a ninja mask. On the floor in front: a pickaxe and a mining helmet. Outside the vault: small HashBeast miners looking in hopefully. Warm amber light from outside, cold darkness inside. Style: detailed pixel art, dramatic lighting.
```

**Asset: Filling degenBTC Bag (accumulating state)**
```
A leather bag with the degenBTC logo (Bitcoin + ninja mask) gradually filling with golden glowing coins. Each coin has a small "dBTC" engraving. As the bag fills, it pulses with warm light. Sparks and particle effects around the top. Dark background. Style: game UI asset with particle animation frames.
```

**Asset: Diamond Hands Badge**
```
A pair of diamond-textured hands holding a glowing degenBTC coin. The hands have a subtle circuit-board pattern. Behind: a dark background with faint charts showing upward price movement. Cyan and gold color palette. Style: crypto degen badge, suitable for achievement unlock.
```

**Asset: Paper Hands Burn Animation**
```
A paper hand dropping a degenBTC coin into a flaming furnace. From the furnace smoke, golden particles float up and rain down onto diamond hands below. Dark background, fire orange and gold palette. Style: 8-frame sprite sheet for game animation.
```

**Asset: Vault Unlock / Withdraw Reveal**
```
A massive vault door with Bitcoin-ninja branding slowly opening with hydraulic steam. Inside: a glowing stack of degenBTC coins with light rays. A lever on the side pulses. When opened, confetti and spark particles burst out. Style: dramatic game cutscene asset.
```

### 5.3 Join Arena / Onboarding Assets

**Asset: degenBTC Mining Rig (hero animation)**
```
A HashBeast operator (doge-like character with mining helmet and headlamp) operating a massive Bitcoin ASIC mining rig. The rig has spinning fans with cyan LED lights. degenBTC coins shoot out of a chute like a slot machine. Sparks fly from welding points. In the background: a dark mine shaft with country flags hanging. Countdown timer "00:47" glows red. Style: animated sprite sheet, 8 frames, pixel art with modern lighting.
```

**Asset: Country Selection Globe**
```
A spinning globe made of Bitcoin blocks, with 12 country flags as hotspots. When a country is selected, a beam of light shoots up and the flag transforms into a HashBeast silhouette. Cyan energy lines connect countries. Dark space background. Style: futuristic game UI asset.
```

**Asset: Round Spectator Mode Preview**
```
A compressed view of the arena: three lanes (Up/Flat/Down) with country flags racing. The winning lane explodes with golden coins and confetti. A countdown clock ticks. HashBeast operators cheer from the sidelines. Style: thumbnail/preview card for spectate mode.
```

### 5.4 HashBeast NFT Sale Assets

**Asset: Genesis Mint Card (per country)**
```
A premium trading card frame with golden borders and Bitcoin circuit patterns. Inside: the specific HashBeast for the country (e.g., USA eagle-doge, China panda-doge, Russia bear-doge) in heroic pose wearing mining gear. The card has holographic shimmer. At bottom: "GENESIS | 1/36000" with a progress bar. Style: collectible card game asset, high detail.
```

**Asset: Bonding Curve Visualizer**
```
Not a chart — a visual of a conveyor belt with HashBeast eggs moving along it. As eggs pass, they hatch into increasingly rare/more-glowing HashBeasts. The belt speeds up as price increases. Behind: a factory with "MINT" in neon lights. Style: animated game asset, 8 frames.
```

**Asset: HashBeast XP Evolution Sequence**
```
Three-frame transformation: 
1. Base HashBeast (glowing faintly)
2. Power-up HashBeast (sparks, aura)
3. Evolution HashBeast (fully glowing, DNA strands visible, max multiplier aura)
Style: transformation animation sprite sheet, dramatic lighting.
```

**Asset: Country Leaderboard Climb**
```
A mountain peak with a flag at the top. HashBeasts from different countries climb the mountain. Your country's HashBeast is highlighted with a golden aura. As it climbs, it pushes others down. Confetti at the summit. Style: game achievement animation, 6 frames.
```

**Asset: Mint Celebration Burst**
```
On successful mint: a HashBeast egg cracks open with golden light, the specific HashBeast emerges with a flex pose, and "MINTED" text explodes in fireworks. Country flag banner unfurls behind. Style: celebration cutscene asset, 10 frames.
```

### 5.5 General Game Assets (Reusable)

**Asset: degenBTC Coin (for all animations)**
```
A golden coin with the Bitcoin B but with a ninja mask over it. The coin has a subtle circuit pattern on the rim. When spinning, it shows "dBTC" on the reverse. Cyan glow on edges. Style: game currency icon, high detail, suitable for particle systems.
```

**Asset: SOL Coin (for referral animations)**
```
A purple-blue coin with the Solana S but stylized with HashBeast ears. Cyan and purple gradient. Style: game currency icon, matches degenBTC coin style.
```

**Asset: 4h Cycle Countdown Orb**
```
A glowing orb that fills clockwise over 4 hours. When full, it explodes into a mini economy cycle visualization: price snapshot → rate update → LP burn. The orb has Bitcoin and degenBTC symbols inside. Style: HUD element, animated sprite sheet.
```

**Asset: Mutation Event Burst**
```
Three variants:
- Evolution: rainbow DNA helix + lightning + HashBeast silhouette transforming
- Power: red/orange flame burst + muscle flex pose
- Trait: green/teal particle swirl + new accessory appearing
Style: 8-frame sprite sheets, one per mutation type.
```

---

## 6. USER JOURNEY MAP

### Journey 1: First Visit (No Wallet)
1. Lands on Arena → sees live round countdown, country leaderboard
2. **Join Arena panel** shows mining rig animation + "Mine degenBTC. Bitcoin, but degen."
3. **HODL TAX badge** pulses → hover shows empty vault + "Diamond hands get paid"
4. **REF badge** pulses → hover shows beacon + "Recruit degens. Earn SOL."
5. Clicks "Pick country" → country selector with globe animation
6. Connects wallet → registers with referral code option

### Journey 2: Registered, First Round
1. **User Console** → PickConsole with quick-bet buttons
2. **HashBeast panel** → "Mint your operator" with genesis card animation
3. Bets 0.1 SOL → round animation plays
4. Wins/loses → claim screen with mutation roll animation
5. **HODL TAX** → bag starts filling with first dBTC

### Journey 3: Daily Player (Active)
1. Opens app → sees accumulated dBTC, live round, country rank
2. **HashBeast panel** → operator mining animation, XP bar filling
3. Checks **HODL TAX** → "You earned X from paper hands this cycle"
4. Checks **REF** → "3 recruits | 0.12 SOL claimable"
5. Claims round rewards → mutation event animation plays
6. Decides: withdraw (vault unlock) or hold (bag keeps filling)

### Journey 4: Whale / Referrer
1. **REF dropdown** → big number: "Lifetime: 45.2 SOL earned"
2. Referral leaderboard → "#3 this week"
3. **HODL TAX** → massive bag, high APR, compound button
4. Breeds HashBeasts → breeding animation, new offspring revealed

---

## 7. COPY TONE GUIDE

**Always:**
- Lead with the degen angle, not the technical angle
- Use "mine", "earn", "claim", "evolve", "climb", "hold", "bag"
- Reference Bitcoin but always add the degen twist
- Celebrate wins with exaggeration ("LFG", "diamond hands", "paper hand tax")
- Show numbers prominently — users want to see what they earned

**Never:**
- Say "prediction market", "geopolitical risk", "oracle", "intelligence"
- Use bank/finance language ("yield", "APY", "dividends") — use game language instead
- Make it feel like work — it's a game first, investment second

**Example translations:**
- "Stake for yield" → "Lock your bag, earn while you sleep"
- "Withdraw with tax" → "Paper hands pay diamond hands"
- "Referral program" → "Recruit desk"
- "Emission rate" → "Mining power"
- "Bonding curve" → "Mint price climbs as supply drops"

---

## 8. OPEN QUESTIONS / BACKEND NEEDS

For the frontend to fully realize these designs, we need:

1. **Referral leaderboard API** — current rank, top 10, weekly stats
2. **HODL tax cycle earnings** — "You earned X dBTC from tax this cycle" per user
3. **Paper hand counter** — "Y users withdrew this cycle" for social proof
4. **Live payout stats** — "X SOL paid out today" for onboarding social proof
5. **HashBeast accumulated_val** — show in UI so users see their NFT earning dBTC
6. **Spectate mode** — show last round results without betting (for 0-state engagement)
7. **Country-specific HashBeast previews** — don't show generic Corgi, show the actual one for their country

---

*Document version: 2026-05-09*
*Source: MineBtc-fi contracts + mdogeWifBtcFE frontend*
*Next update: After asset generation sprint*
