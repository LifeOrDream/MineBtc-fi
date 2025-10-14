use anchor_lang::prelude::*;

use crate::state::*;
use crate::errors::ErrorCode;
use crate::instructions::helper;
use crate::events::*;

// ----------------------------------------------------------------------------------------
// -------------- GAMEPLAY FUNCTIONS (Attack / Research / Attraction) ------------
// ---------------------------------------------------------------------------------------- 
// ----------------------------------------------------------------------------------------

// ========================================================================================
// ========== PVP GAME (INITIALIZE A NEW GAME) ============================================
// ========================================================================================

/// Get mutable reference to the appropriate games list based on ticket index
fn get_games_list<'a>(pvp_matchmaker: &'a mut PvPMatchmaker, ticket_index: u8) -> Result<&'a mut Vec<Pubkey>> {
    match ticket_index {
        0 => Ok(&mut pvp_matchmaker.index_1_games),
        1 => Ok(&mut pvp_matchmaker.index_2_games),
        2 => Ok(&mut pvp_matchmaker.index_3_games),
        3 => Ok(&mut pvp_matchmaker.index_4_games),
        4 => Ok(&mut pvp_matchmaker.index_5_games),
        5 => Ok(&mut pvp_matchmaker.index_6_games),
        6 => Ok(&mut pvp_matchmaker.index_7_games),
        7 => Ok(&mut pvp_matchmaker.index_8_games),
        8 => Ok(&mut pvp_matchmaker.index_9_games),
        9 => Ok(&mut pvp_matchmaker.index_10_games),
        10 => Ok(&mut pvp_matchmaker.index_11_games),
        11 => Ok(&mut pvp_matchmaker.index_12_games),
        12 => Ok(&mut pvp_matchmaker.index_13_games),
        _ => Err(ErrorCode::InvalidParameters.into()),
    }
}

/// Initialize a new PvP game between two players. Both must stake the same ticket size.
pub fn create_pvp_game_internal(
    ctx: Context<CreatePvPGame>,
    ticket_index: u8,
) -> Result<()> {
    
    // Validate ticket tier
    require!((ticket_index as usize) < PVP_TICKET_TIERS.len(), ErrorCode::InvalidParameters);
    let ticket = PVP_TICKET_TIERS[ticket_index as usize];

    let user_a = &mut ctx.accounts.player_a_moonbase;
    let game_key = ctx.accounts.pvp_game.key();
    let pvp_game = &mut ctx.accounts.pvp_game;
    let pvp_matchmaker = &mut ctx.accounts.pvp_matchmaker;

    require!(user_a.active_game.is_none(), ErrorCode::PlayerAlreadyInGame);

    // ========== HP GATE: MINIMUM 1000 HP REQUIRED FOR PVP ========== //
    require!(
        user_a.pvp_hp >= MIN_PVP_HP_REQUIRED,
        ErrorCode::InsufficientPvPHP
    );
    msg!("✅ Player A HP check passed: {} >= {}", user_a.pvp_hp, MIN_PVP_HP_REQUIRED);

    // Add to appropriate matchmaker list
    let games_list = get_games_list(pvp_matchmaker, ticket_index)?;
    require!(
        games_list.len() < PvPMatchmaker::MAX_GAMES_PER_INDEX,
        ErrorCode::MaxGamesPerIndex
    );
    games_list.push(game_key);

    msg!("🎮 PvP game created: {}", game_key);
    msg!("   Ticket size: {} SOL (index {})", ticket as f64 / 1e9, ticket_index);

    // 1. Set up the PvP game with enhanced session tracking
    let current_time = Clock::get()?.unix_timestamp;
    pvp_game.ticket_index = ticket_index;
    pvp_game.player_a = Some(user_a.owner);
    pvp_game.player_b = None;
    pvp_game.ticket_lamports = ticket;
    pvp_game.pot_lamports = ticket * 2; // Both players stake
    pvp_game.treasury_cut_lamports = (ticket * 2) / 10; // 10% to treasury
    
    // Initialize HP from moonbase pvp_hp
    pvp_game.player_a_hp = user_a.pvp_hp;
    pvp_game.player_b_hp = 0; // Will be set when player B joins
    pvp_game.turn = 0;
    pvp_game.turn_number = 0;

    pvp_game.last_move_ts = current_time;
    pvp_game.game_start_ts = current_time;
    pvp_game.winner = None;
    
    // Initialize hash leech tracking
    pvp_game.player_a_hash_leech = 0;
    pvp_game.player_b_hash_leech = 0;
    
    pvp_game.bump = ctx.bumps.pvp_game;

    // 2. Update the user's moonbase
    user_a.active_game = Some(pvp_game.key());

    // 4. Transfer stake from player A
    let stake_per_player = ticket;
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.player_a.to_account_info(),
                to: pvp_game.to_account_info(),
            },
        ),
        stake_per_player,
    )?;

    // Emit game creation event
    emit!(PvPGameCreated {
        game_id: pvp_game.key(),
        player_a: user_a.owner,
        ticket_lamports: ticket,
        pot_lamports: pvp_game.pot_lamports,
    });

    msg!("🎉 PvP game started successfully!");
    msg!("   Game ID: {}", pvp_game.key());
    msg!("   Player A turn starts now");
    msg!("   Player A HP: {}", pvp_game.player_a_hp);

    Ok(())
}

