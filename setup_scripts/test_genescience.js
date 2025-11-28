/**
 * GeneScience JavaScript Implementation
 * 
 * DNA STRUCTURE (256 bits / 32 bytes):
 * 1. Faction/Family (4 bits): Maps to Faction ID (0-11)
 * 2. Evolution Stage (3 bits): Current level (0-7)
 * 3. Appearance Genes (105 bits): 7 groups × 3 traits × 5 bits (0-31)
 * 4. Combat Genes (60 bits): 5 groups × 3 traits × 4 bits (0-15)
 * 5. Reserved (84 bits): Future use
 */

const { keccak_256 } = require("js-sha3");
const { PublicKey } = require("@solana/web3.js");

// ============================================================================
// CONSTANTS
// ============================================================================

const FACTION_TYPE_BITS = 4;
const EVOLUTION_STAGE_BITS = 3;
const APPEARANCE_TRAIT_BITS = 5;  // 0-31 values
const POWER_TRAIT_BITS = 4;       // 0-15 values

const APPEARANCE_OFFSET = FACTION_TYPE_BITS + EVOLUTION_STAGE_BITS; // 7
const COMBAT_OFFSET = APPEARANCE_OFFSET + (21 * APPEARANCE_TRAIT_BITS); // 7 + 105 = 112

const APPEARANCE_TOTAL_TRAITS = 21;  // 7 groups × 3
const COMBAT_TOTAL_TRAITS = 15;      // 5 groups × 3

const APPEARANCE_MAX = 31;
const COMBAT_MAX = 15;

const APPEARANCE_GROUPS = 7;
const COMBAT_GROUPS = 5;
const TRAITS_PER_GROUP = 3;

// ============================================================================
// BIT MANIPULATION HELPERS
// ============================================================================

/**
 * Get a trait value from DNA at specified position
 * @param {Uint8Array} dna - 32-byte DNA array
 * @param {number} baseOffset - Starting bit offset
 * @param {number} traitBits - Number of bits per trait
 * @param {number} index - Trait index
 * @returns {number} Trait value
 */
function getTraitValue(dna, baseOffset, traitBits, index) {
    const startBit = baseOffset + (index * traitBits);
    const byteIdx = Math.floor(startBit / 8);
    const bitIdx = startBit % 8;
    
    if (byteIdx >= 32) return 0;
    
    let val = 0;
    let remaining = traitBits;
    let currByte = byteIdx;
    let currBit = bitIdx;
    let processed = 0;
    
    while (remaining > 0 && currByte < 32) {
        const bitsInByte = 8 - currBit;
        const take = Math.min(remaining, bitsInByte);
        const mask = ((1 << take) - 1) << currBit;
        const bits = (dna[currByte] & mask) >> currBit;
        val |= bits << processed;
        processed += take;
        remaining -= take;
        currByte++;
        currBit = 0;
    }
    
    return val;
}

/**
 * Set a trait value in DNA at specified position
 * @param {Uint8Array} dna - 32-byte DNA array (mutated)
 * @param {number} baseOffset - Starting bit offset
 * @param {number} traitBits - Number of bits per trait
 * @param {number} index - Trait index
 * @param {number} value - Value to set
 */
function setTraitValue(dna, baseOffset, traitBits, index, value) {
    const startBit = baseOffset + (index * traitBits);
    const byteIdx = Math.floor(startBit / 8);
    const bitIdx = startBit % 8;
    
    if (byteIdx >= 32) return;
    
    let remaining = traitBits;
    let currByte = byteIdx;
    let currBit = bitIdx;
    let valProcessed = 0;
    
    while (remaining > 0 && currByte < 32) {
        const bitsInByte = 8 - currBit;
        const set = Math.min(remaining, bitsInByte);
        const mask = ((1 << set) - 1) << currBit;
        const chunk = (value >> valProcessed) & ((1 << set) - 1);
        dna[currByte] = (dna[currByte] & ~mask) | (chunk << currBit);
        remaining -= set;
        valProcessed += set;
        currByte++;
        currBit = 0;
    }
}

// ============================================================================
// DNA DECODER FUNCTIONS
// ============================================================================

/**
 * Get faction/family type from DNA (first 4 bits)
 */
function getFamilyType(dna) {
    return dna[0] & 0x0F;
}

/**
 * Get evolution stage from DNA (bits 4-6)
 */
function getEvolutionStage(dna) {
    return (dna[0] >> 4) & 0x07;
}

/**
 * Decode all 21 appearance traits
 * @returns {number[]} Array of 21 trait values (0-31)
 */
