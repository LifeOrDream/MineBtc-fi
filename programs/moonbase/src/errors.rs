use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("The authority is invalid")]
    InvalidAuthority,

    #[msg("Invalid referral account")]
    InvalidReferralAccount,

    #[msg("User not authorized to perform this action")]
    Unauthorized,

    #[msg("Invalid mint")]
    InvalidMint,

    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,

    #[msg("Mining already initialized")]
    MiningAlreadyInitialized,

    #[msg("Referral pubkey cannot be the same as owner")]
    ReferralCannotBeSameAsOwner,

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

    #[msg("Maximum number of factions reached (10 max)")]
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

    #[msg("Dragon Egg is already incubated in a moonbase")]
    EggAlreadyIncubated,

    #[msg("Dragon Egg is not incubated in this moonbase")]
    EggNotIncubated,

    #[msg("Egg limit for this tier has been reached")]
    EggLimitExceeded,

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
}