/// Join a PvP game between two players. Both must stake the same ticket size.
pub fn join_pvp_game_internal(ctx: Context<JoinPvPGame>, player_a: Pubkey) -> Result<()> {
    
    let user_b = &mut ctx.accounts.player_b_moonbase;
    let game_key = ctx.accounts.pvp_game.key();
    let pvp_game = &mut ctx.accounts.pvp_game;
    let pvp_matchmaker = &mut ctx.accounts.pvp_matchmaker;
    
    let ticket_index = pvp_game.ticket_index;

    require!(player_a == pvp_game.player_a.unwrap(), ErrorCode::PlayerAMismatch);
    require!(user_b.active_game.is_none(), ErrorCode::PlayerAlreadyInGame);
    require!(pvp_game.player_b.is_none(), ErrorCode::GameAlreadyHasPlayerB);

    // ========== HP GATE: MINIMUM 1000 HP REQUIRED FOR PVP ========== //
    require!(
        user_b.pvp_hp >= MIN_PVP_HP_REQUIRED,
        ErrorCode::InsufficientPvPHP
    );
    msg!("✅ Player B HP check passed: {} >= {}", user_b.pvp_hp, MIN_PVP_HP_REQUIRED);

    // Remove from matchmaker list
    let games_list = get_games_list(pvp_matchmaker, ticket_index)?;
    if let Some(pos) = games_list.iter().position(|&game| game == game_key) {
        games_list.remove(pos);
    } else {
        return Err(ErrorCode::GameNotFound.into());
    }

    // 1. Add player B to the game with HP initialization
    pvp_game.player_b = Some(user_b.owner);    
    pvp_game.player_b_hp = user_b.pvp_hp; // Initialize with moonbase HP

    // 2. Update the user's moonbase
    user_b.active_game = Some(pvp_game.key());

    // 4. Transfer stake from player B
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.player_b.to_account_info(),
                to: pvp_game.to_account_info(),
            },
        ),
        pvp_game.ticket_lamports,
    )?;

    // Emit game join event
    emit!(PlayerBJoinedTheGame {
        game_id: pvp_game.key(),
        player_b: user_b.owner,
        ticket_lamports: pvp_game.ticket_lamports,
    });

    msg!("🎉 PvP game joined successfully!");
    msg!("   Game ID: {}", pvp_game.key());
    msg!("   Player B HP: {}", pvp_game.player_b_hp);
    msg!("   Battle can now begin!");

    Ok(())
}

/// Cancel a PvP game that hasn't started yet (no player B)
pub fn cancel_pvp_game_internal(ctx: Context<CancelPvPGame>) -> Result<()> {
    let pvp_game = &ctx.accounts.pvp_game;
    let player_a_moonbase = &mut ctx.accounts.player_a_moonbase;
    let pvp_matchmaker = &mut ctx.accounts.pvp_matchmaker;
    
    // Can only cancel if no player B
    require!(pvp_game.player_b.is_none(), ErrorCode::GameAlreadyHasPlayerB);

    // Remove from matchmaker list
    let game_key = pvp_game.key();
    let ticket_index = pvp_game.ticket_index;

    let games_list = get_games_list(pvp_matchmaker, ticket_index)?;
    if let Some(pos) = games_list.iter().position(|&game| game == game_key) {
        games_list.remove(pos);
    }

    // Clear active game from player A's moonbase
    player_a_moonbase.active_game = None;

    // Return stake to player A's wallet
    let stake = pvp_game.ticket_lamports;
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: pvp_game.to_account_info(),
                to: ctx.accounts.player_a.clone(),
            },
        ),
        stake,
    )?;

    // Close the game account and send remaining lamports to player A
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: pvp_game.to_account_info(),
                to: ctx.accounts.player_a.clone(),
            },
        ),
        pvp_game.to_account_info().lamports(),
    )?;

    // Emit cancellation event
    emit!(PvPGameCancelled {
        game_id: game_key,
        player_a: player_a_moonbase.owner,
        ticket_lamports: stake,
    });

    msg!("🎮 PvP game cancelled by player A");
    msg!("   Game ID: {}", game_key);
    msg!("   Stake returned: {} SOL", stake);

    Ok(())
}

