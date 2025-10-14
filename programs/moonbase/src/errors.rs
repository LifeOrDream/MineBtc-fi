use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("The authority is invalid")]
    InvalidAuthority,

    #[msg("Invalid referral account")]
    InvalidReferralAccount,

    #[msg("User not authorized to perform this action")]
    Unauthorized,

    #[msg("The facility is already at maximum size")]
    FacilityAlreadyMaxSize,

    #[msg("The facility already has the maximum number of doges")]
    MaxDogesReached,    

    #[msg("Invalid mint")]
    InvalidMint,

    #[msg("Cannot set max doges less than current")]
    MaxDogesLessThanCurrent,



    #[msg("Cannot set max tiles less than current")]
    MaxTilesLessThanCurrent,

    #[msg("Arithmetic overflow")] 
    ArithmeticOverflow,

    #[msg("Cannot set max upgrades less than current")]
    MaxUpgradesLessThanCurrent,

    #[msg("Insufficient electricity capacity")]
    InsufficientElectricity,
    
    #[msg("Mining already initialized")]
    MiningAlreadyInitialized,
    
    #[msg("Config ID already exists")]
    ConfigIdAlreadyExists,
    
    #[msg("Config not found")]
    ConfigNotFound,
    
    #[msg("ID counter overflow")]
    IdOverflow,
    
    #[msg("Module config store not provided")]
    ModuleConfigStoreMissing,
        
    #[msg("Moon doge mining account not provided")]
    MoonDogeMiningSAccountMissing,

    #[msg("User moonbase already exists")]
    UserMoonBaseAlreadyExists,

    #[msg("User moonbase not found")]
    UserMoonBaseNotFound,

    #[msg("Referral pubkey cannot be the same as owner")]
    ReferralCannotBeSameAsOwner,

    #[msg("Module config not found")]
    ModuleConfigNotFound,

    #[msg("Module instance not found")]
    ModuleInstanceNotFound,

    #[msg("Invalid module upgrade: already at max level")]
    ModuleAlreadyMaxLevel,

    #[msg("Invalid tile index")]
    InvalidTileIndex,

    #[msg("Tile already occupied")]
    TileAlreadyOccupied,

    #[msg("Insufficient module capacity")]
    InsufficientModuleCapacity,

    #[msg("Module already has maximum doges")]
    ModuleMaxDogesReached,



    #[msg("Insufficient SOL for operation")]
    InsufficientSOL,

    #[msg("Invalid parameters provided for operation")]
    InvalidParameters,    
    
    #[msg("Module does not have any more space for doges")]
    ModuleFullOfDoges,
    
    #[msg("Insufficient funds in treasury")]
    InsufficientTreasuryFunds,
    
    #[msg("Amount overflow")]
    AmountOverflow,
    
    #[msg("Invalid attribute value")]
    InvalidAttributeValue,



    #[msg("Mining not initialized yet")]
    MiningNotInitialized,

    #[msg("Token vault not initialized")]
    TokenVaultNotInitialized,

    #[msg("Insufficient tokens in vault")]
    InsufficientTokensInVault,

    #[msg("Invalid mining reward rate")]
    InvalidMiningRewardRate,

    #[msg("Cannot reduce electricity below what is currently in use")]
    ElectricityInUse,
    
    #[msg("Electricity consumption exceeds available capacity")]
    ElectricityCapacityExceeded,

    #[msg("Update too early - must wait at least 1 hour between updates")]
    UpdateTooEarly,

    // ========== FACTION-RELATED ERRORS ========== //
    #[msg("Invalid faction name: must be 1-16 characters")]
    InvalidFactionName,

    #[msg("Maximum number of factions reached (10 max)")]
    MaxFactionsReached,

    #[msg("Faction with this name already exists")]
    FactionAlreadyExists,

    #[msg("Invalid faction ID: faction does not exist")]
    InvalidFactionId,

    // ========== MODULE SYSTEM ERRORS ========== //
    #[msg("Invalid module name: must be 1-32 characters")]
    InvalidModuleName,

    #[msg("Invalid image URL: must be 1-64 characters")]
    InvalidImageUrl,

    #[msg("Invalid tile configuration")]
    InvalidTileConfiguration,

    #[msg("Invalid upgrade configuration")]
    InvalidUpgradeConfiguration,

    #[msg("Invalid module configuration")]
    InvalidModuleConfiguration,

    #[msg("Too many faction IDs specified for module")]
    TooManyFactionIds,

    #[msg("Module type does not match provided stats")]
    ModuleTypeMismatch,

    #[msg("Invalid module type for this operation")]
    InvalidModuleType,

    #[msg("User level too low for this module")]
    UserLevelTooLow,

    #[msg("User faction not allowed for this module")]
    FactionNotAllowed,

    #[msg("Maximum instances of this module already reached")]
    MaxModuleInstancesReached,

    #[msg("Module is not active")]
    ModuleNotActive,

    #[msg("Module is damaged and cannot function (requires repair)")]
    ModuleDamaged,

    #[msg("Invalid upgrade level")]
    InvalidUpgradeLevel,

    #[msg("Module instance already at maximum upgrade level")]
    ModuleInstanceMaxLevel,

    #[msg("Research is not ready - cooldown period not elapsed")]
    ResearchNotReady,

    // ========== GRID PLACEMENT ERRORS ========== //
    #[msg("Invalid grid coordinates")]
    InvalidGridCoordinates,

    #[msg("Module placement out of bounds")]
    PlacementOutOfBounds,

    // ========== MOONBASE EXPANSION ERRORS ========== //
    #[msg("Expansion not available: level requirement not met, already purchased, or inactive")]
    ExpansionNotAvailable,

    #[msg("Invalid expansion configuration")]
    InvalidExpansionConfiguration,

    #[msg("Maximum number of expansions reached")]
    MaxExpansionsReached,

    #[msg("Expansion already exists with this ID")]
    ExpansionAlreadyExists,

    #[msg("Expansion not found")]
    ExpansionNotFound,

    #[msg("Cannot place module outside current moonbase area - upgrade your moonbase first")]
    PlacementOutsideMoonbaseArea,

    // ========== LIQUIDITY POOL ERRORS ========== //
    #[msg("LP token burn was incomplete - final balance doesn't match expected")]
    IncompleteTokenBurn,

    #[msg("Runtime state does not match module stats type")] 
    StateMismatch,

    #[msg("Module is already at full HP")] 
    ModuleFullyRepaired,

    // ========== PVP GAME ERRORS ========== //
    #[msg("Player is already in an active PvP game")]
    PlayerAlreadyInGame,

    #[msg("Maximum number of games per index reached")]
    MaxGamesPerIndex,

    #[msg("Game already has a player B")]
    GameAlreadyHasPlayerB,

    #[msg("Game not found")]
    GameNotFound,

    #[msg("Player A mismatch")]
    PlayerAMismatch,

    #[msg("Game has not expired yet (must be at least 30 minutes old)")]
    GameNotExpired,

    // ========== PVP GAME SESSION ERRORS ========== //
    #[msg("Game has already finished")]
    GameAlreadyFinished,

    #[msg("It's not your turn to attack")]
    NotYourTurn,

    #[msg("Attack module not found in moonbase")]
    AttackModuleNotFound,

    #[msg("Target module type not found in enemy moonbase")]
    TargetModuleNotFound,

    #[msg("Attack module has no ammunition left")]
    NoAmmunitionLeft,

    #[msg("Attack module is on cooldown")]
    AttackOnCooldown,

    #[msg("Insufficient moonbase HP for PvP (minimum 1000 HP required)")]
    InsufficientPvPHP,

    #[msg("Game timeout exceeded (5 minute limit)")]
    GameTimeout,

    #[msg("Moonbase under repair - PvP unavailable")]
    MoonbaseUnderRepair,

    #[msg("PvP games are currently disabled")]
    PvPGamesDisabled,

    #[msg("Module config ID does not match the expected config")]
    ModuleConfigMismatch,

    #[msg("No undeployed modules available to delete")]
    NoUndeployedModulesAvailable,

    #[msg("Module is already deployed and active")]
    ModuleAlreadyActive,
} 