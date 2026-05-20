#!/usr/bin/env node

import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const config = JSON.parse(fs.readFileSync(path.join(__dirname, "config.json"), "utf8"));

/**
 * HashBeast Minting Simulation Script
 * Simulates the cost of minting all hashbeasts using the bonding curve pricing formula.
 */

// ============================================================================
// CONFIGURATION - Edit these values
// ============================================================================

const BASE_PRICE = config.hashbeasts_config.base_price;
const CURVE_A = config.hashbeasts_config.curve_a;
const TOTAL_SUPPLY = config.hashbeasts_config.genesis_mint_limit;
const SOL_PRICE_USD = Number(process.env.SOL_PRICE_USD ?? 90);

// ============================================================================
// PRICING CALCULATION (matches Rust compute_gene_price)
// ============================================================================

/**
 * Calculate cube root using binary search
 * Used to approximate x^(2/3) = cube_root(x^2)
 */
function cubeRootBinarySearch(squared) {
    let low = 1n;
    let high = squared < 1_000_000_000_000_000_000n ? squared : 1_000_000_000_000_000_000n;
    let result = 0n;
    
    while (low <= high) {
        const mid = (low + high) / 2n;
        const cube = mid * mid * mid;
        
        if (cube <= squared) {
            result = mid;
            low = mid + 1n;
        } else {
            if (mid === 0n) break;
            high = mid - 1n;
        }
    }
    
    return result;
}

/**
 * Calculate gene price using bonding curve formula
 * Formula: price = base_price + curve_a * (items_minted^(2/3))
 * 
 * @param {number} basePrice - Base price in lamports
 * @param {number} curveA - Curve steepness parameter
 * @param {number} itemsMinted - Number of items already minted (0-indexed)
 * @returns {number} Price in lamports
 */
function computeGenePrice(basePrice, curveA, itemsMinted) {
    if (itemsMinted === 0) {
        return basePrice;
    }
    
    // Convert to BigInt for precision
    const itemsMintedBig = BigInt(itemsMinted);
    
    // Calculate x^2
    const squared = itemsMintedBig * itemsMintedBig;
    
    // Find cube root of x^2 (this gives us x^(2/3))
    const exponentComponent = cubeRootBinarySearch(squared);
    
    // Cap to u64::MAX
    const exponentComponentU64 = exponentComponent > BigInt(Number.MAX_SAFE_INTEGER) 
        ? Number.MAX_SAFE_INTEGER 
        : Number(exponentComponent);
    
    // Multiply by curve_a
    const exponentPriceFactor = curveA * exponentComponentU64;
    
    // Final price = base_price + exponential component
    const mintPrice = basePrice + exponentPriceFactor;
    
    return mintPrice;
}

// ============================================================================
// PROCEEDS CALCULATION
// ============================================================================

/**
 * Calculate proceeds for a single mint.
 * Genesis mint proceeds route to the configured fee_recipient after any referral cut.
 * This simulation assumes no referral.
 * @param {number} totalPrice - Total price in lamports
 * @returns {{feeRecipient: number}} Proceeds in lamports
 */
function calculateProceeds(totalPrice) {
    return {
        feeRecipient: totalPrice
    };
}

// ============================================================================
// FORMATTING HELPERS
// ============================================================================

/**
 * Convert lamports to SOL
 */
function lamportsToSol(lamports) {
    return lamports / 1_000_000_000;
}

/**
 * Convert SOL to USD
 */
function solToUsd(sol) {
    return sol * SOL_PRICE_USD;
}

/**
 * Format number with commas
 */
function formatNumber(num) {
    return num.toLocaleString('en-US', { 
        minimumFractionDigits: 0, 
        maximumFractionDigits: 2 
    });
}

/**
 * Format SOL amount
 */
function formatSol(lamports) {
    const sol = lamportsToSol(lamports);
    return `${formatNumber(sol)} SOL`;
}

/**
 * Format USD amount
 */
function formatUsd(lamports) {
    const sol = lamportsToSol(lamports);
    const usd = solToUsd(sol);
    return `$${formatNumber(usd)}`;
}