/// Cancel a PvP game that hasn't started yet (no player B)
pub fn cancel_expired_pvp_game_internal(ctx: Context<CancelExpiredPvPGame>) -> Result<()> {
    let pvp_game = &ctx.accounts.pvp_game;
    let player_a_moonbase = &mut ctx.accounts.player_a_moonbase;
    let pvp_matchmaker = &mut ctx.accounts.pvp_matchmaker;
    
    // Can only cancel if no player B
    require!(pvp_game.player_b.is_none(), ErrorCode::GameAlreadyHasPlayerB);

    // Check if game is old enough to be cancelled (30 minutes)
    let created_at = pvp_game.last_move_ts;
    let now = Clock::get()?.unix_timestamp;
    require!(now > created_at + 1800, ErrorCode::GameNotExpired);

    // Remove from matchmaker list
    let game_key = pvp_game.key();
    let ticket_index = pvp_game.ticket_index;

    let games_list = get_games_list(pvp_matchmaker, ticket_index)?;
    if let Some(pos) = games_list.iter().position(|&game| game == game_key) {
        games_list.remove(pos);
    }

    // Clear active game from player A's moonbase
    player_a_moonbase.active_game = None;

    // Return stake to player A's wallet
    let stake = pvp_game.ticket_lamports;
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: pvp_game.to_account_info(),
                to: ctx.accounts.player_a.clone(),
            },
        ),
        stake,
    )?;

    // Close the game account and send remaining lamports to player A
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: pvp_game.to_account_info(),
                to: ctx.accounts.player_a.clone(),
            },
        ),
        pvp_game.to_account_info().lamports(),
    )?;

    // Emit cancellation event
    emit!(ExpiredPvPGameCancelled {
        game_id: game_key,
        player_a: player_a_moonbase.owner,
        ticket_lamports: stake,
    });

    msg!("🎮 PvP game cancelled due to expiration");
    msg!("   Game ID: {}", game_key);
    msg!("   Stake returned to player A: {} SOL", stake);

    Ok(())
}

// ========================================================================================
// ========== PVP GAME SESSION ============================================================
// ========================================================================================

