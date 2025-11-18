use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;
use anchor_spl::token::Token;
use anchor_spl::associated_token::AssociatedToken;

use crate::errors::ErrorCode;
use crate::events::*;
use crate::state::*;
use crate::instructions::helper;




// ========================================================================================
// =============================== FACTION SURGE INSTRUCTIONS ============================
// ========================================================================================

/// Initialize a player account for the Faction Surge game
pub fn initialize_player(ctx: Context<InitializePlayer>, faction_id: u8, referral_code: Option<Pubkey>) -> Result<()> {
    msg!("👤 [initialize_player] Initializing player account. Authority: {}. Faction ID: {}", ctx.accounts.authority.key(), faction_id);
    
    let player_data = &mut ctx.accounts.player_data;
    let global_config = &mut ctx.accounts.global_config;    
    global_config.total_players = global_config.total_players + 1;
    
    // Validate faction_id
    require!(
        (faction_id as usize) < global_config.supported_factions.len(),
        ErrorCode::InvalidFactionId
    );
    
    // Initialize player data
    player_data.owner = ctx.accounts.authority.key();
    player_data.bump = ctx.bumps.player_data;
    player_data.faction_id = faction_id;
    
    // Handle referral code logic
    let referrer_pubkey = if let Some(ref_code) = referral_code {
        msg!("     Referral code provided: {}", ref_code);
        require!( ref_code != ctx.accounts.authority.key(), ErrorCode::ReferralCannotBeSameAsOwner);
        
        // Update referrer's referral count if referrer_rewards account is provided
        if let Some(ref mut referrer_rewards) = ctx.accounts.referrer_rewards {
            require!( referrer_rewards.owner == ref_code, ErrorCode::InvalidReferralAccount);            
            referrer_rewards.referrals_count = referrer_rewards.referrals_count + 1;
        } 
        
        // Set player's referral code
        player_data.referral_code = ref_code;
        ref_code
    } else {
        msg!("     No referral code provided, using system referral account");
        let system_referral_pubkey = ctx.accounts.system_program.key();
        player_data.referral_code = system_referral_pubkey;
        system_referral_pubkey
    };

    player_data.bets_rounds = Vec::new();
    player_data.bets_points = Vec::new();

    // Initialize statistics
    player_data.rounds_played = 0;

    player_data.total_sol_bet = 0;
    player_data.total_points_bet = 0;
    player_data.total_sol_won = 0;
    player_data.total_dbtc_won = 0;
    
    // Initialize DogeBtc staking fields
    player_data.dogebtc_hashpower = 0;
    player_data.dogebtc_staked = 0;
    player_data.dbtc_dbtc_reward_debt = 0;
    player_data.dbtc_sol_reward_debt = 0;
    msg!("     DogeBtc staking fields initialized");
    
    // Initialize LP staking fields
    player_data.lp_hashpower = 0;
    player_data.lp_staked = 0;
    player_data.lp_sol_reward_debt = 0;
    player_data.lp_dbtc_reward_debt = 0;
    msg!("     LP staking fields initialized");
    
    // Initialize pending rewards
    player_data.pending_sol_rewards = 0;
    player_data.pending_dbtc_rewards = 0;
    msg!("     Pending rewards initialized");
    
    // Initialize position tracking vectors
    player_data.moondoge_position_indices = Vec::new();
    player_data.lp_position_indices = Vec::new();
    msg!("     Position tracking initialized");
    
    // Initialize egg staking
    player_data.staked_eggs = Vec::new();
    player_data.egg_multiplier = 100; // Default 1.0x (no eggs staked)
    msg!("     Egg staking initialized (0 eggs, 1.0x multiplier)");
    
    // Initialize free tickets vectors
    player_data.free_tickets = Vec::new();
    player_data.free_tickets_remaining = Vec::new();
    msg!("     Free tickets vectors initialized (empty)");
    
    // Initialize new player's referral rewards account
    msg!("   Initializing new player's referral rewards account...");
    let new_player_rewards = &mut ctx.accounts.new_player_rewards;
    new_player_rewards.owner = ctx.accounts.authority.key();
    new_player_rewards.bump = ctx.bumps.new_player_rewards;
    new_player_rewards.referrals_count = 0;
    new_player_rewards.pending_sol_rewards = 0;
    new_player_rewards.pending_dbtc_rewards = 0;
    new_player_rewards.total_sol_earned = 0;
    new_player_rewards.total_dbtc_earned = 0;
    msg!("     Referral rewards account initialized");
    
    msg!("✅ [initialize_player] Player initialized successfully");
    msg!("   Player: {} for faction {}", ctx.accounts.authority.key(), faction_id);
    if referral_code.is_some() {
        msg!("   Referral code: {}", referrer_pubkey);
    } else {
        msg!("   Using system referral account: {}", referrer_pubkey);
    }
    
    emit!(PlayerInitialized {
        user: ctx.accounts.authority.key(),
        player_data: ctx.accounts.player_data.key(),
        faction_id,
        referral_code,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    Ok(())
}

/// Change user's faction
/// Requires:
/// - No dbtc hashpower (dogebtc_hashpower == 0)
/// - No lp hashpower (lp_hashpower == 0)
/// - No eggs staked (staked_eggs.is_empty())
/// Charges change_faction_fee: 50% to sol_treasury, 50% to fee_recipient (as WSOL)
pub fn change_faction(ctx: Context<ChangeFaction>, new_faction_id: u8) -> Result<()> {
    msg!("🔄 [change_faction] User changing faction. User: {}", ctx.accounts.authority.key());
    msg!("   Current faction ID: {}. New faction ID: {}", ctx.accounts.player_data.faction_id, new_faction_id);
    
    let player_data = &mut ctx.accounts.player_data;
    let global_config = &ctx.accounts.global_config;
    
    // Validate new faction_id
    require!( (new_faction_id as usize) < global_config.supported_factions.len(), ErrorCode::InvalidFactionId);    
    require!( player_data.faction_id != new_faction_id, ErrorCode::InvalidParameters);
    
    // Validate user has no staked positions
    msg!("   Validating user has no staked positions...");
    require!( player_data.dogebtc_hashpower == 0 && player_data.lp_hashpower == 0 && player_data.staked_eggs.is_empty(), ErrorCode::InvalidParameters);
    
    // Charge change_faction_fee
    let change_fee = global_config.change_faction_fee;
    require!(change_fee > 0, ErrorCode::InvalidAmount);
    msg!("   Change faction fee: {} SOL", (change_fee as f64 / 1e9));
    
    // Split fee: 50% to sol_treasury, 50% to fee_recipient (as WSOL)
    let treasury_amt = change_fee / 2;
    let dev_amt = change_fee - treasury_amt;
    
    msg!("   Transferring {} SOL to sol_treasury", (treasury_amt as f64 / 1e9));
    helper::transfer_to_sol_treasury(
        &ctx.accounts.user_wallet.to_account_info(),
        &ctx.accounts.sol_treasury.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        treasury_amt,
    )?;
    
    msg!("   Transferring {} SOL to fee_recipient (as WSOL)", (dev_amt as f64 / 1e9));
    helper::transfer_wsol_to_multisig(
        &ctx.accounts.user_wallet.to_account_info(),
        &ctx.accounts.multisig_wsol_account.to_account_info(),
        &ctx.accounts.user_wsol_account.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
        dev_amt,
    )?;
    
    // Update faction_id
    let old_faction_id = player_data.faction_id;
    player_data.faction_id = new_faction_id;
    
    msg!("✅ [change_faction] Faction changed successfully");
    msg!("   User: {}", ctx.accounts.authority.key());
    msg!("   Old faction ID: {} -> New faction ID: {}", old_faction_id, new_faction_id);
    
    emit!(FactionChanged {
        user: ctx.accounts.authority.key(),
        player_data: ctx.accounts.player_data.key(),
        new_faction_id,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    Ok(())
}

/// Join a round by betting SOL or using free tickets (single bet)
/// Users can bet on either:
/// - A specific block (block_id: 0-23, 0-indexed)
/// - A faction + highest/lowest option (faction_id + is_highest)
/// 
/// Parameters:
/// - amount: Bet amount in lamports (for SOL) or points (for tickets). 1 point = 1 SOL lamport
/// - bet_type: The bet type (Block, FactionHighestLowest, FactionBoth, or RandomBlock)
/// - use_ticket: Optional ticket type index (0-4). If None, uses SOL. If Some(index), uses ticket from free_tickets[index]
/// 
/// Note: Faction accounts are not required for user betting functions. Faction-related calculations
/// are handled in end_round_faction_rewards by cranker bots.
pub fn join_round(
    ctx: Context<JoinRound>, 
    amount: u64,
    bet_type: BetType,
    use_ticket: Option<u8>,
) -> Result<()> {
    msg!("🎲 [join_round] User joining round (single bet). User: {}", ctx.accounts.authority.key());
    msg!("   Bet type: {:?}", bet_type);
        
    // Call internal join_round with user as payer
    let (target_block, net_amount, fee_amount, points_amount) = internal_join_round(
        &ctx.accounts.global_game_state,
        &ctx.accounts.global_config,
        &mut ctx.accounts.player_data,
        &mut ctx.accounts.game_session,
        &mut ctx.accounts.user_game_bet,
        &ctx.accounts.user_wallet.to_account_info(),
        &ctx.accounts.sol_treasury.to_account_info(),
        &ctx.accounts.sol_prize_pot_vault.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ctx.bumps.user_game_bet,
        ctx.accounts.authority.key(),
        amount,
        bet_type.clone(),
        use_ticket,
    )?;
 
    
    emit!(RoundJoined {
        user: ctx.accounts.authority.key(),
        player_data: ctx.accounts.player_data.key(),
        round_id: ctx.accounts.global_game_state.current_round_id,
        target_block,
        net_amount,
        fee_amount,
        points_amount,
        used_ticket: use_ticket.is_some(),
        ticket_type_index: use_ticket,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    msg!("✅ [join_round] Bet placed successfully");
    Ok(())
}

/// Join a round with multiple bets in a single transaction
/// Users can bet on:
/// - Multiple blocks (e.g., [0, 4, 9, 14] - 0-indexed: 0-23)
/// - Multiple factions with settings: "low", "high", "both", or "random"
/// 
/// Parameters:
/// - bet_types: Vector of bet types to place (all must be for the same faction)
/// - amount_per_bet: Bet amount per bet type in lamports (for SOL) or points (for tickets)
/// - use_ticket: Optional ticket type index (0-4). If None, uses SOL. If Some(index), uses ticket from free_tickets[index]
/// 
/// Note: Faction accounts are not required for user betting functions. Faction-related calculations
/// are handled in end_round_faction_rewards by cranker bots.
pub fn join_round_batch(
    ctx: Context<JoinRoundBatch>,
    bet_types: Vec<BetType>,
    amount_per_bet: u64,
    use_ticket: Option<u8>,
) -> Result<()> {
    msg!("🎲 [join_round_batch] User joining round with {} bets", bet_types.len());
    msg!("   User: {}", ctx.accounts.authority.key());
    msg!("   Amount per bet: {} lamports", amount_per_bet);
    
    require!(!bet_types.is_empty(), ErrorCode::InvalidParameters);
    require!(bet_types.len() <= 24, ErrorCode::InvalidParameters); // Max 24 bets (one per block)
    
    // Expand bet types (handle FactionBoth and RandomBlock)
    let mut expanded_bet_types = Vec::new();
    for bet_type in bet_types.iter() {
        match bet_type {
            BetType::FactionBoth { faction_id } => {
                // Expand to both highest and lowest
                expanded_bet_types.push(BetType::FactionHighestLowest { 
                    faction_id: *faction_id, 
                    is_highest: true 
                });
                expanded_bet_types.push(BetType::FactionHighestLowest { 
                    faction_id: *faction_id, 
                    is_highest: false 
                });
            }
            BetType::RandomBlock => {
                // For random block, we need to select a random block at runtime
                // Use slot hash or similar for randomness
                let clock = Clock::get()?;
                let slot_bytes = clock.slot.to_le_bytes();
                let random_block = (slot_bytes[0] % 24) as u8; // 0-23 (0-indexed)
                expanded_bet_types.push(BetType::Block { block_id: random_block });
                msg!("   Random block selected: {}", random_block);
            }
            _ => {
                expanded_bet_types.push(bet_type.clone());
            }
        }
    }
    
    msg!("   Expanded to {} bet types", expanded_bet_types.len());

    let mut target_blocks = Vec::new();
    let mut net_amounts = Vec::new();
    let mut fee_amounts = Vec::new();
    let mut points_amounts = Vec::new();
    
    // Place each bet
    // Note: No faction validation needed - faction_state is not required for betting
    for (idx, bet_type) in expanded_bet_types.iter().enumerate() {
        msg!("   Placing bet {} of {}: {:?}", idx + 1, expanded_bet_types.len(), bet_type);
        
        // Call internal join_round for each bet
        let (target_block, net_amount, fee_amount, points_amount) = internal_join_round(
            &ctx.accounts.global_game_state,
            &ctx.accounts.global_config,
            &mut ctx.accounts.player_data,
            &mut ctx.accounts.game_session,
            &mut ctx.accounts.user_game_bet,
            &ctx.accounts.user_wallet.to_account_info(),
            &ctx.accounts.sol_treasury.to_account_info(),
            &ctx.accounts.sol_prize_pot_vault.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            ctx.bumps.user_game_bet,
            ctx.accounts.authority.key(),
            amount_per_bet,
            bet_type.clone(),
            use_ticket,
        )?;
        
        target_blocks.push(target_block);
        net_amounts.push(net_amount);
        fee_amounts.push(fee_amount);
        points_amounts.push(points_amount);
    }
    
    emit!(RoundJoinedBatch {
        user: ctx.accounts.authority.key(),
        player_data: ctx.accounts.player_data.key(),
        round_id: ctx.accounts.global_game_state.current_round_id,
        num_bets: expanded_bet_types.len() as u8,
        target_blocks,
        net_amounts,
        fee_amounts,
        points_amounts,
        used_ticket: use_ticket.is_some(),
        ticket_type_index: use_ticket,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    msg!("✅ [join_round_batch] All {} bets placed successfully", expanded_bet_types.len());
    Ok(())
}

 


/// Initialize autominer vault with flexible block/faction configuration
/// Users configure either blocks OR factions (at least one required)
/// Can be called multiple times, but only when rounds_remaining == 0
/// Total SOL = sol_per_round × num_rounds
/// Bet size per bet = sol_per_round / total_bets_per_round
pub fn init_autominer(
    ctx: Context<InitAutominer>,
    blocks_config: Option<BlocksConfig>,
    factions_config: Option<FactionsConfig>,
    sol_per_round: u64,
    num_rounds: u32,
) -> Result<()> {
    msg!("🤖 [init_autominer] Initializing autominer vault");
    msg!("   Owner: {}", ctx.accounts.user_wallet.key());
    msg!("   SOL per round: {} lamports", sol_per_round);
    msg!("   Number of rounds: {}", num_rounds);
    
    let autominer_vault = &mut ctx.accounts.autominer_vault;
    let global_config = &ctx.accounts.global_config;
    
    msg!("   Validating parameters...");
    require!(sol_per_round > 0 && num_rounds > 0, ErrorCode::InvalidAmount);    
    require!( blocks_config.is_some() || factions_config.is_some(), ErrorCode::InvalidParameters);
    require!( !(blocks_config.is_some() && factions_config.is_some()), ErrorCode::InvalidParameters); // Only one config allowed
    
    // Check if vault already exists and has remaining rounds
    // Only allow initialization if rounds_remaining == 0 (must stop first if in progress)
    require!( autominer_vault.rounds_remaining == 0,  ErrorCode::InvalidParameters);
    let mut bets_per_round = 0;
    
    // Validate blocks_config if provided
    if let Some(ref blocks_cfg) = blocks_config {
        match blocks_cfg {
            BlocksConfig::Specific { blocks } => {
                require!(!blocks.is_empty(), ErrorCode::InvalidParameters);
                require!(blocks.len() <= AutominerVault::MAX_BLOCKS, ErrorCode::InvalidParameters);
                for &block_id in blocks.iter() {
                    require!(block_id < NUM_BLOCKS as u8, ErrorCode::InvalidParameters);
                }
                bets_per_round = blocks.len() as u64;
                msg!("     ✓ Blocks: {} specific blocks", blocks.len());
            }
            BlocksConfig::Random { count } => {
                require!(*count > 0 && *count <= NUM_BLOCKS as u8, ErrorCode::InvalidParameters);
                msg!("     ✓ Blocks: {} random blocks", count);
                bets_per_round = *count as u64;
            }
        }
    }
    
    // Validate factions_config if provided
    if let Some(ref factions_cfg) = factions_config {
        match factions_cfg {
            FactionsConfig::Specific { factions, strategy } => {
                require!(!factions.is_empty(), ErrorCode::InvalidParameters);
                require!(factions.len() <= AutominerVault::MAX_FACTIONS, ErrorCode::InvalidParameters);
                for &faction_id in factions.iter() {
                    require!( (faction_id as usize) < global_config.supported_factions.len(), ErrorCode::InvalidFactionId);
                }
                let strategy_multiplier = get_strategy_multiplier(strategy.clone());
                bets_per_round = factions.len() as u64 * strategy_multiplier;
                msg!("     ✓ Factions: {} specific factions", factions.len());
            }
            FactionsConfig::Random { count, strategy } => {
                require!(*count > 0 && *count <= global_config.supported_factions.len() as u8, ErrorCode::InvalidParameters);
                msg!("     ✓ Factions: {} random factions", count);
                let strategy_multiplier = get_strategy_multiplier(strategy.clone());
                bets_per_round = *count as u64 * strategy_multiplier;
            }
        }
    }

    require!(bets_per_round > 0, ErrorCode::InvalidParameters);
    msg!("     ✓ Bets per round: {}", bets_per_round);
     
    // Calculate bet size per bet
    let bet_size_per_bet = sol_per_round / bets_per_round;
    require!(bet_size_per_bet > 0, ErrorCode::InvalidAmount);
    msg!("     Bet size per bet:{} SOL per bet ({} SOL / {} bets)", (bet_size_per_bet as f64 / 1e9), (sol_per_round as f64 / 1e9), bets_per_round);
    
    msg!("   Initializing autominer vault...");

    // Store config flags before moving values
    let has_blocks_config = blocks_config.is_some();
    let has_factions_config = factions_config.is_some();

    autominer_vault.owner = ctx.accounts.user_wallet.key();
    autominer_vault.blocks_config = blocks_config;
    autominer_vault.factions_config = factions_config;
    autominer_vault.sol_per_round = sol_per_round;
    autominer_vault.rounds_remaining = num_rounds;
    autominer_vault.last_bet_round_id = 0;
    autominer_vault.vault_bump = ctx.bumps.autominer_vault;
    msg!("     Vault initialized for owner: {}", autominer_vault.owner);
    
    // Calculate total SOL needed: sol_per_round × num_rounds + rent (if new account)
    msg!("   Calculating total SOL needed...");
    let total_sol = sol_per_round * num_rounds as u64;
    msg!("     Total SOL for all rounds: {} SOL ({} rounds × {} SOL)", (total_sol as f64 / 1e9), num_rounds, (sol_per_round as f64 / 1e9));
    
    let rent = Rent::get()?.minimum_balance(AutominerVault::LEN);
    let vault_lamports = ctx.accounts.autominer_vault.to_account_info().lamports();
    
    // If vault already exists, only transfer the SOL needed
    // If new vault, also need rent
    let total_transfer = if vault_lamports == 0 {
        total_sol + rent
    } else {
        total_sol
    };
    
    msg!("     Rent: {} lamports", rent);
    msg!("     Total transfer: {} lamports", total_transfer);
    
    // Transfer SOL to vault
    msg!("   Transferring SOL to vault...");
    **ctx.accounts.autominer_vault.to_account_info().try_borrow_mut_lamports()? += total_transfer;
    **ctx.accounts.user_wallet.to_account_info().try_borrow_mut_lamports()? -= total_transfer;

    msg!("✅ [init_autominer] Autominer initialized successfully");
    msg!("   {} SOL per round, {} rounds ({} SOL total)", (sol_per_round as f64 / 1e9), num_rounds, (total_sol as f64 / 1e9));
    
    emit!(AutominerInitialized {
        owner: ctx.accounts.user_wallet.key(),
        player_data: ctx.accounts.player_data.key(),
        autominer_vault: ctx.accounts.autominer_vault.key(),
        sol_per_round,
        num_rounds,
        bets_per_round,
        bet_size_per_bet,
        has_blocks_config,
        has_factions_config,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    Ok(())
}


fn get_strategy_multiplier(strategy: FactionStrategy) -> u64 {
    match strategy {
        FactionStrategy::Both => 2,
        _ => 1,
    }
}


/// Execute autominer bets (keeper instruction - callable by anyone)
/// Generates bet types dynamically from blocks_config and factions_config
/// Pays caller 1% of bet size (max 0.005 SOL) per bet for tx costs
/// Uses join_round_batch to place all bets efficiently
pub fn execute_autominer_bet(ctx: Context<ExecuteAutominerBet>) -> Result<()> {
    msg!("🤖 [execute_autominer_bet] Executing autominer bets");
    msg!("   Owner: {}", ctx.accounts.autominer_vault.owner);
    msg!("   Caller: {}", ctx.accounts.caller.key());
    
    let global_state = &ctx.accounts.global_game_state;
    let global_config = &ctx.accounts.global_config;
    let clock = Clock::get()?;
    
    // Read values before mutable borrow
    let rounds_remaining = ctx.accounts.autominer_vault.rounds_remaining;
    let last_bet_round_id = ctx.accounts.autominer_vault.last_bet_round_id;
    let sol_per_round = ctx.accounts.autominer_vault.sol_per_round;
    let blocks_config = ctx.accounts.autominer_vault.blocks_config.clone(); // Already Option<BlocksConfig>
    let factions_config = ctx.accounts.autominer_vault.factions_config.clone();
    
    msg!("   Vault state:");
    msg!("     Rounds remaining: {}. Last bet round ID: {}. SOL per round: {} SOL", rounds_remaining, last_bet_round_id, (sol_per_round as f64 / 1e9));
    msg!("   Current round ID: {}. Current timestamp: {}. Round end timestamp: {}", global_state.current_round_id, clock.unix_timestamp, global_state.round_end_timestamp);
    
    require!(rounds_remaining > 0, ErrorCode::NoRoundsRemaining);
    require!(clock.unix_timestamp < global_state.round_end_timestamp, ErrorCode::RoundEnded);
    require!(last_bet_round_id != global_state.current_round_id, ErrorCode::InvalidRound);
    
    // Generate bet types dynamically from configuration
    msg!("   Generating bet types from configuration...");
    
    // Determine blocks to bet on (if blocks_config provided)
    let blocks_to_bet = compute_blocks_to_bet(blocks_config, &clock)?;
    
    // Generate bet types using helper function
    let bet_types = make_bets_vec(
        factions_config.clone(),
        blocks_to_bet.clone(),
        &ctx.accounts.game_session,
        &clock,
        &global_config,
    )?;
    msg!("     Generated {} bet types", bet_types.len());
    
    require!(!bet_types.is_empty(), ErrorCode::InvalidParameters);
    
    // Calculate caller compensation FIRST: 1% of sol_per_round, max 0.005 SOL
    let total_caller_compensation = get_caller_compensation(sol_per_round)?;
    msg!("     Caller compensation: {} SOL (1% of {} SOL, max 0.005 SOL)", 
        (total_caller_compensation as f64 / 1e9), 
        (sol_per_round as f64 / 1e9));
    
    // Deduct caller compensation from sol_per_round to get actual betting amount
    let sol_for_betting = sol_per_round
        .checked_sub(total_caller_compensation)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!("     SOL for betting: {} SOL ({} SOL - {} SOL compensation)", 
        (sol_for_betting as f64 / 1e9),
        (sol_per_round as f64 / 1e9),
        (total_caller_compensation as f64 / 1e9));
    
    // Calculate bet size per bet (distributed across all bets)
    let bet_size_per_bet = sol_for_betting / bet_types.len() as u64;
    require!(bet_size_per_bet > 0, ErrorCode::InvalidAmount);
    msg!("     Bet size per bet: {} SOL ({} SOL / {} bets)", 
        (bet_size_per_bet as f64 / 1e9), 
        (sol_for_betting as f64 / 1e9), 
        bet_types.len());
    
    // Pay caller compensation
    if total_caller_compensation > 0 {
        msg!("   Paying caller compensation...");
        let caller_before = ctx.accounts.caller.lamports();
        **ctx.accounts.caller.to_account_info().try_borrow_mut_lamports()? += total_caller_compensation;
        **ctx.accounts.autominer_vault.to_account_info().try_borrow_mut_lamports()? -= total_caller_compensation;
        let caller_after = ctx.accounts.caller.lamports();
        msg!("     Caller: {} -> {} lamports (+{})", caller_before, caller_after, total_caller_compensation);
    }
    
    // Now borrow mutably to update state
    let autominer_vault = &mut ctx.accounts.autominer_vault;
    let current_round_id = global_state.current_round_id;
    
    // Mark bets as placed for this round
    autominer_vault.last_bet_round_id = current_round_id;
    msg!("   Updated last_bet_round_id: {} -> {}", last_bet_round_id, current_round_id);
    
    // Decrement rounds remaining
    let new_rounds_remaining = rounds_remaining
        .checked_sub(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    autominer_vault.rounds_remaining = new_rounds_remaining;
    msg!("   Updated rounds_remaining: {} -> {}", rounds_remaining, new_rounds_remaining);
    
    // If no rounds remaining, close vault and return remaining SOL
    if new_rounds_remaining == 0 {
        msg!("   No rounds remaining - closing vault and returning remaining SOL...");
        let rent = Rent::get()?.minimum_balance(AutominerVault::LEN);
        let remaining_sol = ctx.accounts.autominer_vault.to_account_info()
            .lamports()
            .checked_sub(rent)
            .ok_or(ErrorCode::InsufficientFunds)?;
        
        let owner_before = ctx.accounts.owner.lamports();
        **ctx.accounts.owner.to_account_info().try_borrow_mut_lamports()? += remaining_sol;
        **ctx.accounts.autominer_vault.to_account_info().try_borrow_mut_lamports()? -= remaining_sol;
        let owner_after = ctx.accounts.owner.lamports();
        msg!("     Owner: {} -> {} lamports (+{})", owner_before, owner_after, remaining_sol);
        msg!("     Vault closed");
    }
    
    // Place bets using join_round_batch
    msg!("   Placing {} bets for round {} using join_round_batch...", bet_types.len(), current_round_id);
    
    // Use internal join_round_batch logic but with autominer vault as payer
    let owner_key = ctx.accounts.autominer_vault.owner;
    
    // Expand bet types (handle FactionBoth and RandomBlock)
    let mut expanded_bet_types = Vec::new();
    for bet_type in bet_types.iter() {
        match bet_type {
            BetType::FactionBoth { faction_id } => {
                expanded_bet_types.push(BetType::FactionHighestLowest { 
                    faction_id: *faction_id, 
                    is_highest: true 
                });
                expanded_bet_types.push(BetType::FactionHighestLowest { 
                    faction_id: *faction_id, 
                    is_highest: false 
                });
            }
            BetType::RandomBlock => {
                let slot_bytes = clock.slot.to_le_bytes();
                let random_block = (slot_bytes[0] % 24) as u8;
                expanded_bet_types.push(BetType::Block { block_id: random_block });
            }
            _ => {
                expanded_bet_types.push(bet_type.clone());
            }
        }
    }
    
    msg!("     Expanded to {} bet types", expanded_bet_types.len());

    let mut target_blocks = Vec::new();
    let mut net_amounts = Vec::new();
    let mut fee_amounts = Vec::new();
    let mut points_amounts = Vec::new();
    // Place each bet using internal_join_round
    // Note: No faction validation needed - faction_state is not required for betting
    for (idx, bet_type) in expanded_bet_types.iter().enumerate() {
        msg!("     Placing bet {} of {}: {:?} for {} SOL", 
            idx + 1, 
            expanded_bet_types.len(), 
            bet_type, 
            (bet_size_per_bet as f64 / 1e9));
        
        // Call internal_join_round with autominer vault as payer
        let (target_block, net_amount, fee_amount, points_amount) = internal_join_round(
            &ctx.accounts.global_game_state,
            &ctx.accounts.global_config,
            &mut ctx.accounts.player_data,
            &mut ctx.accounts.game_session,
            &mut ctx.accounts.user_game_bet,
            &ctx.accounts.autominer_vault.to_account_info(),
            &ctx.accounts.sol_treasury.to_account_info(),
            &ctx.accounts.sol_prize_pot_vault.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            ctx.bumps.user_game_bet,
            owner_key,
            bet_size_per_bet,
            bet_type.clone(),
            None, // autominer always uses SOL, not tickets
        )?;
        
        target_blocks.push(target_block);
        net_amounts.push(net_amount);
        fee_amounts.push(fee_amount);
        points_amounts.push(points_amount);
        msg!("       ✓ Bet #{} placed successfully", idx + 1);

    }
    
    msg!("✅ [execute_autominer_bet] Autominer bets executed successfully");
    msg!("   {} bets of {} SOL each for round {}", 
        expanded_bet_types.len(), 
        (bet_size_per_bet as f64 / 1e9), 
        current_round_id);
    msg!("   Rounds remaining: {}", new_rounds_remaining);
    msg!("   Caller compensation: {} SOL", (total_caller_compensation as f64 / 1e9));
    
    emit!(AutominerBetExecuted {
        owner: owner_key,
        player_data: ctx.accounts.player_data.key(),
        autominer_vault: ctx.accounts.autominer_vault.key(),
        round_id: current_round_id,
        target_blocks,
        net_amounts,
        fee_amounts,
        points_amounts,
        caller: ctx.accounts.caller.key(),
        caller_compensation: total_caller_compensation,
        rounds_remaining: new_rounds_remaining,
        vault_closed: new_rounds_remaining == 0,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    Ok(())
}

/// Stop autominer and refund remaining SOL
/// Can only be called by vault owner
/// Refunds all remaining SOL (after rent) and resets rounds_remaining to 0
pub fn stop_autominer(ctx: Context<StopAutominer>) -> Result<()> {
    msg!("🛑 [stop_autominer] Stopping autominer");
    
    // Read values before mutable borrow
    let owner_key = ctx.accounts.autominer_vault.owner;
    let rounds_remaining = ctx.accounts.autominer_vault.rounds_remaining;
    let vault_lamports = ctx.accounts.autominer_vault.to_account_info().lamports();
    
    msg!("   Owner: {}", owner_key);
    
    // Verify caller is owner
    require!(
        ctx.accounts.authority.key() == owner_key,
        ErrorCode::Unauthorized
    );
    msg!("     ✓ Caller is owner");
    
    // Calculate remaining SOL (after rent)
    let rent = Rent::get()?.minimum_balance(AutominerVault::LEN);
    let remaining_sol = vault_lamports
        .checked_sub(rent)
        .ok_or(ErrorCode::InsufficientFunds)?;
    
    msg!("   Vault state:");
    msg!("     Rounds remaining: {}", rounds_remaining);
    msg!("     Vault lamports: {}", vault_lamports);
    msg!("     Rent: {}", rent);
    msg!("     Remaining SOL to refund: {}", remaining_sol);
    
    // Refund remaining SOL to owner
    if remaining_sol > 0 {
        msg!("   Refunding {} lamports to owner...", remaining_sol);
        let owner_before = ctx.accounts.owner.lamports();
        **ctx.accounts.owner.to_account_info().try_borrow_mut_lamports()? += remaining_sol;
        **ctx.accounts.autominer_vault.to_account_info().try_borrow_mut_lamports()? -= remaining_sol;
        let owner_after = ctx.accounts.owner.lamports();
        msg!("     Owner: {} -> {} lamports (+{})", owner_before, owner_after, remaining_sol);
    }
    
    // Now borrow mutably to update state
    let autominer_vault = &mut ctx.accounts.autominer_vault;
    
    // Reset vault state
    autominer_vault.rounds_remaining = 0;
    autominer_vault.last_bet_round_id = 0;
    autominer_vault.blocks_config = None;
    autominer_vault.factions_config = None;
    autominer_vault.sol_per_round = 0;
    msg!("   Reset vault state: rounds_remaining = 0, last_bet_round_id = 0");
    
    msg!("✅ [stop_autominer] Autominer stopped successfully");
    msg!("   Refunded {} lamports to owner", remaining_sol);
    
    emit!(AutominerStopped {
        owner: owner_key,
        player_data: ctx.accounts.player_data.key(),
        autominer_vault: ctx.accounts.autominer_vault.key(),
        rounds_remaining,
        refund_amount: remaining_sol,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    Ok(())
}

 
/// Claim rewards for a user after round ends
/// Checks if user won based on their bet type and the winning block
pub fn internal_claim_round_rewards(round_id: u64, ctx: Context<ClaimRoundRewards>) -> Result<()> {
    msg!("💰 [claim_rewards] User claiming rewards. User: {}", ctx.accounts.user_wallet.key());
    msg!("   Round ID: {}", round_id);
    
    let game_session = &ctx.accounts.game_session;
    let user_bet = &ctx.accounts.user_game_bet;
    let player_data = &mut ctx.accounts.player_data;

    // Round should be completely over before user can claim rewards
    require!( game_session.stage == 2, ErrorCode::InvalidStage );
    
    msg!("   User bet round ID: {}. GameSession round ID: {}", user_bet.round_id, game_session.round_id);    
    require!( round_id == user_bet.round_id && round_id == game_session.round_id, ErrorCode::InvalidRound);
        
    // Check which blocks user bet on and calculate rewards
    msg!("   User bet on {} blocks: {:?}", user_bet.block_ids.len(), user_bet.block_ids);
    msg!("   Winning block: {}. Follow-up block: {}", game_session.winning_block, game_session.same_faction_other_block);
    msg!("     Winning faction ID: {}", game_session.winning_faction_id);
        
    // Calculate rewards for each block user bet on
    let mut total_sol_reward = 0u64;
    let mut total_dbtc_reward = 0u64;
    
    for (idx, &block_id) in user_bet.block_ids.iter().enumerate() {
        let points_bet_on_block = user_bet.points_bets.get(idx).copied().unwrap_or(0);
        
        msg!("     Block {}: Points bet: {} SOL", block_id, points_bet_on_block as f64 / 1_000_000_000.0);
        
        let is_winning_block = block_id == game_session.winning_block;
        let is_same_faction_block = block_id == game_session.same_faction_other_block;
        
        if is_winning_block {
            msg!("       ✓ Winning block - calculating rewards...");
            
            // SOL rewards (only for winning block)
            if game_session.sol_rewards_index > 0 && points_bet_on_block > 0 {
                let sol_reward = helper::mul_div(points_bet_on_block, game_session.sol_rewards_index as u64, INDEX_PRECISION)? as u64;
                total_sol_reward += sol_reward;
                msg!("         SOL reward: {} lamports", sol_reward);
            }
            
            // DogeBtc rewards (winning block)
            if game_session.dbtc_rewards_index > 0 && points_bet_on_block > 0 {
                let dbtc_reward = helper::mul_div(points_bet_on_block, game_session.dbtc_rewards_index as u64, INDEX_PRECISION)? as u64;
                total_dbtc_reward += dbtc_reward;
                msg!("         DogeBtc reward: {} tokens", dbtc_reward);
            }
        } else if is_same_faction_block {
            msg!("       ✓ Same-faction other block - calculating DogeBtc rewards...");
            
            // DogeBtc rewards (same-faction other block)
            if game_session.same_faction_dbtc_rewards_index > 0 && points_bet_on_block > 0 {
                let dbtc_reward = helper::mul_div(points_bet_on_block, game_session.same_faction_dbtc_rewards_index as u64, INDEX_PRECISION)? as u64;
                total_dbtc_reward += dbtc_reward;
                msg!("         DogeBtc reward: {} tokens", dbtc_reward);
            }
        } else {
            msg!("       ✗ Not a winning or same-faction block - no rewards");
        }
    }
    
    msg!("   Total SOL reward: {} lamports", total_sol_reward);
    msg!("   Total DogeBtc reward: {} tokens", total_dbtc_reward);

    player_data.total_sol_won += total_sol_reward;
    msg!("     Total SOL won: {} (+{})", player_data.total_sol_won, total_sol_reward);
    msg!("     Total DogeBtc won: {} (+{})", player_data.total_dbtc_won, total_dbtc_reward);

    player_data.pending_sol_rewards += total_sol_reward;
    helper::add_to_total_claimable(&mut ctx.accounts.unrefined_rewards, player_data, total_dbtc_reward);
    msg!("     Pending SOL rewards: {} (+{})", player_data.pending_sol_rewards, total_sol_reward);
    msg!("     Pending DogeBtc rewards: {} (+{})", player_data.pending_dbtc_rewards, total_dbtc_reward);
        
    // Remove round from player's active rounds list
    msg!("   Removing round from player's active rounds list...");
    if let Some(index) = player_data.bets_rounds.iter().position(|&r| r == user_bet.round_id) {
        let old_count = player_data.bets_rounds.len();
        player_data.bets_rounds.remove(index);
        player_data.bets_points.remove(index);
        msg!("     Removed round {} from active rounds (count: {} -> {})", user_bet.round_id, old_count, player_data.bets_rounds.len());
    }
    
    // Close bet account and return rent
    msg!("   Closing bet account and returning rent...");
    let signer_key = ctx.accounts.user_wallet.key();
    let rent = Rent::get()?.minimum_balance(UserGameBet::LEN);
    **ctx.accounts.user_wallet.to_account_info().try_borrow_mut_lamports()? += rent;
    msg!("     Returned {} lamports rent to user", rent);
    
    msg!("✅ [claim_rewards] Rewards claimed successfully");
    msg!("   User: {}", signer_key);
    msg!("   Round: {}", user_bet.round_id);
    
    emit!(RoundRewardsClaimed {
        user: signer_key,
        player_data: ctx.accounts.player_data.key(),
        round_id: user_bet.round_id,
        sol_reward: total_sol_reward,
        dbtc_reward: total_dbtc_reward,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    Ok(())
}
 














/// Internal join_round logic that can be called by both user and autominer
/// Payer can be either user wallet or autominer vault PDA
/// Returns (net_amount, fee_amount, points_amount) for event emission
#[allow(clippy::too_many_arguments)]
fn internal_join_round<'info>(
    global_state: &Account<'info, GlobalGameSate>,
    global_config: &Account<'info, GlobalConfig>,
    player_data: &mut Account<'info, PlayerData>,
    game_session: &mut Account<'info, GameSession>,
    user_game_bet: &mut Account<'info, UserGameBet>,
    payer: &AccountInfo<'info>,
    sol_treasury: &AccountInfo<'info>,
    sol_prize_pot_vault: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    user_game_bet_bump: u8,
    owner_key: Pubkey,
    amount: u64,
    bet_type: BetType,
    use_ticket: Option<u8>,
) -> Result<(u8, u64, u64, u64)> {
    let clock = Clock::get()?;
    
    require!(game_session.round_id == global_state.current_round_id, ErrorCode::InvalidRound);
    require!(  game_session.block_assignments.iter().any(|&f| f != 0), ErrorCode::InvalidParameters);
    let round_id = global_state.current_round_id;
    msg!("   Current round ID: {}, Current timestamp: {}, Round end timestamp: {}", round_id, clock.unix_timestamp, global_state.round_end_timestamp);    
    require!(amount > 0 || use_ticket.is_some(), ErrorCode::InvalidAmount);
    
    // Validate bet type
    msg!("   Validating bet type...");
    let target_block = get_target_block_from_bet_type( &bet_type, &game_session.block_assignments)?;
    let target_faction = game_session.block_assignments[target_block as usize];
    msg!("     ✓ Target faction {}", target_faction);

    // Determine if using ticket or SOL
    let (fee_amount, net_amount, points_amount) = if let Some(ticket_type_index) = use_ticket {
        msg!("   Using ticket type index: {}", ticket_type_index);
        require!(  (ticket_type_index as usize) < player_data.free_tickets.len() && (ticket_type_index as usize) < player_data.free_tickets_remaining.len(), ErrorCode::InvalidParameters );
        
        let ticket_value = player_data.free_tickets[ticket_type_index as usize];
        require!(ticket_value > 0, ErrorCode::InvalidAmount);
        msg!("     Ticket value: {} points ({} SOL)", ticket_value, ticket_value as f64 / 1_000_000_000.0);
        
        require!( player_data.free_tickets_remaining[ticket_type_index as usize] > 0, ErrorCode::InsufficientFunds);
        msg!("     Tickets remaining: {}", player_data.free_tickets_remaining[ticket_type_index as usize]);
        
        require!(amount == ticket_value, ErrorCode::InvalidAmount);
        msg!("     ✓ Ticket amount matches ticket value");
        
        validate_points_percentage_limit(game_session.total_points_bets, game_session.total_sol_bets, amount)?;
        
        // Deduct ticket
        player_data.free_tickets_remaining[ticket_type_index as usize] -= 1;
        msg!("     ✓ Ticket deducted (remaining: {})", player_data.free_tickets_remaining[ticket_type_index as usize]);
        
        // Points bets don't have fees and don't go to prize pot
        (0, 0, amount)
    } else {
        require!(amount > 0, ErrorCode::InvalidAmount);
        msg!("   Using SOL bet. Bet amount: {} SOL", (amount as f64) / 1_000_000_000.0);
        
        // Calculate fees using protocol_fee_pct from GlobalConfig
        let (net, fee_amount) = handle_fee(amount, global_config.sol_fee_config.protocol_fee_pct as u64)?;

        // Calculate faction staker fees (split between dbtc and LP stakers)
        let stakers_fee = fee_amount * global_config.sol_fee_config.stakers_pct as u64 / M_HUNDRED;
        game_session.stakers_fee += stakers_fee;



        // Transfer remaining protocol fees to sol_treasury
        let protocol_fee = fee_amount - stakers_fee;
        if protocol_fee > 0 {
            msg!("   Transferring protocol fees ({} SOL) to sol_treasury", (protocol_fee as f64 / 1_000_000_000.0));
            helper::transfer_to_sol_treasury(payer, sol_treasury, system_program, protocol_fee)?;
            msg!("     ✓ Protocol fees transferred to sol_treasury");
        }    

        // Transfer net amount to prize pot
        msg!("   Transferring net amount ({} SOL) to sol_prize_pot_vault", (net as f64 / 1_000_000_000.0));
        helper::transfer_to_sol_prize_pot_vault(payer, sol_prize_pot_vault, system_program, net)?;
        msg!("     ✓ Net amount transferred to prize pot");
        
        (fee_amount, net, net)
    };

    // Initialize or update UserGameBet PDA
    msg!("   Processing user bet account...");
    let is_new_bet = user_game_bet.owner == Pubkey::default();
    if is_new_bet {
        user_game_bet.owner = owner_key;
        user_game_bet.round_id = round_id;
        user_game_bet.block_ids = Vec::new();
        user_game_bet.sol_bets = Vec::new();
        user_game_bet.points_bets = Vec::new();
        user_game_bet.total_sol_bet = 0;
        user_game_bet.total_points_bet = 0;
        user_game_bet.total_fee = 0;
        user_game_bet.bump = user_game_bet_bump;
        msg!("     ✓ New bet account initialized");
    } else {
        require!(
            user_game_bet.round_id == round_id,
            ErrorCode::InvalidRound
        );
        msg!("     ✓ Existing bet account found for round {}", round_id);
    }
    
    // Update block_ids, sol_bets, and points_bets vectors
    // Check if target_block is already in block_ids
    let block_index_in_user_bet = user_game_bet.block_ids.iter().position(|&b| b == target_block);
    
    if let Some(index) = block_index_in_user_bet {
        // Block already exists - update existing values
        msg!("     Block {} already in user bet, updating at index {}", target_block, index);
        user_game_bet.sol_bets[index] += net_amount;
        user_game_bet.points_bets[index] += points_amount;
        msg!("       Updated SOL bet: {})",  user_game_bet.sol_bets[index] as f64 / 1_000_000_000.0);
        msg!("       Updated points bet: {})",  user_game_bet.points_bets[index] as f64 / 1_000_000_000.0);
    } else {
        // New block - add to vectors
        msg!("     Adding new block {} to user bet", target_block);
        user_game_bet.block_ids.push(target_block);
        user_game_bet.sol_bets.push(net_amount);
        user_game_bet.points_bets.push(points_amount);
        msg!("       Added SOL bet: {}, points bet: {}", net_amount, points_amount);
    }
    
    // Update totals
    user_game_bet.total_sol_bet += net_amount;
    user_game_bet.total_points_bet += points_amount;
    user_game_bet.total_fee += fee_amount;
    msg!("     Total SOL bet: {} SOL. Total points bet: {} SOL. Total fee: {} SOL", 
        (user_game_bet.total_sol_bet as f64) / 1_000_000_000.0, 
        (user_game_bet.total_points_bet as f64) / 1_000_000_000.0,
        (user_game_bet.total_fee as f64) / 1_000_000_000.0);
    
    // Update block tracking arrays in GameSession (0-indexed: blocks 0-23)
    let block_index = target_block as usize;
    require!(block_index < NUM_BLOCKS, ErrorCode::InvalidParameters);
    
    // Only increment user count if this is a new bet for this block
    if block_index_in_user_bet.is_none() {
        game_session.user_block_indexes[block_index] += 1;
        msg!("     User count for block {}: {}", target_block, game_session.user_block_indexes[block_index]);
    }
    
    // Update SOL bet tracking in GameSession
    game_session.sol_bets_indexes[block_index] += net_amount;
    game_session.points_bets_indexes[block_index] += points_amount;
    game_session.total_sol_bets += net_amount;
    game_session.total_points_bets += points_amount;
    msg!("     SOL bet for block {}: {} (total: {})", target_block, net_amount, game_session.sol_bets_indexes[block_index]);
    msg!("     Points bet for block {}: {} (total: {})", target_block, points_amount, game_session.points_bets_indexes[block_index]);

    // Update PlayerData to track this round
    msg!("   Updating PlayerData for round {}...", round_id);
    if !player_data.bets_rounds.contains(&round_id) {
        player_data.rounds_played += 1;
        player_data.bets_rounds.push(round_id);
        player_data.bets_points.push(0);
        msg!("     Added round {} to player's active rounds", round_id);
    }
    
    // Update the bet amount for this round in PlayerData
    if let Some(index) = player_data.bets_rounds.iter().position(|&r| r == round_id) {
        player_data.bets_points[index] += points_amount;
        msg!("     Player bet amount for round {}: {} SOL", round_id, (player_data.bets_points[index] as f64) / 1_000_000_000.0);
    }
    
    // Update cumulative statistics
    player_data.total_sol_bet += net_amount;
    player_data.total_points_bet += points_amount;
    msg!("     Player total points bet: {} SOL", (player_data.total_points_bet as f64) / 1_000_000_000.0);
    msg!("     Player total SOL bet: {} SOL", (player_data.total_sol_bet as f64) / 1_000_000_000.0);
    
    msg!("   ✓ Bet placed: {} SOL on block {} (bet_type: {:?})", (amount as f64) / 1_000_000_000.0, target_block, bet_type);
     
    Ok((target_block, net_amount, fee_amount, points_amount))
}
 
  

/// Get the target block ID from bet_type (0-indexed: 0-23)
/// For Block bets, returns the block_id directly (0-indexed)
/// For FactionHighestLowest bets, finds the faction's blocks and returns highest/lowest (0-indexed)
fn get_target_block_from_bet_type(bet_type: &BetType, block_assignments: &[u8; NUM_BLOCKS]) -> Result<u8> {
    match bet_type {
        BetType::Block { block_id } => {
            require!(*block_id < NUM_BLOCKS as u8, ErrorCode::InvalidParameters);
            Ok(*block_id)
        }
        BetType::FactionHighestLowest { faction_id, is_highest } => {
            require!((*faction_id as usize) < block_assignments.len(), ErrorCode::InvalidParameters);
            // Find the two blocks assigned to this faction (0-indexed)
            let mut faction_blocks: Vec<u8> = Vec::new();
            for (block_idx, assigned_faction) in block_assignments.iter().enumerate() {
                if *assigned_faction == *faction_id {
                    faction_blocks.push(block_idx as u8); // 0-indexed (0-23)
                }
            }
            
            require!(
                faction_blocks.len() == BLOCKS_PER_FACTION as usize,
                ErrorCode::InvalidParameters
            );
            
            if *is_highest {
                Ok(*faction_blocks.iter().max().unwrap())
            } else {
                Ok(*faction_blocks.iter().min().unwrap())
            }
        }
        BetType::FactionBoth { faction_id } => {
            // For "both", return the highest block (will be expanded in batch function)
            require!((*faction_id as usize) < block_assignments.len(), ErrorCode::InvalidParameters);
            let mut faction_blocks: Vec<u8> = Vec::new();
            for (block_idx, assigned_faction) in block_assignments.iter().enumerate() {
                if *assigned_faction == *faction_id {
                    faction_blocks.push(block_idx as u8); // 0-indexed (0-23)
                }
            }
            require!(
                faction_blocks.len() == BLOCKS_PER_FACTION as usize,
                ErrorCode::InvalidParameters
            );
            Ok(*faction_blocks.iter().max().unwrap()) // Return highest, but will be expanded
        }
        BetType::RandomBlock => {
            // Random block - use clock slot for randomness (0-indexed: 0-23)
            let clock = Clock::get()?;
            let slot_bytes = clock.slot.to_le_bytes();
            let random_block = ((slot_bytes[0] % 24)) as u8; // 0-23
            Ok(random_block)
        }
    }
}

fn handle_fee(amount: u64, protocol_fee_pct: u64) -> Result<(u64, u64)> {
    let fee = amount * protocol_fee_pct / M_HUNDRED;
    let net_amount = amount - fee;
    msg!("     Net amount (after fee): {} SOL. Protocol fee ({}%): {} SOL", (net_amount as f64) / 1_000_000_000.0, protocol_fee_pct, (fee as f64) / 1_000_000_000.0);
    return Ok((net_amount, fee));
}


 

fn validate_points_percentage_limit(current_points_bets: u64, current_sol_bets: u64, amount: u64) -> Result<()> {
        // Validate points percentage limit: points bets must stay at or below 25% of SOL bets for this session
        // Tickets can only be used when: (total_points_bets + ticket_amount) <= (total_sol_bets * 25 / 100)
        let new_points_bets = current_points_bets + amount;        
        msg!("     Current session stats: SOL bets: {} lamports, Points bets: {} lamports, New points bets if allowed: {} lamports", current_sol_bets, current_points_bets, new_points_bets);
        
        // Require that SOL bets exist before allowing ticket bets -  This ensures points percentage can be calculated and stays within 25% limit
        require!(  current_sol_bets > 0,  ErrorCode::InvalidParameters);
        msg!("     ✓ SOL bets exist in session");
        
        // Calculate max allowed points bets (25% of SOL bets) -  This ensures points percentage can be calculated and stays within 25% limit
        let max_allowed_points = current_sol_bets * 25 / 100;        
        msg!("       Max allowed points (25% of SOL): {} lamports", max_allowed_points);
        require!( new_points_bets <= max_allowed_points, ErrorCode::InvalidParameters);
        msg!("     ✓ Points bets stay within 25% limit");
        Ok(())
}

 


 



// Compute blocks to bet on based on blocks_config and clock
fn compute_blocks_to_bet(blocks_config: Option<BlocksConfig>, clock: &Clock) -> Result<Option<Vec<u8>>> {
    let blocks_to_bet = if let Some(ref blocks_cfg) = blocks_config {
        match blocks_cfg {
            BlocksConfig::Specific { blocks } => Some(blocks.clone()),
            BlocksConfig::Random { count } => {
            // Generate random blocks using slot hash
            let mut random_blocks = Vec::new();
            let mut used_blocks = [false; 24];
            let mut attempts: u64 = 0;
            while random_blocks.len() < *count as usize && attempts < 100 {
                let slot_bytes = clock.slot.to_le_bytes();
                let hash = keccak::hash(&[slot_bytes, attempts.to_le_bytes()].concat());
                let block_id = (hash.0[0] % 24) as u8;
                if !used_blocks[block_id as usize] {
                    random_blocks.push(block_id);
                    used_blocks[block_id as usize] = true;
                }
                attempts += 1;
            }
                require!(random_blocks.len() == *count as usize, ErrorCode::InvalidParameters);
                Some(random_blocks)
            }
        }
    } else {
        None
    };

    if let Some(ref blocks) = blocks_to_bet {
        msg!("     Blocks to bet on: {:?}", blocks);
    }

    Ok(blocks_to_bet)
}


/// Generate bet types from blocks_config and factions_config
/// Returns vector of bet types to place
fn make_bets_vec<'info>(
    factions_config: Option<FactionsConfig>,
    blocks_to_bet: Option<Vec<u8>>,
    game_session: &Account<'info, GameSession>,
    clock: &Clock,
    global_config: &Account<'info, GlobalConfig>,
) -> Result<Vec<BetType>> {
    let mut bet_types = Vec::new();

    // Generate faction bets if factions_config is provided
    if let Some(ref factions_cfg) = factions_config {
        let factions_to_bet = match factions_cfg {
            FactionsConfig::Specific { factions, .. } => factions.clone(),
            FactionsConfig::Random { count, .. } => {
                // Generate random factions
                let mut random_factions = Vec::new();
                let mut used_factions = [false; 12];
                let mut attempts: u64 = 0;
                let max_factions = global_config.supported_factions.len();
                while random_factions.len() < *count as usize && attempts < 100 {
                    let slot_bytes = clock.slot.to_le_bytes();
                    let hash = keccak::hash(&[slot_bytes, (attempts + 100u64).to_le_bytes()].concat());
                    let faction_id = (hash.0[0] % max_factions as u8) as u8;
                    if !used_factions[faction_id as usize] && (faction_id as usize) < max_factions {
                        random_factions.push(faction_id);
                        used_factions[faction_id as usize] = true;
                    }
                    attempts += 1;
                }
                require!(random_factions.len() == *count as usize, ErrorCode::InvalidParameters);
                random_factions
            }
        };
        
        let strategy = match factions_cfg {
            FactionsConfig::Specific { strategy, .. } => strategy,
            FactionsConfig::Random { strategy, .. } => strategy,
        };
        
        msg!("     Factions to bet on: {:?} (strategy: {:?})", factions_to_bet, strategy);
        
        // Generate bet types for each faction
        // If blocks_to_bet provided, bet on those blocks for each faction
        // If no blocks_to_bet, bet on all blocks assigned to those factions in current round
        if let Some(ref blocks) = blocks_to_bet {
            // Blocks config provided - bet on specified blocks for each faction
            for &faction_id in factions_to_bet.iter() {
                match strategy {
                    FactionStrategy::Highest => {
                        for _block_id in blocks.iter() {
                            bet_types.push(BetType::FactionHighestLowest { 
                                faction_id, 
                                is_highest: true 
                            });
                        }
                    }
                    FactionStrategy::Lowest => {
                        for _block_id in blocks.iter() {
                            bet_types.push(BetType::FactionHighestLowest { 
                                faction_id, 
                                is_highest: false 
                            });
                        }
                    }
                    FactionStrategy::Both => {
                        for _block_id in blocks.iter() {
                            bet_types.push(BetType::FactionHighestLowest { 
                                faction_id, 
                                is_highest: true 
                            });
                            bet_types.push(BetType::FactionHighestLowest { 
                                faction_id, 
                                is_highest: false 
                            });
                        }
                    }
                }
            }
        } else {
            // No blocks config - bet on all blocks assigned to selected factions in current round
            for &faction_id in factions_to_bet.iter() {
                for block_id in 0..NUM_BLOCKS as u8 {
                    if game_session.block_assignments[block_id as usize] == faction_id {
                        match strategy {
                            FactionStrategy::Highest => {
                                bet_types.push(BetType::FactionHighestLowest { 
                                    faction_id, 
                                    is_highest: true 
                                });
                            }
                            FactionStrategy::Lowest => {
                                bet_types.push(BetType::FactionHighestLowest { 
                                    faction_id, 
                                    is_highest: false 
                                });
                            }
                            FactionStrategy::Both => {
                                bet_types.push(BetType::FactionHighestLowest { 
                                    faction_id, 
                                    is_highest: true 
                                });
                                bet_types.push(BetType::FactionHighestLowest { 
                                    faction_id, 
                                    is_highest: false 
                                });
                            }
                        }
                    }
                }
            }
        }
    } else if let Some(ref blocks) = blocks_to_bet {
        // Only blocks config - bet on blocks directly
        for &block_id in blocks.iter() {
            bet_types.push(BetType::Block { block_id });
        }
    } else {
        return Err(ErrorCode::InvalidParameters.into());
    }

    Ok(bet_types)
}

/// Calculate caller compensation: 1% of sol_per_round, max 0.005 SOL
fn get_caller_compensation(sol_per_round: u64) -> Result<u64> {
    let caller_compensation = (sol_per_round / 100).min(crate::state::MAX_CALLER_COMPENSATION);
    Ok(caller_compensation)
}


 


// ========================================================================================
// =============================== ACCOUNT CONTEXTS ======================================
// ========================================================================================

#[derive(Accounts)]
#[instruction(faction_id: u8, referral_code: Option<Pubkey>)]
pub struct InitializePlayer<'info> {
    #[account(
        init,
        payer = authority,
        space = PlayerData::LEN,
        seeds = [PLAYER_DATA_SEED.as_ref(), authority.key().as_ref()],
        bump
    )]
    pub player_data: Account<'info, PlayerData>,
    
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    /// Optional: Referrer's referral rewards account (if referral code is provided)
    /// CHECK: Validated manually that owner matches referral_code pubkey
    #[account(mut)]
    pub referrer_rewards: Option<Account<'info, ReferralRewards>>,
    
    #[account(
        init,
        payer = authority,
        space = ReferralRewards::LEN,
        seeds = [REFERRAL_REWARDS_SEED.as_ref(), authority.key().as_ref()],
        bump
    )]
    pub new_player_rewards: Account<'info, ReferralRewards>,

    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ChangeFaction<'info> {
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), authority.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == authority.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,
    
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    /// CHECK: SOL treasury PDA (50% of fee goes here)
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump
    )]
    pub sol_treasury: UncheckedAccount<'info>,
    
    /// Multisig WSOL token account (destination for WSOL transfers)
    /// MUST be owned by global_config.fee_recipient (the multisig address)
    #[account(
        mut,
        constraint = multisig_wsol_account.mint == wsol_mint.key() @ ErrorCode::InvalidMint,
        constraint = multisig_wsol_account.owner == global_config.fee_recipient @ ErrorCode::Unauthorized
    )]
    pub multisig_wsol_account: Account<'info, anchor_spl::token::TokenAccount>,
    
    /// User's WSOL token account (for wrapping SOL to WSOL)
    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = wsol_mint,
        associated_token::authority = authority,
    )]
    pub user_wsol_account: Account<'info, anchor_spl::token::TokenAccount>,
    
    /// CHECK: WSOL mint
    #[account(
        constraint = wsol_mint.key() == anchor_spl::token::spl_token::native_mint::id() @ ErrorCode::InvalidMint
    )]
    pub wsol_mint: UncheckedAccount<'info>,
    
    #[account(mut)]
    pub user_wallet: Signer<'info>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct JoinRound<'info> {
    #[account(
        seeds = [GLOBAL_GAME_STATE_SEED.as_ref()],
        bump = global_game_state.bump
    )]
    pub global_game_state: Account<'info, GlobalGameSate>,
    
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), authority.key().as_ref()],
        bump = player_data.bump
    )]
    pub player_data: Account<'info, PlayerData>,
        
    /// GameSession PDA for the current round (must be initialized by crank function)
    #[account(
        mut,
        seeds = [GAME_SESSION_SEED.as_ref(), &global_game_state.current_round_id.to_le_bytes()],
        bump = game_session.bump
    )]
    pub game_session: Account<'info, GameSession>,
    
    /// UserGameBet PDA for this user's bet in this round
    #[account(
        init_if_needed,
        payer = authority,
        space = UserGameBet::LEN,
        seeds = [USER_GAME_BET_SEED.as_ref(), authority.key().as_ref(), &global_game_state.current_round_id.to_le_bytes()],
        bump
    )]
    pub user_game_bet: Account<'info, UserGameBet>,
    
    /// CHECK: SOL treasury PDA (fees go here)
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump
    )]
    pub sol_treasury: UncheckedAccount<'info>,
    
    /// CHECK: SOL prize pot vault (PDA)
    #[account(
        mut,
        seeds = [SOL_PRIZE_POT_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_prize_pot_vault: UncheckedAccount<'info>,
        
    #[account(mut)]
    pub user_wallet: Signer<'info>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

/// Account struct for batch betting
/// Note: All bets must be for the same faction (same faction_state account)
#[derive(Accounts)]
pub struct JoinRoundBatch<'info> {
    #[account(
        seeds = [GLOBAL_GAME_STATE_SEED.as_ref()],
        bump = global_game_state.bump
    )]
    pub global_game_state: Account<'info, GlobalGameSate>,
    
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), authority.key().as_ref()],
        bump = player_data.bump
    )]
    pub player_data: Account<'info, PlayerData>,
        
    /// GameSession PDA for the current round
    #[account(
        mut,
        seeds = [GAME_SESSION_SEED.as_ref(), &global_game_state.current_round_id.to_le_bytes()],
        bump = game_session.bump
    )]
    pub game_session: Account<'info, GameSession>,
    
    /// UserGameBet PDA (shared across all bets in batch)
    #[account(
        init_if_needed,
        payer = authority,
        space = UserGameBet::LEN,
        seeds = [USER_GAME_BET_SEED.as_ref(), authority.key().as_ref(), &global_game_state.current_round_id.to_le_bytes()],
        bump
    )]
    pub user_game_bet: Account<'info, UserGameBet>,
    
    /// CHECK: SOL treasury PDA (fees go here)
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump
    )]
    pub sol_treasury: UncheckedAccount<'info>,
    
    /// CHECK: SOL prize pot vault (PDA)
    #[account(
        mut,
        seeds = [SOL_PRIZE_POT_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_prize_pot_vault: UncheckedAccount<'info>,
        
    #[account(mut)]
    pub user_wallet: Signer<'info>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}


#[derive(Accounts)]
#[instruction(round_id: u64)]
pub struct ClaimRoundRewards<'info> {
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user_wallet.key().as_ref()],
        bump = player_data.bump
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(
        mut,
        seeds = [UNREFINED_REWARDS_SEED.as_ref()],
        bump
    )]
    pub unrefined_rewards: Account<'info, UnrefinedRewards>,

    #[account(
        seeds = [GAME_SESSION_SEED.as_ref(), &round_id.to_le_bytes()],
        bump = game_session.bump
    )]
    pub game_session: Account<'info, GameSession>,
    
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
        
    /// CHECK: UserGameBet PDA (validated in instruction)
    #[account(
        mut,
        close = user_wallet
    )]
    pub user_game_bet: Account<'info, UserGameBet>,
                    
    /// User whose bet this is (doesn't need to be signer - anyone can claim for them)
    /// CHECK: Validated via player_data.owner matching user_wallet
    #[account(mut)]
    pub user_wallet: UncheckedAccount<'info>,
    
    /// Caller (bot or user themselves) - can be anyone
    pub caller: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(blocks_config: Option<BlocksConfig>, factions_config: Option<FactionsConfig>, sol_per_round: u64, num_rounds: u32)]