function decodeAppearanceTraits(dna) {
    const traits = [];
    for (let i = 0; i < APPEARANCE_TOTAL_TRAITS; i++) {
        traits.push(getTraitValue(dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i));
    }
    return traits;
}

/**
 * Decode dominant appearance traits (first from each of 7 groups)
 * @returns {number[]} Array of 7 dominant trait values
 */
function decodeDominantAppearanceTraits(dna) {
    const traits = [];
    for (let i = 0; i < APPEARANCE_GROUPS; i++) {
        traits.push(getTraitValue(dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i * TRAITS_PER_GROUP));
    }
    return traits;
}

/**
 * Decode all 15 power/combat traits
 * @returns {number[]} Array of 15 trait values (0-15)
 */
function decodePowerTraits(dna) {
    const traits = [];
    for (let i = 0; i < COMBAT_TOTAL_TRAITS; i++) {
        traits.push(getTraitValue(dna, COMBAT_OFFSET, POWER_TRAIT_BITS, i));
    }
    return traits;
}

/**
 * Decode dominant power traits (first from each of 5 groups)
 * @returns {number[]} Array of 5 dominant trait values
 */
function decodeDominantPowerTraits(dna) {
    const traits = [];
    for (let i = 0; i < COMBAT_GROUPS; i++) {
        traits.push(getTraitValue(dna, COMBAT_OFFSET, POWER_TRAIT_BITS, i * TRAITS_PER_GROUP));
    }
    return traits;
}

/**
 * Get appearance traits grouped (7 groups of 3 traits each)
 */
function getAppearanceTraitsGrouped(dna) {
    const all = decodeAppearanceTraits(dna);
    const grouped = [];
    for (let g = 0; g < APPEARANCE_GROUPS; g++) {
        grouped.push({
            group: g,
            dominant: all[g * 3],
            recessive: all[g * 3 + 1],
            minorRecessive: all[g * 3 + 2]
        });
    }
    return grouped;
}

/**
 * Get power traits grouped (5 groups of 3 traits each)
 */
function getPowerTraitsGrouped(dna) {
    const all = decodePowerTraits(dna);
    const grouped = [];
    for (let g = 0; g < COMBAT_GROUPS; g++) {
        grouped.push({
            group: g,
            dominant: all[g * 3],
            recessive: all[g * 3 + 1],
            minorRecessive: all[g * 3 + 2]
        });
    }
    return grouped;
}

// ============================================================================
// DNA GENERATION (for testing)
// ============================================================================

/**
 * Generate genesis DNA (mimics Rust implementation)
 */
function generateGenesisDna(mintNumber, minterPubkey, slot, factionId) {
    // Create seed data
    const seedData = Buffer.alloc(8 + 32 + 8);
    seedData.writeBigUInt64LE(BigInt(mintNumber), 0);
    minterPubkey.toBuffer().copy(seedData, 8);
    seedData.writeBigUInt64LE(BigInt(slot), 40);
    
    // Hash to get DNA
    const hashHex = keccak_256(seedData);
    const dna = new Uint8Array(Buffer.from(hashHex, 'hex'));
    
    // Encode faction (first 4 bits)
    dna[0] = (dna[0] & 0xF0) | (factionId & 0x0F);
    // Evolution stage 0 (bits 4-6)
    dna[0] = dna[0] & 0x8F;
    
    // Limit trait ranges for genesis (weaker initial stats)
    limitGenesisTraitRanges(dna);
    
    return dna;
}

/**
 * Limit trait ranges for genesis DNA (weaker initial stats)
 * - Appearance: 0-9 (less than 10)
 * - First 3 power groups (9 traits): 0-4 (less than 5)
 * - Last 2 power groups (6 traits): 0 (locked)
 */
function limitGenesisTraitRanges(dna) {
    // Appearance traits: 0-9
    for (let i = 0; i < APPEARANCE_TOTAL_TRAITS; i++) {
        const val = getTraitValue(dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i);
        setTraitValue(dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i, val % 10);
    }
    
    // Power traits:
    // - First 3 groups (9 traits, indices 0-8): 0-4
    // - Last 2 groups (6 traits, indices 9-14): 0
    for (let i = 0; i < COMBAT_TOTAL_TRAITS; i++) {
        const val = i < 9 
            ? getTraitValue(dna, COMBAT_OFFSET, POWER_TRAIT_BITS, i) % 5
            : 0;
        setTraitValue(dna, COMBAT_OFFSET, POWER_TRAIT_BITS, i, val);
    }
}