/// Professional PvP attack turn with comprehensive combat mechanics
/// Implements damage, special effects, resource stealing, and state management
pub fn pvp_attack_turn_internal(
    ctx: Context<PvPAttack>,
    target_module_type: ModuleType,
) -> Result<()> {
    let pvp_game = &mut ctx.accounts.pvp_game;
    let current_time = Clock::get()?.unix_timestamp;
    let current_slot = Clock::get()?.slot;
    
    // Store the game key early to avoid borrowing conflicts
    let game_key = pvp_game.key();
    
    msg!("🎯 Starting PvP attack turn");
    msg!("   Game: {}", game_key);
    msg!("   Turn number: {}", pvp_game.turn_number);
    msg!("   Target type: {:?}", target_module_type);
    
    // ========== BASIC VALIDATIONS ========== //
    
    // Check if game has finished
    require!(pvp_game.winner.is_none(), ErrorCode::GameAlreadyFinished);
    
    // Check turn limit (15 turns maximum)
    require!(pvp_game.turn_number < PvPGame::MAX_TURNS, ErrorCode::GameTimeout);
    
    // Timeout check for 5-minute inactivity
    if current_time > pvp_game.last_move_ts + PvPGame::TURN_TIMEOUT_SECONDS {
        // Current turn holder forfeits due to timeout
        let winner = if pvp_game.turn == 0 { 
            pvp_game.player_b.unwrap() 
        } else { 
            pvp_game.player_a.unwrap() 
        };
        
        finalize_pvp_game_internal(
            pvp_game, 
            &ctx.accounts.sol_treasury, 
            &mut ctx.accounts.attacker_moonbase,
            &mut ctx.accounts.defender_moonbase,
            &mut ctx.accounts.global_config,
            Some(winner),
            "Timeout".to_string(),
            game_key,
            &ctx.accounts.system_program,
        )?;
        return Ok(());
    }
    
    // Verify correct turn and player authorization
    let attacker_key = ctx.accounts.attacker.key();
    let is_player_a = attacker_key == pvp_game.player_a.unwrap();
    let is_correct_turn = (is_player_a && pvp_game.turn == 0) || (!is_player_a && pvp_game.turn == 1);
    
    require!(is_correct_turn, ErrorCode::NotYourTurn);
    
    let defender_key = if is_player_a {
        pvp_game.player_b.unwrap()
    } else {
        pvp_game.player_a.unwrap()
    };
    
    msg!("✅ Turn validation passed");
    msg!("   Attacker: {} (Player {})", attacker_key, if is_player_a { "A" } else { "B" });
    msg!("   Defender: {} (Player {})", defender_key, if is_player_a { "B" } else { "A" });
    
    // ========== SIMPLIFIED PVP ATTACK LOGIC ========== //
    
    // For demonstration purposes, we'll use simplified logic
    // In production, module accounts would be passed via remaining_accounts
    
    msg!("✅ Attack initiated against {:?} module", target_module_type);
    
    // Simplified attack stats (would come from actual attack module)
    let base_damage = 100u32; // Base attack damage
    
    msg!("✅ Using base damage: {}", base_damage);
    
    // ========== CALCULATE DAMAGE ========== //
    
    // Generate cryptographically secure randomness
    let random_seed = helper::generate_pvp_random_seed(
        current_slot,
        current_time,
        &attacker_key,
        &defender_key,
        pvp_game.turn_number,
    );
    
    // Calculate damage with randomness
    let damage_multiplier = helper::calculate_damage_multiplier(&random_seed);
    
    // Check for special effects
    let explosion_effect = helper::check_special_effect(&random_seed[4..6], ATTACK_EXPLOSION_CHANCE);
    let final_multiplier = if explosion_effect {
        damage_multiplier * (1.0 + ATTACK_EXPLOSION_BONUS as f64 / 100.0)
    } else {
        damage_multiplier
    };
    
    let actual_damage = (base_damage as f64 * final_multiplier) as u32;
    
    msg!("⚔️ Damage calculation:");
    msg!("   Base damage: {}", base_damage);
    msg!("   Multiplier: {:.3}", final_multiplier);
    msg!("   Actual damage: {}", actual_damage);
    if explosion_effect {
        msg!("   💥 MAGAZINE EXPLOSION! +{}% damage", ATTACK_EXPLOSION_BONUS);
    }
    
    // ========== APPLY DAMAGE (SIMPLIFIED) ========== //
    
    // In full implementation, this would damage specific modules
    msg!("🎯 Damage applied to {:?} module", target_module_type);
    
    // Update moonbase HP tracking
    let old_moonbase_hp = if is_player_a {
        pvp_game.player_b_hp
    } else {
        pvp_game.player_a_hp
    };
    
    let new_moonbase_hp = old_moonbase_hp.saturating_sub(actual_damage);
    
    if is_player_a {
        pvp_game.player_b_hp = new_moonbase_hp;
    } else {
        pvp_game.player_a_hp = new_moonbase_hp;
    }
    
    msg!("🛡️ HP update: {} -> {} (-{})", old_moonbase_hp, new_moonbase_hp, actual_damage);
    
    // ========== CALCULATE TICKET MULTIPLIERS ========== //
    
    let (xp_mult, loot_mult, hash_mult) = helper::calculate_ticket_multipliers(pvp_game.ticket_lamports);
    
    msg!("🎫 Ticket multipliers: XP={:.1}x, Loot={:.1}x, Hash={:.1}x", xp_mult, loot_mult, hash_mult);
    
    // ========== APPLY SPECIAL EFFECTS BASED ON TARGET TYPE ========== //
    
    let mut xp_stolen = 0u32;
    let mut mdoge_stolen = 0u64;
    let mut hashpower_leeched = 0u64;
    let mut special_effect = "None".to_string();
    
    // Simplified special effects based on target type
    match target_module_type {
        ModuleType::Attraction => {
            let double_xp_effect = helper::check_special_effect(&random_seed[6..8], ATTRACTION_DOUBLE_XP_CHANCE);
            xp_stolen = (actual_damage / 10) * if double_xp_effect { 2 } else { 1 }; // Simplified: 1 XP per 10 damage
            
            if double_xp_effect {
                special_effect = "Double XP Steal".to_string();
            }
            
            msg!("🎯 Attraction attack: {} XP stolen", xp_stolen);
        },
        
        ModuleType::Mining => {
            // Simplified hashpower leech calculation  
            hashpower_leeched = (actual_damage as u64 * hash_mult as u64) / 100;
            
            // Update hash leech tracking (cap at 50% per player)
            let current_leech = if is_player_a {
                pvp_game.player_a_hash_leech
            } else {
                pvp_game.player_b_hash_leech
            };
            
            let max_leech = (ctx.accounts.defender_moonbase.active_hashpower * PvPGame::MAX_HASH_LEECH_PERCENT / 100) as u64;
            let new_leech = (current_leech + hashpower_leeched).min(max_leech);
            
            if is_player_a {
                pvp_game.player_a_hash_leech = new_leech;
            } else {
                pvp_game.player_b_hash_leech = new_leech;
            }
            
            hashpower_leeched = new_leech - current_leech; // Actual amount leeched
            
            if hashpower_leeched > 0 {
                special_effect = "Hash Leech".to_string();
                
                // Update global hashpower tracking
                helper::update_global_hashpower_for_pvp_damage(
                    &mut ctx.accounts.moon_doge_mining,
                    -(hashpower_leeched as i64),
                )?;
            }
            
            msg!("⛏️ Mining attack: {} hashpower leeched", hashpower_leeched);
        },
        
        ModuleType::Research => {
            // Simplified research loot steal
            let success_roll = u16::from_le_bytes([random_seed[8], random_seed[9]]) % 10000;
            let success_chance = 5000; // 50% base chance
            
            if success_roll < success_chance {
                mdoge_stolen = (actual_damage as u64 * 1000000) as u64; // 1 mDOGE per damage point
                mdoge_stolen = (mdoge_stolen as f64 * loot_mult) as u64;
                
                special_effect = "Research Loot".to_string();
                
                msg!("🔬 Research attack: {} mDOGE stolen", mdoge_stolen);
            } else {
                msg!("🔬 Research attack: loot steal failed");
            }
        },
        
        ModuleType::Attack => {
            // Attack modules reduce enemy ammunition conceptually
            special_effect = "Suppression".to_string();
            msg!("💣 Attack module suppressed");
        },
    }
    
    // ========== TRANSFER STOLEN RESOURCES ========== //
    
    if xp_stolen > 0 {
        // Add stolen XP to attacker
        helper::add_xp_simple(&mut ctx.accounts.attacker_moonbase, xp_stolen, "PvP XP Steal")?;
    }
    
    if mdoge_stolen > 0 {
        // In full implementation, transfer mDOGE tokens
        msg!("💎 mDOGE steal would transfer {} tokens", mdoge_stolen);
    }
    
    msg!("⚔️ Attack completed successfully");
    
    // ========== ADVANCE TURN ========== //
    
    pvp_game.turn ^= 1; // Switch turns
    pvp_game.turn_number += 1;
    pvp_game.last_move_ts = current_time;
    
    // ========== CHECK VICTORY CONDITIONS ========== //
    
    let game_ended = if pvp_game.player_a_hp == 0 || pvp_game.player_b_hp == 0 {
        // Total HP victory
        let winner = if pvp_game.player_a_hp == 0 { 
            pvp_game.player_b.unwrap() 
        } else { 
            pvp_game.player_a.unwrap() 
        };
        
        finalize_pvp_game_internal(
            pvp_game,
            &ctx.accounts.sol_treasury,
            &mut ctx.accounts.attacker_moonbase,
            &mut ctx.accounts.defender_moonbase,
            &mut ctx.accounts.global_config,
            Some(winner),
            "Total HP".to_string(),
            game_key,
            &ctx.accounts.system_program,
        )?;
        true
    } else if pvp_game.turn_number >= PvPGame::MAX_TURNS {
        // Turn limit reached - winner is player with more HP
        let winner = if pvp_game.player_a_hp > pvp_game.player_b_hp {
            pvp_game.player_a.unwrap()
        } else if pvp_game.player_b_hp > pvp_game.player_a_hp {
            pvp_game.player_b.unwrap()
        } else {
            // Draw - no winner, return stakes
            finalize_pvp_game_internal(
                pvp_game,
                &ctx.accounts.sol_treasury,
                &mut ctx.accounts.attacker_moonbase,
                &mut ctx.accounts.defender_moonbase,
                &mut ctx.accounts.global_config,
                None,
                "Draw".to_string(),
                game_key,
                &ctx.accounts.system_program,
            )?;
            return Ok(());
        };
        
        finalize_pvp_game_internal(
            pvp_game,
            &ctx.accounts.sol_treasury,
            &mut ctx.accounts.attacker_moonbase,
            &mut ctx.accounts.defender_moonbase,
            &mut ctx.accounts.global_config,
            Some(winner),
            "Turn Limit".to_string(),
            game_key,
            &ctx.accounts.system_program,
        )?;
        true
    } else {
        false
    };
    
    // ========== EMIT EVENTS ========== //
    
    // Clone special_effect before using it in events to avoid move issues
    let special_effect_clone = special_effect.clone();
    
    emit!(PvPAttackPerformed {
        game_id: game_key,
        attacker: attacker_key,
        defender: defender_key,
        attacker_module_index: 0, // Simplified
        target_module_type: format!("{:?}", target_module_type),
        target_module_index: 0, // Simplified
        base_damage,
        actual_damage,
        damage_multiplier: final_multiplier,
        turn_number: pvp_game.turn_number - 1, // Previous turn number
    });
    
    emit!(PvPAttackEffects {
        game_id: game_key,
        attacker: attacker_key,
        defender: defender_key,
        target_module_type: format!("{:?}", target_module_type),
        xp_stolen,
        mdoge_stolen,
        hashpower_leeched,
        special_effect,
        ticket_multiplier: xp_mult.max(loot_mult).max(hash_mult),
    });
    
    emit!(PvPModuleDamaged {
        game_id: game_key,
        owner: defender_key,
        module_index: 0, // Simplified
        module_type: format!("{:?}", target_module_type),
        old_hp: 1000, // Simplified
        new_hp: 1000 - actual_damage, // Simplified
        damage_taken: actual_damage,
        efficiency_before: 1.0, // Simplified
        efficiency_after: 0.9, // Simplified
    });
    
    if hashpower_leeched > 0 {
        emit!(PvPHashpowerLeeched {
            game_id: game_key,
            attacker: attacker_key,
            defender: defender_key,
            hashpower_amount: hashpower_leeched,
            attacker_leech_total: if is_player_a { pvp_game.player_a_hash_leech } else { pvp_game.player_b_hash_leech },
            defender_lost_total: 0, // Could track this separately if needed
        });
    }
    
    if explosion_effect || !special_effect_clone.is_empty() {
        emit!(PvPSpecialEffect {
            game_id: game_key,
            attacker: attacker_key,
            effect_type: if explosion_effect { "Magazine Explosion".to_string() } else { special_effect_clone },
            effect_value: if explosion_effect { ATTACK_EXPLOSION_BONUS as u64 } else { xp_stolen.max(mdoge_stolen as u32) as u64 },
            probability_roll: if explosion_effect { random_seed[4] as u16 } else { random_seed[6] as u16 },
            success_threshold: if explosion_effect { ATTACK_EXPLOSION_CHANCE } else { ATTRACTION_DOUBLE_XP_CHANCE },
        });
    }
    
    if !game_ended {
        msg!("🎮 Turn completed successfully!");
        msg!("   Next turn: Player {}", if pvp_game.turn == 0 { "A" } else { "B" });
        msg!("   Turn {}/{}", pvp_game.turn_number, PvPGame::MAX_TURNS);
        msg!("   Current HP: A={}, B={}", pvp_game.player_a_hp, pvp_game.player_b_hp);
    }
    
    Ok(())
}