pub struct InitAutominer<'info> {
    #[account(
        init_if_needed,
        payer = user_wallet,
        space = AutominerVault::LEN,
        seeds = [AUTOMINER_VAULT_SEED.as_ref(), user_wallet.key().as_ref()],
        bump
    )]
    pub autominer_vault: Account<'info, AutominerVault>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(
        seeds = [PLAYER_DATA_SEED.as_ref(), user_wallet.key().as_ref()],
        bump
    )]
    pub player_data: Account<'info, PlayerData>,
    
    #[account(mut)]
    pub user_wallet: Signer<'info>,
        
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct StopAutominer<'info> {
    #[account(
        mut,
        seeds = [AUTOMINER_VAULT_SEED.as_ref(), autominer_vault.owner.as_ref()],
        bump = autominer_vault.vault_bump
    )]
    pub autominer_vault: Account<'info, AutominerVault>,
    
    #[account(
        seeds = [PLAYER_DATA_SEED.as_ref(), autominer_vault.owner.as_ref()],
        bump
    )]
    pub player_data: Account<'info, PlayerData>,
    
    /// CHECK: Owner account (to receive refunded SOL)
    #[account(
        mut,
        constraint = owner.key() == autominer_vault.owner @ ErrorCode::Unauthorized
    )]
    pub owner: UncheckedAccount<'info>,
    
    /// Authority (must be owner)
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ExecuteAutominerBet<'info> {
    #[account(
        mut,
        seeds = [AUTOMINER_VAULT_SEED.as_ref(), autominer_vault.owner.as_ref()],
        bump = autominer_vault.vault_bump
    )]
    pub autominer_vault: Account<'info, AutominerVault>,

    #[account(
        mut,
        seeds = [GLOBAL_GAME_STATE_SEED.as_ref()],
        bump = global_game_state.bump
    )]
    pub global_game_state: Account<'info, GlobalGameSate>,
    
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), autominer_vault.owner.as_ref()],
        bump
    )]
    pub player_data: Account<'info, PlayerData>,
        
    #[account(
        mut,
        seeds = [GAME_SESSION_SEED.as_ref(), &global_game_state.current_round_id.to_le_bytes()],
        bump
    )]
    pub game_session: Account<'info, GameSession>,
    
    /// UserGameBet PDA for autominer bets (aggregates all bets from this vault for this round)
    #[account(
        init_if_needed,
        payer = caller,
        space = UserGameBet::LEN,
        seeds = [USER_GAME_BET_SEED.as_ref(), autominer_vault.owner.as_ref(), &global_game_state.current_round_id.to_le_bytes()],
        bump
    )]
    pub user_game_bet: Account<'info, UserGameBet>,
    
    /// CHECK: SOL treasury PDA
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump
    )]
    pub sol_treasury: UncheckedAccount<'info>,
    
    /// CHECK: SOL prize pot vault
    #[account(
        mut,
        seeds = [SOL_PRIZE_POT_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_prize_pot_vault: UncheckedAccount<'info>,
        
    /// CHECK: Owner account (to receive remaining SOL when vault closes)
    #[account(
        mut,
        constraint = owner.key() == autominer_vault.owner @ ErrorCode::Unauthorized
    )]
    pub owner: UncheckedAccount<'info>,
    
    /// Caller (bot or anyone) - doesn't need to be owner
    #[account(mut)]
    pub caller: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}
 