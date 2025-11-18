# Frontend Usage: `simulate_purchase_cost`

## Overview
The `simulate_purchase_cost` function is a read-only query function that calculates the cost of minting multiple Dragon Eggs and returns ticket information without modifying any on-chain state.

## Return Value
Returns a tuple: `(total_price: u64, individual_prices: Vec<u64>, ticket_amounts_per_tier: Vec<(u64, u64)>)`

- **total_price**: Total SOL cost (in lamports) for minting `mint_count` eggs
- **individual_prices**: Array of individual prices for each egg (in lamports)
- **ticket_amounts_per_tier**: Array of `(ticket_value, ticket_count)` tuples for each of the 3 ticket tiers

## Frontend Implementation

### TypeScript/JavaScript Example

```typescript
import { Program, AnchorProvider } from '@coral-xyz/anchor';
import { Connection, PublicKey } from '@solana/web3.js';
import { BN } from '@coral-xyz/anchor';
import IDL from './idl/moonbase.json'; // Your generated IDL

// Initialize connection and provider
const connection = new Connection('YOUR_RPC_URL', 'confirmed');
const provider = new AnchorProvider(connection, wallet, { commitment: 'confirmed' });
const program = new Program(IDL, provider);

/**
 * Simulate the cost of purchasing multiple Dragon Eggs
 * @param mintCount - Number of eggs to mint (1-10)
 * @returns Object with total price, individual prices, and ticket amounts
 */
async function simulatePurchaseCost(mintCount: number) {
  try {
    // Derive EggConfig PDA
    const [eggConfigPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from('egg-config')],
      program.programId
    );

    // Call simulate_purchase_cost using .simulate() method
    // This executes the instruction without sending a transaction
    const result = await program.methods
      .simulatePurchaseCost(new BN(mintCount))
      .accounts({
        eggConfig: eggConfigPDA,
      })
      .simulate();

    // Parse the return value
    const [totalPrice, individualPrices, ticketAmounts] = result.value;

    return {
      totalPrice: totalPrice.toNumber(), // Total cost in lamports
      totalPriceSOL: totalPrice.toNumber() / 1e9, // Total cost in SOL
      individualPrices: individualPrices.map((p: BN) => ({
        lamports: p.toNumber(),
        sol: p.toNumber() / 1e9
      })),
      ticketAmounts: ticketAmounts.map(([value, count]: [BN, BN]) => ({
        ticketValue: value.toNumber(), // Ticket value in lamports
        ticketValueSOL: value.toNumber() / 1e9, // Ticket value in SOL
        ticketCount: count.toNumber(), // Number of tickets
        totalTicketValue: value.toNumber() * count.toNumber(), // Total ticket value
        totalTicketValueSOL: (value.toNumber() * count.toNumber()) / 1e9
      }))
    };
  } catch (error) {
    console.error('Error simulating purchase cost:', error);
    throw error;
  }
}

// Usage example
async function displayMintingInfo() {
  const mintCount = 5; // User wants to mint 5 eggs
  
  const simulation = await simulatePurchaseCost(mintCount);
  
  console.log(`Total Cost: ${simulation.totalPriceSOL} SOL`);
  console.log(`Individual Prices:`);
  simulation.individualPrices.forEach((price, index) => {
    console.log(`  Egg ${index + 1}: ${price.sol} SOL`);
  });
  
  console.log(`\nTicket Rewards (1.5x SOL spent):`);
  simulation.ticketAmounts.forEach((tier, index) => {
    console.log(`  Tier ${index + 1}:`);
    console.log(`    Ticket Value: ${tier.ticketValueSOL} SOL`);
    console.log(`    Ticket Count: ${tier.ticketCount}`);
    console.log(`    Total Value: ${tier.totalTicketValueSOL} SOL`);
  });
}
```

### React Hook Example