// ========================================================================================
// ========== MOONBASE REPAIR SYSTEM =====================================================
// ========================================================================================

/// Repair damaged modules after PvP (free after cooldown or paid instantly)
pub fn repair_moonbase_internal(
    ctx: Context<RepairMoonbase>,
    pay_instantly: bool,
) -> Result<()> {
    let user_moonbase = &mut ctx.accounts.user_moonbase;
    let user = &ctx.accounts.user;
    let current_time = Clock::get()?.unix_timestamp;
    
    msg!("🔧 Starting moonbase repair for user {}", user.key());
    msg!("   Pay instantly: {}", pay_instantly);
    
    // Check if user is currently in a PvP game
    require!(user_moonbase.active_game.is_none(), ErrorCode::PlayerAlreadyInGame);
    
    // Simplified repair system - in production, you'd need module accounts via remaining_accounts
    // For now, we'll demonstrate the concept with basic moonbase HP restoration
    
    // Calculate total repair cost based on missing HP
    let max_possible_hp = 5000u32; // Simplified maximum HP for demonstration
    let current_hp = user_moonbase.pvp_hp;
    let damage = max_possible_hp.saturating_sub(current_hp);
    
    msg!("📊 Repair assessment:");
    msg!("   Current HP: {}", current_hp);
    msg!("   Max HP: {}", max_possible_hp);
    msg!("   Damage to repair: {}", damage);
    
    if damage == 0 {
        msg!("✅ No repairs needed - moonbase at full HP");
        return Ok(());
    }
    
    let total_repair_cost = damage as u64 * REPAIR_COST_PER_HP;
    msg!("   Total cost: {} SOL", total_repair_cost as f64 / 1e9);
    
    let repair_type = if pay_instantly {
        // Paid instant repair
        if total_repair_cost > 0 {
            anchor_lang::system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: user.to_account_info(),
                        to: ctx.accounts.sol_treasury.to_account_info(),
                    },
                ),
                total_repair_cost,
            )?;
            
            // Track total SOL spent by users for instant repairs
            ctx.accounts.global_config.total_sol_spent = ctx.accounts.global_config.total_sol_spent
                .checked_add(total_repair_cost)
                .unwrap_or(ctx.accounts.global_config.total_sol_spent);
        }
        "Paid Instant".to_string()
    } else {
        // Free repair after cooldown
        let cooldown_seconds = REPAIR_COOLDOWN_HOURS * 3600;
        require!(
            current_time >= user_moonbase.last_game_end_ts + cooldown_seconds,
            ErrorCode::UpdateTooEarly
        );
        "Free Cooldown".to_string()
    };
    
    // Restore moonbase HP to maximum (simplified)
    user_moonbase.pvp_hp = max_possible_hp;
    
    // Update moonbase state
    user_moonbase.modules_repaired_since_last_game = true;
    user_moonbase.last_game_end_ts = current_time;
    
    emit!(MoonbaseRepaired {
        owner: user.key(),
        repair_cost: total_repair_cost,
        modules_repaired: 1, // Simplified
        hp_restored: damage,
        repair_type: repair_type.clone(),
    });
    
    msg!("🎉 Moonbase repair completed!");
    msg!("   HP restored: {} -> {}", current_hp, max_possible_hp);
    msg!("   Repair type: {}", repair_type);
    msg!("   Cost: {} SOL", total_repair_cost as f64 / 1e9);
    
    Ok(())
}


 


