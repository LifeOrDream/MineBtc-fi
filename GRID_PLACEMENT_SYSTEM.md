# Grid Placement System Implementation

## Overview

This document outlines the implementation of a sophisticated tile-based grid placement system for the DogeTech moonbase game. The system provides efficient overlap checking, constant-size storage, and future-proofing for map expansions.

## Core Architecture

### 1. Grid System Constants

```rust
// Grid dimensions
pub const GRID_WIDTH: u8 = 20; // 20 tiles wide
pub const GRID_HEIGHT: u8 = 15; // 15 tiles tall
pub const TOTAL_TILES: usize = (GRID_WIDTH as usize) * (GRID_HEIGHT as usize); // 300 tiles
pub const BITMAP_SIZE: usize = (TOTAL_TILES + 7) / 8; // 38 bytes (300 bits rounded up to bytes)
```

### 2. Enhanced Data Structures

#### UserMoonBaseInstance
```rust
#[account]
pub struct UserMoonBaseInstance {
    // ... existing fields ...
    /// Grid occupation bitmap (300 tiles = 38 bytes)
    pub occupied_bitmap: [u8; BITMAP_SIZE],
}
```

#### ModuleConfig
```rust
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ModuleConfig {
    // ... existing fields ...
    /// Module width in tiles
    pub width: u8,
    /// Module height in tiles
    pub height: u8,
    // ... rest of fields ...
}
```

#### ModuleInstance
```rust
#[account]
pub struct ModuleInstance {
    // ... existing fields ...
    /// Position on the grid
    pub pos_x: u8,      // left-most tile (0..GRID_WIDTH-1)
    pub pos_y: u8,      // top-most tile (0..GRID_HEIGHT-1)
    pub width: u8,      // tiles wide
    pub height: u8,     // tiles tall
    // ... rest of fields ...
}
```

## Core Placement Functions

### 1. Placement Validation
```rust
/// Check if a module can be placed at the given coordinates
pub fn can_place_module(
    user_moonbase: &UserMoonBaseInstance,
    x: u8,
    y: u8,
    width: u8,
    height: u8,
) -> Result<bool> {
    // 1. Bounds check
    if x.checked_add(width).ok_or(ErrorCode::ArithmeticOverflow)? > GRID_WIDTH {
        return Ok(false);
    }
    if y.checked_add(height).ok_or(ErrorCode::ArithmeticOverflow)? > GRID_HEIGHT {
        return Ok(false);
    }
    
    // 2. Overlap check
    for dy in 0..height {
        for dx in 0..width {
            let tile_x = x.checked_add(dx).ok_or(ErrorCode::ArithmeticOverflow)?;
            let tile_y = y.checked_add(dy).ok_or(ErrorCode::ArithmeticOverflow)?;
            
            if is_tile_occupied(user_moonbase, tile_x, tile_y)? {
                return Ok(false);
            }
        }
    }
    
    Ok(true)
}
```

### 2. Tile Occupation Management
```rust
/// Mark tiles as occupied for a module
pub fn mark_tiles_occupied(
    user_moonbase: &mut UserMoonBaseInstance,
    x: u8,
    y: u8,
    width: u8,
    height: u8,
) -> Result<()> {
    // Mark all tiles as occupied using bit manipulation
    for dy in 0..height {
        for dx in 0..width {
            let tile_x = x.checked_add(dx).ok_or(ErrorCode::ArithmeticOverflow)?;
            let tile_y = y.checked_add(dy).ok_or(ErrorCode::ArithmeticOverflow)?;
            
            let idx = (tile_y as usize) * (GRID_WIDTH as usize) + (tile_x as usize);
            let byte_idx = idx / 8;
            let bit_idx = idx % 8;
            
            user_moonbase.occupied_bitmap[byte_idx] |= 1 << bit_idx;
        }
    }
    
    Ok(())
}
```