/**
 * Limit trait ranges for general use (full ranges)
 */
function limitTraitRanges(dna) {
    for (let i = 0; i < APPEARANCE_TOTAL_TRAITS; i++) {
        const val = getTraitValue(dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i);
        setTraitValue(dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i, val % 32);
    }
    for (let i = 0; i < COMBAT_TOTAL_TRAITS; i++) {
        const val = getTraitValue(dna, COMBAT_OFFSET, POWER_TRAIT_BITS, i);
        setTraitValue(dna, COMBAT_OFFSET, POWER_TRAIT_BITS, i, val % 16);
    }
}

// ============================================================================
// FULL DNA ANALYSIS
// ============================================================================

/**
 * Fully decode and analyze DNA
 */
function analyzeDna(dna) {
    return {
        factionId: getFamilyType(dna),
        evolutionStage: getEvolutionStage(dna),
        appearance: {
            all: decodeAppearanceTraits(dna),
            dominant: decodeDominantAppearanceTraits(dna),
            grouped: getAppearanceTraitsGrouped(dna)
        },
        power: {
            all: decodePowerTraits(dna),
            dominant: decodeDominantPowerTraits(dna),
            grouped: getPowerTraitsGrouped(dna)
        },
        raw: Buffer.from(dna).toString('hex')
    };
}

/**
 * Pretty print DNA analysis
 */
function printDnaAnalysis(dna, label = "DNA") {
    const analysis = analyzeDna(dna);
    
    console.log(`\n=== ${label} ===`);
    console.log(`Faction: ${analysis.factionId}`);
    console.log(`Evolution Stage: ${analysis.evolutionStage}/7`);
    console.log(`\nAppearance Traits (7 groups × 3):`);
    analysis.appearance.grouped.forEach(g => {
        console.log(`  Group ${g.group}: Dom=${g.dominant}, Rec=${g.recessive}, MinRec=${g.minorRecessive}`);
    });
    console.log(`  Dominant Summary: [${analysis.appearance.dominant.join(', ')}]`);
    
    console.log(`\nPower Traits (5 groups × 3):`);
    analysis.power.grouped.forEach(g => {
        console.log(`  Group ${g.group}: Dom=${g.dominant}, Rec=${g.recessive}, MinRec=${g.minorRecessive}`);
    });
    console.log(`  Dominant Summary: [${analysis.power.dominant.join(', ')}]`);
    
    console.log(`\nRaw DNA: ${analysis.raw}`);
}

// ============================================================================
// TESTS
// ============================================================================