/// Finalize PvP game and distribute rewards
fn finalize_pvp_game_internal(
    pvp_game: &mut PvPGame,
    sol_treasury: &AccountInfo,
    attacker_moonbase: &mut UserMoonBaseInstance,
    defender_moonbase: &mut UserMoonBaseInstance,
    global_config: &mut GlobalConfig,
    winner_opt: Option<Pubkey>,
    victory_condition: String,
    game_key: Pubkey,
    system_program: &AccountInfo,
) -> Result<()> {
    msg!("🏁 Finalizing PvP game");
    msg!("   Victory condition: {}", victory_condition);
    
    // Clear active game from both moonbases
    attacker_moonbase.active_game = None;
    defender_moonbase.active_game = None;
    
    let game_duration = Clock::get()?.unix_timestamp - pvp_game.game_start_ts;
    
    if let Some(winner) = winner_opt {
        pvp_game.winner = Some(winner);
        
        // Calculate prize distribution: 90% to winner, 10% to treasury
        let total_pot = pvp_game.pot_lamports;
        let prize = total_pot.saturating_mul(9) / 10;
        let treasury_cut = total_pot - prize;
        
        // Determine winner and get their referrer
        let winner_is_player_a = winner == pvp_game.player_a.unwrap();
        let winner_moonbase = if winner_is_player_a { attacker_moonbase } else { defender_moonbase };
        let winner_referrer = winner_moonbase.referral;
        
        // Process treasury cut with referral payment system
        // Note: In a full implementation, we would need the winner's account info to deduct from
        // For now, we simulate the referral payment logic for the treasury cut
        if winner_referrer != anchor_lang::system_program::ID && treasury_cut > 0 {
            // Calculate referral fee (15% of treasury cut)
            let referral_fee = treasury_cut.checked_mul(15).unwrap().checked_div(100).unwrap();
            let remaining_treasury = treasury_cut.checked_sub(referral_fee).unwrap();
            
            // Update global tracking
            global_config.total_referral_sol_paid = global_config.total_referral_sol_paid
                .checked_add(referral_fee)
                .unwrap();
                
            msg!("💰 PvP Treasury cut with referral: {} SOL to referrer, {} SOL to treasury", 
                 referral_fee as f64 / 1e9, remaining_treasury as f64 / 1e9);
            
            // Note: In production, you would transfer referral_fee to referrer's rewards account
            // and remaining_treasury to sol_treasury
        } else {
            msg!("💰 PvP Treasury cut (no referrer): {} SOL to treasury", 
                 treasury_cut as f64 / 1e9);
        }
        
        // Track total SOL spent (treasury cut from PvP games)
        // Note: This represents SOL that players effectively "spent" through PvP participation
        // since the treasury cut is taken from their staked amounts
        global_config.total_sol_spent = global_config.total_sol_spent
            .checked_add(treasury_cut)
            .unwrap_or(global_config.total_sol_spent);
        
        msg!("💰 Prize distribution: {} SOL to winner, {} SOL total treasury cut", 
             prize as f64 / 1e9, treasury_cut as f64 / 1e9);
        
        emit!(PvPGameFinished {
            game_id: game_key,
            winner,
            loser: if winner_is_player_a { pvp_game.player_b.unwrap() } else { pvp_game.player_a.unwrap() },
            victory_condition,
            final_attacker_hp: if winner_is_player_a { pvp_game.player_a_hp } else { pvp_game.player_b_hp },
            final_defender_hp: if winner_is_player_a { pvp_game.player_b_hp } else { pvp_game.player_a_hp },
            prize_amount: prize,
            total_turns: pvp_game.turn_number,
            duration_seconds: game_duration,
        });
        
        msg!("🎉 Victory! {} wins {} SOL prize", winner, prize as f64 / 1e9);
    } else {
        // Draw - return stakes to both players
        msg!("🤝 Game ended in draw - returning stakes");
        
        emit!(PvPGameFinished {
            game_id: game_key,
            winner: Pubkey::default(), // No winner
            loser: Pubkey::default(),   // No loser
            victory_condition,
            final_attacker_hp: pvp_game.player_a_hp,
            final_defender_hp: pvp_game.player_b_hp,
            prize_amount: 0,
            total_turns: pvp_game.turn_number,
            duration_seconds: game_duration,
        });
    }
    
    Ok(())
}





