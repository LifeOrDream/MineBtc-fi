use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("The authority is invalid")]
    InvalidAuthority,

    #[msg("Invalid referral account")]
    InvalidReferralAccount,

    #[msg("User not authorized to perform this action")]
    Unauthorized,
    #[msg("Permissionless reward claims are disabled for this player")]
    PermissionlessRewardClaimsDisabled,
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

    #[msg("Maximum referrals reached for this referral code (50 max)")]
    MaxReferralsReached,

    #[msg("Invalid parameters provided for operation")]
    InvalidParameters,

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

    #[msg("Maximum number of factions reached (15 max)")]
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

    #[msg("Doge already at guard")]
    DogeAlreadyAtGuard,

    #[msg("Doge is not incubated in this minebtc")]
    DogeNotAtGuard,

    #[msg("Doge limit for this tier has been reached")]
    DogeLimitExceeded,

    #[msg("NFT is not owned by the user")]
    NftNotOwnedByUser,

    #[msg("Update dist rate first")]
    UpdateDistRateFirst,

    #[msg("dogeBtc needed for POl cannot be more than 5% of vault balance")]
    MaxLimitError,

    // ========== FACTION SURGE ERRORS ========== //
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

    #[msg("Maximum breed count reached for this doge")]
    MaxBreedCountReached,

    #[msg("Breeding cooldown has not ended yet")]
    CooldownNotEnded,

    #[msg("Maximum evolution stage reached")]
    MaxEvolutionReached,

    #[msg("Doge metadata not found")]
    DogeMetadataNotFound,

    #[msg("Position already exists")]
    PositionAlreadyExists,

    #[msg("Doge DNA mismatch")]
    ClaimPendingRoundRewards,

    #[msg("Minting not allowed")]
    MintingNotAllowed,

    #[msg("Gameplay locking is only available while RPG progression is enabled")]
    GameplayNotEnabled,

    #[msg("Gameplay unlock has already been requested for this doge")]
    GameplayUnlockAlreadyRequested,

    #[msg("Gameplay unlock has not been requested")]
    GameplayUnlockNotRequested,

    #[msg("Gameplay doge can only be unlocked after the next epoch/campaign cycle begins")]
    GameplayUnlockNotReady,

    #[msg("Claim all pending round, epoch, and token rewards before unlocking this gameplay doge")]
    GameplayRewardsPending,

    // ========== EPOCH MINING ERRORS ========== //
    #[msg("Epoch is not currently active")]
    EpochNotActive,

    #[msg("Epoch has not ended yet")]
    EpochNotEnded,

    #[msg("Epoch has not been settled yet")]
    EpochNotSettled,

    #[msg("Epoch has already been settled")]
    EpochAlreadySettled,

    #[msg("Epoch rewards have already been claimed")]
    EpochRewardsAlreadyClaimed,

    #[msg("Ticket-backed bets exceed the session cap")]
    TicketBetCapExceeded,

    #[msg("Hashpower-changing staking actions are paused while a tax round is active")]
    TaxRoundActive,

    #[msg("Round entropy is not ready yet")]
    RoundEntropyNotReady,

    #[msg("Free Doge mint allowance exceeds the per-user maximum")]
    MaxFreeDogeMintsExceeded,

    #[msg("No free Doge mints remaining for this user")]
    NoFreeDogeMintsRemaining,
}
