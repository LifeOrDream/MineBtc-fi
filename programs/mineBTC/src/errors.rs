use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("The authority is invalid")]
    InvalidAuthority,

    #[msg("Invalid referral account")]
    InvalidReferralAccount,

    #[msg("User not authorized to perform this action")]
    Unauthorized,
    #[msg("No pending authority transfer to accept")]
    NoPendingAuthority,

    #[msg("Invalid mint")]
    InvalidMint,

    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,

    #[msg("Mining already initialized")]
    MiningAlreadyInitialized,

    #[msg("Referral pubkey cannot be the same as owner")]
    ReferralCannotBeSameAsOwner,

    #[msg("Referral rewards account required when referral code is provided")]
    ReferralRewardsAccountRequired,

    #[msg("Invalid parameters provided for operation")]
    InvalidParameters,

    #[msg("Game is paused — new bets, autominer execution, round starts, and hashbeast mints are disabled. Claims and round settlement still work.")]
    GamePaused,

    #[msg("Amount overflow")]
    AmountOverflow,

    #[msg("Mining not initialized yet")]
    MiningNotInitialized,

    #[msg("Token vault not initialized")]
    TokenVaultNotInitialized,

    #[msg("Insufficient tokens in vault")]
    InsufficientTokensInVault,

    // ========== FACTION-RELATED ERRORS ========== //
    #[msg("Invalid faction name: must be 1-16 characters")]
    InvalidFactionName,

    #[msg("Maximum number of factions reached (12 max)")]
    MaxFactionsReached,

    #[msg("Faction with this name already exists")]
    FactionAlreadyExists,

    #[msg("Invalid faction ID: faction does not exist")]
    InvalidFactionId,

    #[msg("Metaplex Core program ID mismatch")]
    InvalidMplCoreProgram,

    #[msg("Invalid account provided")]
    InvalidAccount,

    #[msg("Invalid metadata provided")]
    InvalidMetadata,

    #[msg("URI too long - maximum 200 characters")]
    UriTooLong,

    #[msg("HashBeast already at guard")]
    HashBeastAlreadyAtGuard,

    #[msg("HashBeast is not incubated in this degenBTC program")]
    HashBeastNotAtGuard,

    #[msg("HashBeast limit for this tier has been reached")]
    HashBeastLimitExceeded,

    #[msg("NFT is not owned by the user")]
    NftNotOwnedByUser,

    #[msg("Update dist rate first")]
    UpdateDistRateFirst,

    #[msg("degenBtc needed for POl cannot be more than 5% of vault balance")]
    MaxLimitError,

    // ========== ROUND GAME ERRORS ========== //
    #[msg("Round has already ended")]
    RoundEnded,

    #[msg("Round has not ended yet")]
    RoundNotEnded,

    #[msg("Invalid round ID")]
    InvalidRound,

    #[msg("No factions provided")]
    NoFactions,

    #[msg("No bets placed in this round")]
    NoBets,

    #[msg("Faction not found")]
    FactionNotFound,

    #[msg("No rounds remaining in autominer")]
    NoRoundsRemaining,

    #[msg("Invalid owner")]
    InvalidOwner,

    #[msg("Invalid amount")]
    InvalidAmount,

    #[msg("Minimum SOL bet per country-direction position is 0.0001 SOL")]
    BetBelowMinimum,

    #[msg("Insufficient funds")]
    InsufficientFunds,

    #[msg("Invalid init type")]
    InvalidInitType,

    #[msg("Invalid state for this operation")]
    InvalidState,

    #[msg("Invalid program ID")]
    InvalidProgramId,

    // ========== ROYALTY MANAGEMENT ERRORS ========== //
    #[msg("No creators specified")]
    NoCreators,

    #[msg("Sum of creator percentages must be 100")]
    InvalidCreatorShare,

    #[msg("Royalties plugin not found on collection")]
    RoyaltiesPluginMissing,

    #[msg("Royalties rule_set is not ProgramDenyList")]
    UnexpectedRuleSetVariant,

    #[msg("Invalid stage")]
    InvalidStage,

    #[msg("Cannot begin round")]
    CannotBeginRound,

    #[msg("Position not unlocked")]
    PositionNotLocked,

    // ========== BREEDING ERRORS ========== //
    #[msg("Breeding is not currently allowed")]
    BreedingNotAllowed,

    #[msg("Maximum breed count reached for this hashbeast")]
    MaxBreedCountReached,

    #[msg("Breeding cooldown has not ended yet")]
    CooldownNotEnded,

    #[msg("Maximum evolution stage reached")]
    MaxEvolutionReached,

    #[msg("Maximum rebirth count reached for this hashbeast")]
    MaxRebirthCountReached,

    #[msg("HashBeast metadata not found")]
    HashBeastMetadataNotFound,

    #[msg("HashBeasts must be at the same rebirth generation to breed")]
    RebirthLevelMismatch,

    #[msg("Invalid breeding pair")]
    InvalidBreedingPair,

    #[msg("Breeding floor anchor is unavailable or too low")]
    BreedFloorAnchorUnavailable,

    #[msg("dbTC price is unavailable for breeding")]
    DbtcPriceUnavailable,

    #[msg("Genesis HashBeast mint sale must be sold out before breeding")]
    GenesisNotSoldOut,

    #[msg("Position already exists")]
    PositionAlreadyExists,

    #[msg("HashBeast DNA mismatch")]
    ClaimPendingRoundRewards,

    #[msg("Minting not allowed")]
    MintingNotAllowed,

    #[msg("Gameplay locking is only available while RPG progression is enabled")]
    GameplayNotEnabled,

    #[msg("Gameplay unlock has already been requested for this hashbeast")]
    GameplayUnlockAlreadyRequested,

    #[msg("Gameplay unlock has not been requested")]
    GameplayUnlockNotRequested,

    #[msg("Gameplay hashbeast can only be unlocked after the next faction_war cycle begins")]
    GameplayUnlockNotReady,

    #[msg("Claim all pending round and faction-war reward accounts before unlocking this gameplay hashbeast")]
    GameplayRewardsPending,

    // ========== FACTION_WAR MINING ERRORS ========== //
    #[msg("FactionWar is not currently active")]
    FactionWarNotActive,

    #[msg("FactionWar has not ended yet")]
    FactionWarNotEnded,

    #[msg("FactionWar has not been settled yet")]
    FactionWarNotSettled,

    #[msg("FactionWar has already been settled")]
    FactionWarAlreadySettled,

    #[msg("FactionWar rewards have already been claimed")]
    FactionWarRewardsAlreadyClaimed,

    #[msg("Round is pending faction-reward finalization; settle cannot run between end_round and settle_round")]
    RoundFinalizationPending,

    #[msg("Cycle has reached its final round; war must be settled before a new round can start")]
    CycleAwaitingSettlement,

    #[msg("Ticket-backed bets exceed the session cap")]
    TicketBetCapExceeded,

    #[msg("Round entropy is not ready yet")]
    RoundEntropyNotReady,

    #[msg("Free HashBeast mint allowance exceeds the per-user maximum")]
    MaxFreeHashBeastMintsExceeded,

    #[msg("No free HashBeast mints remaining for this user")]
    NoFreeHashBeastMintsRemaining,

    // ============================ Inventory / Lootbox / Market ============================
    #[msg("Inventory pool is at MAX_INVENTORY capacity")]
    InventoryFull,

    #[msg("Reborn entry is in an invalid status for this operation")]
    InvalidRebornStatus,

    #[msg("Marketplace program account does not match cached pubkey")]
    InvalidMarketplaceProgram,

    #[msg("Marketplace config account does not match cached pubkey")]
    InvalidMarketplaceConfig,

    #[msg("Inventory listing price is below the marketplace minimum")]
    ListingPriceTooLow,

    #[msg("Listing price exceeds the buyer's max price")]
    ListingPriceExceedsMax,

    #[msg("Inventory PDA does not own this asset")]
    AssetNotInInventory,

    // -------- Permissionless market making --------
    #[msg("Floor queue has no entries")]
    FloorQueueEmpty,

    #[msg("Floor queue is full and the new entry is not cheaper than the worst entry")]
    FloorQueueFull,

    #[msg("Asset is already registered in the floor queue")]
    AssetAlreadyInQueue,

    #[msg("Cached floor entry is stale and was popped; nothing to sweep this tx")]
    NoLiveFloorEntries,

    #[msg("Listing price exceeds the attractive ceiling vs current anchor")]
    FloorPriceTooHigh,

    #[msg("Sweep would drop the vault below MIN_SWEEP_RESERVE_LAMPORTS")]
    SweepVaultBelowReserve,

    #[msg("Sweep tx exceeds the per-tx sweep cap")]
    SweepTxCapExceeded,

    #[msg("Sweep anchor is below the minimum sweep threshold (no recent volume)")]
    SweepAnchorTooLow,

    #[msg("Floor entry data does not match the live marketplace listing")]
    StaleFloorEntry,

    #[msg("Program-owned listings cannot be registered in the floor queue")]
    ProgramListingNotAllowed,

    #[msg("Listing has not yet aged enough for expire_program_listing")]
    ListingNotYetExpirable,

    #[msg("This ix only operates on program-owned listings")]
    NotProgramListing,

    #[msg("This ix only operates on user-owned listings")]
    NotUserListing,

    #[msg("Floor snapshot was already recorded within the cadence window")]
    SnapshotTooSoon,

    #[msg("Asset is still owned by inventory_pda — sale not actually settled")]
    AssetStillOwnedByInventory,

    #[msg("Listing is not present in the floor queue")]
    ListingNotInQueue,
}
