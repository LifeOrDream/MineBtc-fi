use anchor_lang::prelude::*;

#[error_code]
pub enum DragonHiveError {
    // ========================================================================================
    // ================================ AUTHORITY ERRORS ===================================== 
    // ========================================================================================
    #[msg("Unauthorized: Only program authority can perform this action")]
    Unauthorized,

    #[msg("Invalid authority provided")]
    InvalidAuthority,

    // ========================================================================================
    // ================================= NFT ERRORS ========================================== 
    // ========================================================================================
    #[msg("DragonBee not found or invalid")]
    DragonBeeNotFound,

    #[msg("DragonBee is not owned by the user")]
    DragonBeeNotOwnedByUser,

    #[msg("DragonBee collection is full - maximum supply reached")]
    MaxSupplyReached,

    #[msg("Invalid DragonBee type - must be between 1 and 8")]
    InvalidDragonBeeType,

    #[msg("DragonBee is not ready for evolution")]
    NotReadyForEvolution,

    #[msg("DragonBee is already at maximum evolution stage")]
    AlreadyMaxEvolution,

    #[msg("Invalid genetic data provided")]
    InvalidGeneticData,

    // ========================================================================================
    // =============================== BREEDING ERRORS ===================================== 
    // ========================================================================================
    #[msg("Cannot breed DragonBees of different types")]
    IncompatibleBreedingTypes,

    #[msg("DragonBee is still in breeding cooldown period")]
    BreedingCooldownActive,

    #[msg("Breeding failed - insufficient compatibility")]
    BreedingFailed,

    #[msg("Queen DragonBee is not available for breeding")]
    QueenNotAvailable,

    #[msg("Breeding auction has not ended yet")]
    AuctionStillActive,

    #[msg("Breeding auction has already ended")]
    AuctionEnded,

    #[msg("Bid amount is too low")]
    BidTooLow,

    #[msg("Cannot breed with yourself")]
    SelfBreedingNotAllowed,

    #[msg("Parent DragonBees cannot be the same")]
    SameParentNotAllowed,

    #[msg("DragonBee has reached maximum breeding limit")]
    MaxBreedingLimitReached,

    // ========================================================================================
    // ================================ ECONOMIC ERRORS ====================================== 
    // ========================================================================================
    #[msg("Insufficient SOL balance for purchase")]
    InsufficientSOLBalance,

    #[msg("Insufficient HONEY tokens for operation")]
    InsufficientHoneyTokens,

    #[msg("Invalid payment amount")]
    InvalidPaymentAmount,

    #[msg("Treasury has insufficient funds")]
    InsufficientTreasuryFunds,

    #[msg("Kill rewards pool is empty")]
    EmptyKillRewardsPool,

    #[msg("DragonBee power too low for kill rewards")]
    InsufficientPowerForKill,

    // ========================================================================================
    // =============================== VALIDATION ERRORS =================================== 
    // ========================================================================================
    #[msg("Name too long - maximum 32 characters")]
    NameTooLong,

    #[msg("URI too long - maximum 200 characters")]  
    UriTooLong,

    #[msg("Invalid metadata provided")]
    InvalidMetadata,

    #[msg("Power increase exceeds maximum allowed per update")]
    PowerIncreaseExceedsLimit,

    #[msg("Rate limit exceeded - too many operations")]
    RateLimitExceeded,

    // ========================================================================================
    // ================================ ACCOUNT ERRORS ======================================= 
    // ========================================================================================
    #[msg("Account already initialized")]
    AccountAlreadyInitialized,

    #[msg("Account not initialized")]
    AccountNotInitialized,

    #[msg("Invalid account provided")]
    InvalidAccount,

    #[msg("Account size mismatch")]
    AccountSizeMismatch,

    #[msg("PDA derivation failed")]
    PDADerivationFailed,

    // ========================================================================================
    // =============================== TOKEN ERRORS ======================================== 
    // ========================================================================================
    #[msg("Invalid token mint")]
    InvalidTokenMint,

    #[msg("Invalid token account")]
    InvalidTokenAccount,

    #[msg("Token transfer failed")]
    TokenTransferFailed,

    #[msg("Token mint failed")]
    TokenMintFailed,

    #[msg("Token burn failed")]
    TokenBurnFailed,

    // ========================================================================================
    // =============================== ARITHMETIC ERRORS =================================== 
    // ========================================================================================
    #[msg("Arithmetic overflow occurred")]
    ArithmeticOverflow,

    #[msg("Arithmetic underflow occurred")]
    ArithmeticUnderflow,

    #[msg("Division by zero")]
    DivisionByZero,

    // ========================================================================================
    // ================================ TIME ERRORS ========================================== 
    // ========================================================================================
    #[msg("Invalid timestamp")]
    InvalidTimestamp,

    #[msg("Operation too early - cooldown period not met")]
    OperationTooEarly,

    #[msg("Operation expired")]
    OperationExpired,

    // ========================================================================================
    // =============================== GENETIC ERRORS ====================================== 
    // ========================================================================================
    #[msg("Invalid genetic trait value")]
    InvalidGeneticTrait,

    #[msg("Genetic data corruption detected")]
    GeneticDataCorrupted,

    #[msg("Evolution requirements not met")]
    EvolutionRequirementsNotMet,

    #[msg("Trait enhancement failed")]
    TraitEnhancementFailed,

    // ========================================================================================
    // =============================== GAME ERRORS ========================================= 
    // ========================================================================================
    #[msg("DragonBee is currently in a game and cannot be modified")]
    DragonBeeInGame,

    #[msg("Invalid game state")]
    InvalidGameState,

    #[msg("Game interaction failed")]
    GameInteractionFailed,

    // ========================================================================================
    // ================================ AUCTION ERRORS ======================================= 
    // ========================================================================================
    #[msg("Auction not found")]
    AuctionNotFound,

    #[msg("Cannot bid on your own auction")]
    CannotBidOnOwnAuction,

    #[msg("Auction has no bids")]
    AuctionNoBids,

    #[msg("Not the winning bidder")]
    NotWinningBidder,

    #[msg("User is already participating with a DragonBee in this auction")]
    AlreadyParticipatingWithABee,

    #[msg("Position not found for this user")]
    PositionNotFound,

    #[msg("This is an old position from a previous auction")]
    OldPosition,

    #[msg("User has already added to position in limited phase")]
    AlreadyAddedToPosition,

    #[msg("Bid amount exceeds the allowed limit for limited phase")]
    BidAmountExceedsLimit,

    // ========================================================================================
    // =============================== COLLECTION ERRORS =================================== 
    // ========================================================================================
    #[msg("Collection not found")]
    CollectionNotFound,

    #[msg("Invalid collection configuration")]
    InvalidCollectionConfig,

    #[msg("Collection is not mutable")]
    CollectionNotMutable,

    // ========================================================================================
    // =============================== PROGRAM ERRORS ====================================== 
    // ========================================================================================
    #[msg("Program is paused")]
    ProgramPaused,

    #[msg("Feature not implemented yet")]
    FeatureNotImplemented,

    #[msg("Invalid program state")]
    InvalidProgramState,

    #[msg("Operation not supported")]
    OperationNotSupported,

    // ========================================================================================
    // =============================== QUEEN AUCTION ERRORS ================================== 
    // ========================================================================================
    #[msg("Queen auction manager is not live")]
    QueenMakerNotLive,

    #[msg("Invalid auction phase")]
    InvalidAuctionPhase,

    #[msg("Invalid parameters provided")]
    InvalidParameters,
}