### 3. Module Placement
```rust
/// Place a module at the given coordinates
pub fn place_module(
    user_moonbase: &mut UserMoonBaseInstance,
    module_instance: &mut ModuleInstance,
    x: u8,
    y: u8,
    width: u8,
    height: u8,
) -> Result<()> {
    // 1. Check if placement is valid
    require!(
        can_place_module(user_moonbase, x, y, width, height)?,
        ErrorCode::TileAlreadyOccupied
    );
    
    // 2. Mark tiles as occupied
    mark_tiles_occupied(user_moonbase, x, y, width, height)?;
    
    // 3. Save coordinates on the module instance
    module_instance.pos_x = x;
    module_instance.pos_y = y;
    module_instance.width = width;
    module_instance.height = height;
    
    Ok(())
}
```

## Integration with Module Creation

### Updated create_module_instance Function
```rust
pub fn create_module_instance(
    ctx: Context<CreateModuleInstance>,
    config_id: u16,
    pos_x: u8,
    pos_y: u8,
) -> Result<()> {
    let user_moonbase = &mut ctx.accounts.user_moonbase;
    let module_config = /* find module config */;
    let module_instance = &mut ctx.accounts.module_instance;
    
    // ... validation logic ...
    
    // Place the module on the grid using the new placement system
    helper::place_module(
        user_moonbase,
        module_instance,
        pos_x,
        pos_y,
        module_config.width,
        module_config.height,
    )?;
    
    // ... rest of module initialization ...
    
    Ok(())
}
```

## Advanced Features

### 1. Module Movement
```rust
/// Move a module to new coordinates
pub fn move_module(
    user_moonbase: &mut UserMoonBaseInstance,
    module_instance: &mut ModuleInstance,
    new_x: u8,
    new_y: u8,
) -> Result<()> {
    // 1. Clear current tiles
    clear_tiles(
        user_moonbase,
        module_instance.pos_x,
        module_instance.pos_y,
        module_instance.width,
        module_instance.height,
    )?;
    
    // 2. Check if new placement is valid
    require!(
        can_place_module(user_moonbase, new_x, new_y, module_instance.width, module_instance.height)?,
        ErrorCode::TileAlreadyOccupied
    );
    
    // 3. Mark new tiles as occupied
    mark_tiles_occupied(
        user_moonbase,
        new_x,
        new_y,
        module_instance.width,
        module_instance.height,
    )?;
    
    // 4. Update coordinates
    module_instance.pos_x = new_x;
    module_instance.pos_y = new_y;
    module_instance.last_updated = Clock::get()?.unix_timestamp;
    
    Ok(())
}
```

### 2. Module Removal
```rust
/// Remove a module and clear its tiles
pub fn remove_module(
    user_moonbase: &mut UserMoonBaseInstance,
    module_instance: &ModuleInstance,
) -> Result<()> {
    // Clear the tiles occupied by this module
    clear_tiles(
        user_moonbase,
        module_instance.pos_x,
        module_instance.pos_y,
        module_instance.width,
        module_instance.height,
    )?;
    
    Ok(())
}
```

## Frontend Integration

### 1. Module Creation Call
```javascript
// Frontend calls with grid coordinates
await program.methods
  .createModuleInstance(configId, posX, posY)
  .accounts({
    // ... accounts ...
  })
  .rpc();
```

### 2. Grid Visualization
```javascript
// Convert bitmap to visual grid
function visualizeGrid(occupiedBitmap) {
  const grid = [];
  for (let y = 0; y < GRID_HEIGHT; y++) {
    const row = [];
    for (let x = 0; x < GRID_WIDTH; x++) {
      const idx = y * GRID_WIDTH + x;
      const byteIdx = Math.floor(idx / 8);
      const bitIdx = idx % 8;
      const isOccupied = (occupiedBitmap[byteIdx] & (1 << bitIdx)) !== 0;
      row.push(isOccupied);
    }
    grid.push(row);
  }
  return grid;
}
```

## Performance Benefits

### 1. Constant Storage
- **Fixed Size**: 38 bytes regardless of number of modules
- **Predictable Rent**: No growing Vec means stable account costs
- **Memory Efficient**: Bit-level storage vs object arrays

### 2. Fast Operations
- **O(width × height)**: Placement check complexity
- **Bit Manipulation**: Extremely fast set/clear operations
- **Cache Friendly**: Compact data structure

### 3. Scalability
- **Grid Expansion**: Easy to increase grid size
- **Module Shapes**: Supports any rectangular module
- **Future Features**: Ready for advanced placement rules