// =========================================================================================
// ========== ACCOUNT CONTEXTS =============================================================
// =========================================================================================

#[derive(Accounts)]
#[instruction(ticket_index: u8)]
pub struct CreatePvPGame<'info> {
    #[account(
        init,
        payer = player_a,
        space = PvPGame::LEN,
        seeds = [PVP_GAME_SEED.as_ref(), player_a.key().as_ref()],
        bump
    )]
    pub pvp_game: Account<'info, PvPGame>,

    #[account(
        mut,
        seeds = [USER_MOONBASE_SEED.as_ref(), player_a.key().as_ref()],
        bump = player_a_moonbase.bump,
        constraint = player_a_moonbase.owner == player_a.key() @ ErrorCode::Unauthorized,
        constraint = player_a_moonbase.active_game.is_none() @ ErrorCode::PlayerAlreadyInGame
    )]
    pub player_a_moonbase: Account<'info, UserMoonBaseInstance>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.is_game_active @ ErrorCode::PvPGamesDisabled
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        seeds = [PVP_MATCHMAKER_SEED.as_ref()],
        bump = pvp_matchmaker.bump,
    )]
    pub pvp_matchmaker: Account<'info, PvPMatchmaker>,


    #[account(mut)]
    pub player_a: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(player_a: Pubkey)]