// ============================================================================
// MAIN SIMULATION
// ============================================================================

function simulateHashBeastMints() {
    console.log("=".repeat(80));
    console.log("HASHBEAST MINTING SIMULATION");
    console.log("=".repeat(80));
    console.log(`Base Price: ${formatSol(BASE_PRICE)}`);
    console.log(`Curve A: ${CURVE_A.toLocaleString()}`);
    console.log(`Total Supply: ${TOTAL_SUPPLY.toLocaleString()} hashbeasts`);
    console.log(`SOL Price: $${SOL_PRICE_USD.toLocaleString()}`);
    console.log("Proceeds: 100% fee_recipient in this no-referral simulation");
    console.log("=".repeat(80));
    console.log();
    
    let totalFeeRecipient = 0;
    let totalPrice = 0;
    
    // Track every Nth hashbeast (for display)
    const displayInterval = Math.max(1, Math.floor(TOTAL_SUPPLY / 50)); // Show ~50 hashbeasts
    
    for (let hashbeastNumber = 1; hashbeastNumber <= TOTAL_SUPPLY; hashbeastNumber++) {
        // items_minted is 0-indexed (0 = first hashbeast, 1 = second hashbeast, etc.)
        const itemsMinted = hashbeastNumber - 1;
        
        // Calculate price for this hashbeast
        const price = computeGenePrice(BASE_PRICE, CURVE_A, itemsMinted);
        
        // Calculate proceeds
        const proceeds = calculateProceeds(price);
        
        // Accumulate totals
        totalPrice += price;
        totalFeeRecipient += proceeds.feeRecipient;
        
        // Display every Nth hashbeast or last 10 hashbeasts
        const shouldDisplay = (hashbeastNumber % displayInterval === 0) || 
                             (hashbeastNumber > TOTAL_SUPPLY - 10) ||
                             hashbeastNumber === 1 ||
                             hashbeastNumber === TOTAL_SUPPLY;
        
        if (shouldDisplay) {
            console.log(
                `HashBeast #${hashbeastNumber.toString().padStart(5)} ==> ${formatSol(price).padStart(15)} (${formatUsd(price).padStart(10)}) :: ` +
                `${formatSol(proceeds.feeRecipient).padStart(12)} (${formatUsd(proceeds.feeRecipient).padStart(10)}) FEE_RECIPIENT`
            );
        }
    }
    
    console.log();
    console.log("=".repeat(80));
    console.log("FINAL TOTALS");
    console.log("=".repeat(80));
    console.log(`Total Revenue:     ${formatSol(totalPrice).padStart(20)} (${formatUsd(totalPrice).padStart(15)})`);
    console.log(`Fee Recipient:     ${formatSol(totalFeeRecipient).padStart(20)} (${formatUsd(totalFeeRecipient).padStart(15)})`);
    console.log("=".repeat(80));
    
    // Summary statistics
    console.log();
    console.log("SUMMARY STATISTICS");
    console.log("-".repeat(80));
    const avgPrice = totalPrice / TOTAL_SUPPLY;
    const firstHashBeastPrice = computeGenePrice(BASE_PRICE, CURVE_A, 0);
    const lastHashBeastPrice = computeGenePrice(BASE_PRICE, CURVE_A, TOTAL_SUPPLY - 1);
    
    console.log(`Average Price per HashBeast: ${formatSol(avgPrice)} (${formatUsd(avgPrice)})`);
    console.log(`First HashBeast Price:       ${formatSol(firstHashBeastPrice)} (${formatUsd(firstHashBeastPrice)})`);
    console.log(`Last HashBeast Price:        ${formatSol(lastHashBeastPrice)} (${formatUsd(lastHashBeastPrice)})`);
    console.log(`Price Increase:        ${formatNumber((lastHashBeastPrice / firstHashBeastPrice - 1) * 100)}%`);
    console.log("-".repeat(80));
}

// Run simulation
simulateHashBeastMints();