## Error Handling

### New Error Codes
```rust
#[error_code]
pub enum ErrorCode {
    // ... existing errors ...
    
    #[msg("Invalid grid coordinates")]
    InvalidGridCoordinates,
    
    #[msg("Module placement out of bounds")]
    PlacementOutOfBounds,
    
    #[msg("Tile is already occupied")]
    TileAlreadyOccupied,
}
```

## Example Module Configurations

### Mining Rig (2×2)
```rust
ModuleConfig {
    id: 1,
    name: "Mining Rig".to_string(),
    width: 2,
    height: 2,
    module_type: ModuleType::Mining,
    stats: ModuleStats::Mining(MiningStats {
        base_hashpower: 100,
        hashpower_per_upgrade: 20,
        electricity_cost: 50,
    }),
    // ... other fields ...
}
```

### Defense Turret (1×1)
```rust
ModuleConfig {
    id: 2,
    name: "Defense Turret".to_string(),
    width: 1,
    height: 1,
    module_type: ModuleType::Defense,
    stats: ModuleStats::Defense(DefenseStats {
        shield_hp: 500,
        recharge_secs: 30,
        electricity_cost: 25,
    }),
    // ... other fields ...
}
```

### Research Lab (3×2)
```rust
ModuleConfig {
    id: 3,
    name: "Research Lab".to_string(),
    width: 3,
    height: 2,
    module_type: ModuleType::Research,
    stats: ModuleStats::Research(ResearchStats {
        research_secs: 3600, // 1 hour
        loot_table_id: 1,
        electricity_cost: 75,
    }),
    // ... other fields ...
}
```

## Future Enhancements

### 1. Advanced Placement Rules
- **Adjacency Bonuses**: Modules gain bonuses when placed next to specific types
- **Terrain Effects**: Different grid areas provide different bonuses
- **Exclusion Zones**: Some modules cannot be placed near others

### 2. Dynamic Grid Features
- **Expandable Maps**: Unlock new grid areas as players progress
- **Multi-Level Bases**: 3D placement with multiple floors
- **Destructible Terrain**: Environmental hazards that affect placement

### 3. Optimization Features
- **Placement Suggestions**: AI-powered optimal placement recommendations
- **Template Saves**: Save and load base layouts
- **Bulk Operations**: Move multiple modules at once

## Testing Scenarios

### 1. Basic Placement
```rust
#[test]
fn test_basic_module_placement() {
    let mut moonbase = create_test_moonbase();
    let mut module = create_test_module();
    
    // Should succeed - empty grid
    assert!(place_module(&mut moonbase, &mut module, 0, 0, 2, 2).is_ok());
    
    // Should fail - overlapping placement
    let mut module2 = create_test_module();
    assert!(place_module(&mut moonbase, &mut module2, 1, 1, 2, 2).is_err());
}
```

### 2. Boundary Testing
```rust
#[test]
fn test_boundary_placement() {
    let mut moonbase = create_test_moonbase();
    let mut module = create_test_module();
    
    // Should succeed - exactly at boundary
    assert!(place_module(&mut moonbase, &mut module, 18, 13, 2, 2).is_ok());
    
    // Should fail - out of bounds
    let mut module2 = create_test_module();
    assert!(place_module(&mut moonbase, &mut module2, 19, 14, 2, 2).is_err());
}
```

### 3. Movement Testing
```rust
#[test]
fn test_module_movement() {
    let mut moonbase = create_test_moonbase();
    let mut module = create_test_module();
    
    // Place module
    place_module(&mut moonbase, &mut module, 5, 5, 2, 2).unwrap();
    
    // Move to new location
    assert!(move_module(&mut moonbase, &mut module, 10, 10).is_ok());
    
    // Verify old location is clear
    assert!(!is_tile_occupied(&moonbase, 5, 5).unwrap());
    assert!(!is_tile_occupied(&moonbase, 6, 6).unwrap());
    
    // Verify new location is occupied
    assert!(is_tile_occupied(&moonbase, 10, 10).unwrap());
    assert!(is_tile_occupied(&moonbase, 11, 11).unwrap());
}
```

This grid placement system provides a robust foundation for tile-based gameplay while maintaining excellent performance and scalability characteristics. 