```typescript
import { useConnection, useWallet } from '@solana/wallet-adapter-react';
import { useEffect, useState } from 'react';
import { Program, AnchorProvider, BN } from '@coral-xyz/anchor';
import { PublicKey } from '@solana/web3.js';
import IDL from './idl/moonbase.json';

interface SimulationResult {
  totalPrice: number;
  totalPriceSOL: number;
  individualPrices: Array<{ lamports: number; sol: number }>;
  ticketAmounts: Array<{
    ticketValue: number;
    ticketValueSOL: number;
    ticketCount: number;
    totalTicketValue: number;
    totalTicketValueSOL: number;
  }>;
}

export function useSimulatePurchaseCost(mintCount: number) {
  const { connection } = useConnection();
  const wallet = useWallet();
  const [result, setResult] = useState<SimulationResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<Error | null>(null);

  useEffect(() => {
    if (!wallet.publicKey || !mintCount || mintCount < 1 || mintCount > 10) {
      return;
    }

    async function simulate() {
      setLoading(true);
      setError(null);
      
      try {
        const provider = new AnchorProvider(
          connection,
          wallet as any,
          { commitment: 'confirmed' }
        );
        const program = new Program(IDL, provider);

        const [eggConfigPDA] = PublicKey.findProgramAddressSync(
          [Buffer.from('egg-config')],
          program.programId
        );

        const simulationResult = await program.methods
          .simulatePurchaseCost(new BN(mintCount))
          .accounts({
            eggConfig: eggConfigPDA,
          })
          .simulate();

        const [totalPrice, individualPrices, ticketAmounts] = simulationResult.value;

        setResult({
          totalPrice: totalPrice.toNumber(),
          totalPriceSOL: totalPrice.toNumber() / 1e9,
          individualPrices: individualPrices.map((p: BN) => ({
            lamports: p.toNumber(),
            sol: p.toNumber() / 1e9
          })),
          ticketAmounts: ticketAmounts.map(([value, count]: [BN, BN]) => ({
            ticketValue: value.toNumber(),
            ticketValueSOL: value.toNumber() / 1e9,
            ticketCount: count.toNumber(),
            totalTicketValue: value.toNumber() * count.toNumber(),
            totalTicketValueSOL: (value.toNumber() * count.toNumber()) / 1e9
          }))
        });
      } catch (err) {
        setError(err as Error);
      } finally {
        setLoading(false);
      }
    }

    simulate();
  }, [connection, wallet, mintCount]);

  return { result, loading, error };
}

// Component usage
function MintingInterface() {
  const [mintCount, setMintCount] = useState(1);
  const { result, loading, error } = useSimulatePurchaseCost(mintCount);

  return (
    <div>
      <input
        type="number"
        min="1"
        max="10"
        value={mintCount}
        onChange={(e) => setMintCount(parseInt(e.target.value))}
      />
      
      {loading && <p>Calculating costs...</p>}
      {error && <p>Error: {error.message}</p>}
      {result && (
        <div>
          <h3>Total Cost: {result.totalPriceSOL.toFixed(4)} SOL</h3>
          <h4>Individual Prices:</h4>
          <ul>
            {result.individualPrices.map((price, i) => (
              <li key={i}>Egg {i + 1}: {price.sol.toFixed(4)} SOL</li>
            ))}
          </ul>
          <h4>Ticket Rewards:</h4>
          {result.ticketAmounts.map((tier, i) => (
            <div key={i}>
              <p>Tier {i + 1}: {tier.ticketCount} tickets × {tier.ticketValueSOL} SOL</p>
              <p>Total Value: {tier.totalTicketValueSOL.toFixed(4)} SOL</p>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
```

## Important Notes

1. **No Transaction Required**: The `.simulate()` method executes the instruction without sending a transaction, so no SOL is spent.

2. **Account Derivation**: The `eggConfig` PDA is derived using the seed `"egg-config"` and the program ID.

3. **Return Value Parsing**: Anchor returns BN (BigNumber) objects, so you need to convert them to numbers using `.toNumber()`.

4. **Error Handling**: The function will throw errors if:
   - `mint_count` is 0 or > 10
   - `eggs_minted + mint_count > max_supply`
   - `ticket_tiers.length != 3`

5. **Ticket Calculation**: Tickets are calculated as `(total_price / ticket_value) * 1.5`, giving users tickets worth 1.5x the SOL they spent.

## Logic Verification

The function logic is correct:
- ✅ Calculates individual prices using bonding curve (`compute_gene_price`)
- ✅ Sums prices correctly with overflow checks
- ✅ Calculates ticket amounts using `calc_tickets_count` helper: `((total_price * 150) / ticket_value) / 100`
- ✅ Returns all necessary data for frontend display

