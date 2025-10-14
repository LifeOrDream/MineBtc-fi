use anchor_lang::prelude::*;

#[error_code]
pub enum NftLaunchpadError {
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
    #[msg("MoonDoge NFT not found or invalid")]
    MoonDogeNotFound,

    #[msg("Dragon Egg NFT not found or invalid")]
    DragonEggNotFound,

    #[msg("MoonDoge collection is full - maximum supply reached")]
    MaxMoonDogeSupplyReached,

    #[msg("Dragon Egg collection is full - maximum supply reached")]
    MaxDragonEggSupplyReached,

    #[msg("NFT is not owned by the user")]
    NftNotOwnedByUser,

    // ========================================================================================
    // ============================== ATTACHMENT ERRORS ====================================== 
    // ========================================================================================
    #[msg("MoonDoge is already attached to a moonbase")]
    DogeAlreadyAttached,

    #[msg("Dragon Egg is already incubated in a moonbase")]
    EggAlreadyIncubated,

    #[msg("Moonbase already has a MoonDoge attached (max 1 per moonbase)")]
    MoonbaseAlreadyHasDoge,

    #[msg("Moonbase has reached maximum incubated eggs limit")]
    MaxEggsReached,

    #[msg("MoonDoge is not attached to this moonbase")]
    DogeNotAttached,

    #[msg("Dragon Egg is not incubated in this moonbase")]
    EggNotIncubated,

    // ========================================================================================
    // =============================== ECONOMIC ERRORS ======================================= 
    // ========================================================================================
    #[msg("Insufficient SOL balance for purchase")]
    InsufficientSOLBalance,

    #[msg("Invalid payment amount")]
    InvalidPaymentAmount,

    #[msg("Treasury has insufficient funds")]
    InsufficientTreasuryFunds,

    #[msg("Invalid pricing tier selected")]
    InvalidPricingTier,

    // ========================================================================================
    // =============================== VALIDATION ERRORS ===================================== 
    // ========================================================================================
    #[msg("Name too long - maximum 32 characters")]
    NameTooLong,

    #[msg("URI too long - maximum 200 characters")]  
    UriTooLong,

    #[msg("Invalid metadata provided")]
    InvalidMetadata,

    #[msg("Power value exceeds maximum allowed")]
    PowerExceedsLimit,

    #[msg("Money value exceeds maximum allowed")]
    MoneyExceedsLimit,

    // ========================================================================================
    // ================================ ACCOUNT ERRORS ======================================= 
    // ========================================================================================
    #[msg("Account already initialized")]
    AccountAlreadyInitialized,

    #[msg("Account not initialized")]
    AccountNotInitialized,

    #[msg("Invalid account provided")]
    InvalidAccount,

    #[msg("PDA derivation failed")]
    PDADerivationFailed,

    // ========================================================================================
    // =============================== ARITHMETIC ERRORS ===================================== 
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

    #[msg("Update too early - must wait for cooldown")]
    UpdateTooEarly,

    // ========================================================================================
    // =============================== PROGRAM ERRORS ======================================== 
    // ========================================================================================
    #[msg("Program is paused")]
    ProgramPaused,

    #[msg("Invalid program state")]
    InvalidProgramState,

    // ========================================================================================
    // ============================== METAPLEX CORE ERRORS =================================== 
    // ========================================================================================
    #[msg("Metaplex Core program ID mismatch")]
    InvalidMplCoreProgram,

    #[msg("Invalid collection provided")]
    InvalidCollection,

    #[msg("Asset creation failed")]
    AssetCreationFailed,

    #[msg("Asset transfer failed")]
    AssetTransferFailed,
}