function runTests() {
    console.log("🧬 Running GeneScience Tests...\n");
    
    const testPubkey = new PublicKey(new Uint8Array(32).fill(1));
    let passed = 0;
    let failed = 0;
    
    function test(name, condition) {
        if (condition) {
            console.log(`✅ ${name}`);
            passed++;
        } else {
            console.log(`❌ ${name}`);
            failed++;
        }
    }
    
    // --- DNA Generation Tests ---
    console.log("\n--- DNA Generation ---");
    
    const dna1 = generateGenesisDna(1, testPubkey, 12345, 5);
    test("Genesis DNA has correct faction", getFamilyType(dna1) === 5);
    test("Genesis DNA has stage 0", getEvolutionStage(dna1) === 0);
    
    // Test all factions
    let allFactionsPassed = true;
    for (let f = 0; f < 12; f++) {
        const dna = generateGenesisDna(f, testPubkey, 100, f);
        if (getFamilyType(dna) !== f) allFactionsPassed = false;
    }
    test("All factions encode correctly", allFactionsPassed);
    
    // Deterministic
    const dna2a = generateGenesisDna(100, testPubkey, 999, 3);
    const dna2b = generateGenesisDna(100, testPubkey, 999, 3);
    test("DNA generation is deterministic", Buffer.from(dna2a).equals(Buffer.from(dna2b)));
    
    // Different inputs = different DNA
    const dna3a = generateGenesisDna(1, testPubkey, 100, 0);
    const dna3b = generateGenesisDna(2, testPubkey, 100, 0);
    test("Different inputs produce different DNA", !Buffer.from(dna3a).equals(Buffer.from(dna3b)));
    
    // --- Trait Decoding Tests ---
    console.log("\n--- Trait Decoding ---");
    
    const dna4 = generateGenesisDna(1, testPubkey, 100, 0);
    const appTraits = decodeAppearanceTraits(dna4);
    const pwrTraits = decodePowerTraits(dna4);
    
    test("21 appearance traits decoded", appTraits.length === 21);
    test("15 power traits decoded", pwrTraits.length === 15);
    
    // Genesis constraints: appearance < 10, first 9 power < 5, last 6 power == 0
    let appGenesis = appTraits.every(t => t >= 0 && t < 10);
    let pwrFirst9 = pwrTraits.slice(0, 9).every(t => t >= 0 && t < 5);
    let pwrLast6 = pwrTraits.slice(9, 15).every(t => t === 0);
    test("Appearance traits < 10 (genesis)", appGenesis);
    test("First 9 power traits < 5 (genesis)", pwrFirst9);
    test("Last 6 power traits == 0 (genesis)", pwrLast6);
    
    // Dominant traits
    const domApp = decodeDominantAppearanceTraits(dna4);
    const domPwr = decodeDominantPowerTraits(dna4);
    test("7 dominant appearance traits", domApp.length === 7);
    test("5 dominant power traits", domPwr.length === 5);
    
    let domAppMatch = domApp.every((v, i) => v === appTraits[i * 3]);
    let domPwrMatch = domPwr.every((v, i) => v === pwrTraits[i * 3]);
    test("Dominant appearance matches all[i*3]", domAppMatch);
    test("Dominant power matches all[i*3]", domPwrMatch);
    
    // --- Bit Manipulation Tests ---
    console.log("\n--- Bit Manipulation ---");
    
    const testDna = new Uint8Array(32);
    
    // Test set/get roundtrip for appearance
    let appRoundtrip = true;
    for (let i = 0; i < 21; i++) {
        const val = (i * 3) % 32;
        setTraitValue(testDna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i, val);
        const got = getTraitValue(testDna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i);
        if (got !== val) appRoundtrip = false;
    }
    test("Appearance trait set/get roundtrip", appRoundtrip);
    
    // Test set/get roundtrip for power
    const testDna2 = new Uint8Array(32);
    let pwrRoundtrip = true;
    for (let i = 0; i < 15; i++) {
        const val = (i * 2) % 16;
        setTraitValue(testDna2, COMBAT_OFFSET, POWER_TRAIT_BITS, i, val);
        const got = getTraitValue(testDna2, COMBAT_OFFSET, POWER_TRAIT_BITS, i);
        if (got !== val) pwrRoundtrip = false;
    }
    test("Power trait set/get roundtrip", pwrRoundtrip);
    
    // Trait isolation
    const testDna3 = new Uint8Array(32);
    setTraitValue(testDna3, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, 5, 25);
    let isolated = true;
    for (let i = 0; i < 21; i++) {
        const val = getTraitValue(testDna3, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i);
        if (i === 5) {
            if (val !== 25) isolated = false;
        } else {
            if (val !== 0) isolated = false;
        }
    }
    test("Trait isolation (setting one doesn't affect others)", isolated);
    
    // --- Summary ---
    console.log(`\n${"=".repeat(40)}`);
    console.log(`Tests: ${passed + failed} | Passed: ${passed} | Failed: ${failed}`);
    console.log(`${"=".repeat(40)}\n`);
    
    // --- Example DNA Analysis ---
    console.log("\n--- Example DNA Analysis ---");
    const exampleDna = generateGenesisDna(42, testPubkey, 999999, 7);
    printDnaAnalysis(exampleDna, "Example Doge #42 (Faction 7)");
    
    return failed === 0;
}

// ============================================================================
// EXPORTS
// ============================================================================

module.exports = {
    // Constants
    APPEARANCE_OFFSET,
    COMBAT_OFFSET,
    APPEARANCE_TRAIT_BITS,
    POWER_TRAIT_BITS,
    APPEARANCE_TOTAL_TRAITS,
    COMBAT_TOTAL_TRAITS,
    APPEARANCE_MAX,
    COMBAT_MAX,
    
    // Core functions
    getTraitValue,
    setTraitValue,
    getFamilyType,
    getEvolutionStage,
    
    // Decoding
    decodeAppearanceTraits,
    decodeDominantAppearanceTraits,
    decodePowerTraits,
    decodeDominantPowerTraits,
    getAppearanceTraitsGrouped,
    getPowerTraitsGrouped,
    
    // Generation
    generateGenesisDna,
    limitGenesisTraitRanges,
    limitTraitRanges,
    
    // Analysis
    analyzeDna,
    printDnaAnalysis,
    
    // Tests
    runTests,
};

// Run tests if executed directly
if (require.main === module) {
    const success = runTests();
    process.exit(success ? 0 : 1);
}