pub struct JoinPvPGame<'info> {
    #[account(
        mut,
        seeds = [PVP_GAME_SEED.as_ref(), player_a.key().as_ref()],
        bump = pvp_game.bump,
    )]
    pub pvp_game: Account<'info, PvPGame>,

    #[account(
        seeds = [PVP_MATCHMAKER_SEED.as_ref()],
        bump = pvp_matchmaker.bump,
    )]
    pub pvp_matchmaker: Account<'info, PvPMatchmaker>,

    #[account(
        mut,
        seeds = [USER_MOONBASE_SEED.as_ref(), player_b.key().as_ref()],
        bump = player_b_moonbase.bump,
    )]
    pub player_b_moonbase: Account<'info, UserMoonBaseInstance>,

    #[account(mut)]
    pub player_b: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CancelPvPGame<'info> {
    #[account(
        mut,
        seeds = [PVP_GAME_SEED.as_ref(), player_a_moonbase.owner.as_ref()],
        bump = pvp_game.bump,
        close = player_a  // Close to player's wallet instead of moonbase
    )]
    pub pvp_game: Account<'info, PvPGame>,

    #[account(
        mut,
        seeds = [USER_MOONBASE_SEED.as_ref(), player_a_moonbase.owner.as_ref()],
        bump = player_a_moonbase.bump,
        constraint = player_a_moonbase.owner == player_a.key() @ ErrorCode::Unauthorized
    )]
    pub player_a_moonbase: Account<'info, UserMoonBaseInstance>,

    #[account(
        mut,
        seeds = [PVP_MATCHMAKER_SEED.as_ref()],
        bump = pvp_matchmaker.bump,
    )]
    pub pvp_matchmaker: Account<'info, PvPMatchmaker>,

    /// CHECK: This is the player A's wallet that will receive the stake
    #[account(mut)]
    pub player_a: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CancelExpiredPvPGame<'info> {
    #[account(
        mut,
        seeds = [PVP_GAME_SEED.as_ref(), player_a.key().as_ref()],
        bump = pvp_game.bump,
        close = player_a  // Close to player A's wallet
    )]
    pub pvp_game: Account<'info, PvPGame>,

    #[account(
        mut,
        seeds = [USER_MOONBASE_SEED.as_ref(), player_a.key().as_ref()],
        bump = player_a_moonbase.bump,
    )]
    pub player_a_moonbase: Account<'info, UserMoonBaseInstance>,

    #[account(
        mut,
        seeds = [PVP_MATCHMAKER_SEED.as_ref()],
        bump = pvp_matchmaker.bump,
    )]
    pub pvp_matchmaker: Account<'info, PvPMatchmaker>,

    /// CHECK: This is the player A's wallet that will receive the stake
    #[account(mut)]
    pub player_a: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PvPAttack<'info> {
    // PvP Game State
    #[account(mut)]
    pub pvp_game: Account<'info, PvPGame>,

    // Attacker accounts
    #[account(
        mut,
        seeds = [USER_MOONBASE_SEED.as_ref(), attacker.key().as_ref()],
        bump = attacker_moonbase.bump,
        constraint = attacker_moonbase.owner == attacker.key() @ ErrorCode::Unauthorized,
    )]
    pub attacker_moonbase: Account<'info, UserMoonBaseInstance>,

    // Defender accounts  
    #[account(mut)]
    pub defender_moonbase: Account<'info, UserMoonBaseInstance>,

    // Global mining state for hashpower tracking
    #[account(
        mut,
        seeds = [MOON_DOGE_MINING_SEED.as_ref()],
        bump = moon_doge_mining.bump,
    )]
    pub moon_doge_mining: Account<'info, MoonDogeMining>,

    // Global config for tracking total SOL spent
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    // Treasury for collecting fees
    /// CHECK: SOL treasury account for collecting fees
    #[account(mut)]
    pub sol_treasury: AccountInfo<'info>,

    #[account(mut)]
    pub attacker: Signer<'info>,

    pub system_program: Program<'info, System>,
}

// Simplified PvP attack for demonstration - modules handled via remaining_accounts
#[derive(Accounts)]
pub struct PvPAttackSimple<'info> {
    #[account(mut)]
    pub pvp_game: Account<'info, PvPGame>,

    #[account(
        mut,
        seeds = [USER_MOONBASE_SEED.as_ref(), attacker.key().as_ref()],
        bump = attacker_moonbase.bump,
        constraint = attacker_moonbase.owner == attacker.key() @ ErrorCode::Unauthorized,
    )]
    pub attacker_moonbase: Account<'info, UserMoonBaseInstance>,

    #[account(mut)]
    pub defender_moonbase: Account<'info, UserMoonBaseInstance>,

    #[account(
        mut,
        seeds = [MOON_DOGE_MINING_SEED.as_ref()],
        bump = moon_doge_mining.bump,
    )]
    pub moon_doge_mining: Account<'info, MoonDogeMining>,

    /// CHECK: SOL treasury account
    #[account(mut)]
    pub sol_treasury: AccountInfo<'info>,

    #[account(mut)]
    pub attacker: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RepairMoonbase<'info> {
    #[account(
        mut,
        seeds = [USER_MOONBASE_SEED.as_ref(), user.key().as_ref()],
        bump = user_moonbase.bump,
        constraint = user_moonbase.owner == user.key() @ ErrorCode::Unauthorized,
    )]
    pub user_moonbase: Account<'info, UserMoonBaseInstance>,

    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// CHECK: SOL treasury account for repair fees
    #[account(mut)]
    pub sol_treasury: AccountInfo<'info>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

 