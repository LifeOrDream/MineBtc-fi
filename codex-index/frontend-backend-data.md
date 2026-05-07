# Frontend and Backend Data Implications

This file translates contract truth into backend API/socket and frontend state requirements. It should be refined during Phase 1 after scanning frontend TODOs and backend capabilities.

## Global App Data

Must be loaded once, cached, and reused:

- Program ID, DegenBTC mint, Raydium pool, token decimals, supported factions.
- Global config:
  - pause state, fee config, degenBTC distribution config, snapshot interval, gameplay tuning.
- Global game state:
  - current round ID, active/can begin flags, round duration, last round, jackpot pot.
- Current/next round summary:
  - start/end timestamps, countdown, stage, current totals by faction-direction.
- Current faction war:
  - cycle ID, active/settled stage, settlement target, rankings, gameplay scores, resolved directions when settled, cycle reward pools.
- Economy:
  - emission rate, recent/average/track price, price history, SOL for POL, LP operation count, LP token price, mining multiplier.

## Wallet/User Data

Load on wallet connection and refresh by wallet-scoped socket/API deltas:

- Player registration state and home/origin faction.
- Referral state and referral reward account.
- Current round bet, pending round claims, pending faction-war claims.
- User faction-war bets and estimated cycle rewards.
- Wallet balances: SOL, DegenBTC, LP tokens, relevant token accounts.
- Player rewards: pending SOL, pending degenBTC, unrefined degenBTC, HODL tax state.
- Staking positions, hashpower, multipliers, pending rewards.
- Doge inventory, Doge metadata, staked Doges, gameplay Doge, free mint allowance.
- Autominer status: active config, reserve, rounds remaining, last executed round, claim/reload availability.

## Public Analytics/Data-Room Data

Backend should expose cached aggregate endpoints for:

- New users by day/hour.
- Repeat users and retention cohorts.
- Active users, unique bettors, unique Doge minters, unique autominer users.
- SOL volume by round/day/cycle.
- degenBTC rewards distributed/mined/claimed.
- Doge genesis mints by faction and revenue.
- Referral leaderboard and same-faction referral metrics.
- Faction leaderboard, country distribution, cycle outcomes.
- Economy chart data: price snapshots, emission changes, POL additions, LP burns, token tax distribution.

## Socket Model

Suggested public topics:

- `global:snapshot` - compact global app snapshot.
- `round:current` - current round countdown/stage/totals.
- `round:ended` - winner, reward indexes, jackpot result.
- `faction-war:current` - ranks, scores, cycle telemetry, reward pools.
- `economy:update` - price snapshots, emission/multiplier changes, POL/burn events.
- `doges:mints` - public Doge mint stream.
- `analytics:headline` - lightweight dashboard counters.

Suggested wallet-scoped topics:

- `user:{wallet}:player`
- `user:{wallet}:round`
- `user:{wallet}:rewards`
- `user:{wallet}:staking`
- `user:{wallet}:doges`
- `user:{wallet}:autominer`

Frontend should subscribe once at app shell level and hydrate Redux/store slices from socket deltas. Page components should read store selectors rather than triggering duplicate fetches.

## API Model

Suggested minimal page-load endpoints:

- `GET /api/bootstrap` - public global config, current round, faction war, economy headlines, factions.
- `GET /api/user/:wallet/bootstrap` - all wallet-specific state needed after connect.
- `GET /api/rounds/:roundId` - round detail/history.
- `GET /api/faction-wars/:cycleId` - cycle detail/history.
- `GET /api/economy/chart` - chart-ready economy timeline.
- `GET /api/leaderboards/*` - factions, referrals, users, Doges.
- `GET /api/data-room/*` - investor analytics.

Use short TTL Redis caching for public bootstrap/hot endpoints and event-driven invalidation for round/cycle/economy/user updates.
