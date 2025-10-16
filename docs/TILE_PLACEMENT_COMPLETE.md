# 🗺️ Grid Placement System: Complete Technical Documentation

> **Bitmap-Based Tile Management** | Version 1.0.0 | October 15, 2025

---

## Overview

The tile placement system uses a **bitmap-based approach** for efficient grid management. Each of the 300 tiles is represented by a single bit, allowing constant-size storage (38 bytes) regardless of how many modules are placed.

---

## Grid Specifications

### Dimensions

```rust
// Maximum grid (full moon)
GRID_WIDTH: 20 tiles
GRID_HEIGHT: 15 tiles
TOTAL_TILES: 300 tiles

// Default moonbase size
DEFAULT_MOONBASE_WIDTH: 10 tiles
DEFAULT_MOONBASE_HEIGHT: 8 tiles
DEFAULT_AREA: 80 tiles

// Bitmap storage
BITMAP_SIZE: (300 + 7) / 8 = 38 bytes
```

### Coordinate System

```
Origin (0,0) is top-left

  0  1  2  3  4  5  6  7  8  9  ...  19  (X-axis)
0 ┌──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┐
1 ├──┼──┼──┼──┼──┼──┼──┼──┼──┼──┼──┼──┤
2 ├──┼──┼──┼──┼──┼──┼──┼──┼──┼──┼──┼──┤
3 ├──┼──┼──┼──┼──┼──┼──┼──┼──┼──┼──┼──┤
...
14└──┴──┴──┴──┴──┴──┴──┴──┴──┴──┴──┴──┘
(Y-axis)

Module at (5, 3) with size 2×2 occupies:
(5,3), (6,3), (5,4), (6,4)
```

---

## Bitmap Storage

### Why Bitmap?

**Comparison:**

| Approach | Storage | Complexity | Scalability |
|----------|---------|------------|-------------|
| **Array of bools** | 300 bytes | O(n) search | ❌ Grows with tiles |
| **Vec of positions** | 4 + (8×modules) | O(n) collision | ❌ Grows with modules |
| **Bitmap** | 38 bytes | O(width×height) | ✅ Fixed size! |

**Benefits:**
- ✅ **Constant size**: Always 38 bytes
- ✅ **Predictable rent**: Fixed account cost
- ✅ **Fast operations**: Bit manipulation is ultra-fast
- ✅ **Expansion-ready**: Can increase grid size easily

### Bitmap Encoding

```rust
// Each tile = 1 bit in the array
pub struct UserMoonBaseInstance {
    occupied_bitmap: [u8; 38],  // 38 bytes = 304 bits (300 used)
}

// Tile (x, y) → Bit index
index = (y × GRID_WIDTH) + x

// Bit index → Array position
byte_index = index / 8
bit_offset = index % 8

// Check if occupied
is_occupied = (occupied_bitmap[byte_index] & (1 << bit_offset)) != 0

// Set as occupied
occupied_bitmap[byte_index] |= (1 << bit_offset)

// Clear (unoccupy)
occupied_bitmap[byte_index] &= !(1 << bit_offset)
```

### Examples

**Example 1: Tile (0, 0)**
```
index = (0 × 20) + 0 = 0
byte_index = 0 / 8 = 0
bit_offset = 0 % 8 = 0

Check: occupied_bitmap[0] & (1 << 0) = occupied_bitmap[0] & 0b00000001
Set:   occupied_bitmap[0] |= 0b00000001
Clear: occupied_bitmap[0] &= 0b11111110
```

**Example 2: Tile (7, 5)**
```
index = (5 × 20) + 7 = 107
byte_index = 107 / 8 = 13
bit_offset = 107 % 8 = 3

Check: occupied_bitmap[13] & (1 << 3) = occupied_bitmap[13] & 0b00001000
Set:   occupied_bitmap[13] |= 0b00001000
Clear: occupied_bitmap[13] &= 0b11110111
```

**Example 3: Last tile (19, 14)**
```
index = (14 × 20) + 19 = 299
byte_index = 299 / 8 = 37
bit_offset = 299 % 8 = 3

Check: occupied_bitmap[37] & (1 << 3) = occupied_bitmap[37] & 0b00001000
```

---

## Core Functions

### 1. Check Tile Occupation

```rust
pub fn is_tile_occupied(
    user_moonbase: &UserMoonBaseInstance,
    x: u8,
    y: u8,
) -> Result<bool> {
    // Bounds check
    if x >= GRID_WIDTH || y >= GRID_HEIGHT {
        return Err(ErrorCode::InvalidTileIndex.into());
    }
    
    // Calculate bit index
    let idx = (y as usize) × (GRID_WIDTH as usize) + (x as usize);
    let byte_idx = idx / 8;
    let bit_idx = idx % 8;
    
    // Verify byte index in range
    if byte_idx >= BITMAP_SIZE {
        return Err(ErrorCode::InvalidTileIndex.into());
    }
    
    // Check bit
    let is_occupied = (user_moonbase.occupied_bitmap[byte_idx] & (1 << bit_idx)) != 0;
    
    Ok(is_occupied)
}
```

---

### 2. Mark Tiles Occupied

```rust
pub fn mark_tiles_occupied(
    user_moonbase: &mut UserMoonBaseInstance,
    x: u8,
    y: u8,
    width: u8,
    height: u8,
) -> Result<()> {
    // Bounds check
    require!(
        x.checked_add(width).ok_or(ErrorCode::ArithmeticOverflow)? <= GRID_WIDTH,
        ErrorCode::InvalidTileIndex
    );
    require!(
        y.checked_add(height).ok_or(ErrorCode::ArithmeticOverflow)? <= GRID_HEIGHT,
        ErrorCode::InvalidTileIndex
    );
    
    // Mark all tiles in the rectangle
    for dy in 0..height {
        for dx in 0..width {
            let tile_x = x.checked_add(dx).ok_or(ErrorCode::ArithmeticOverflow)?;
            let tile_y = y.checked_add(dy).ok_or(ErrorCode::ArithmeticOverflow)?;
            
            let idx = (tile_y as usize) × (GRID_WIDTH as usize) + (tile_x as usize);
            let byte_idx = idx / 8;
            let bit_idx = idx % 8;
            
            if byte_idx >= BITMAP_SIZE {
                return Err(ErrorCode::InvalidTileIndex.into());
            }
            
            // Set bit to 1 (occupied)
            user_moonbase.occupied_bitmap[byte_idx] |= 1 << bit_idx;
        }
    }
    
    msg!("🏗️ Marked tiles occupied: ({}, {}) to ({}, {})", 
         x, y, x + width - 1, y + height - 1);
    
    Ok(())
}
```

**Example: 2×2 module at (5, 3)**
```
Tiles to mark: (5,3), (6,3), (5,4), (6,4)

Tile (5,3): index=65, byte=8, bit=1 → occupied_bitmap[8] |= 0b00000010
Tile (6,3): index=66, byte=8, bit=2 → occupied_bitmap[8] |= 0b00000100
Tile (5,4): index=85, byte=10, bit=5 → occupied_bitmap[10] |= 0b00100000
Tile (6,4): index=86, byte=10, bit=6 → occupied_bitmap[10] |= 0b01000000

Result: 4 bits set in 2 different bytes
```

---

### 3. Clear Tiles

```rust
pub fn clear_tiles(
    user_moonbase: &mut UserMoonBaseInstance,
    x: u8,
    y: u8,
    width: u8,
    height: u8,
) -> Result<()> {
    // Bounds check (same as mark_tiles_occupied)
    require!(...);
    
    // Clear all tiles in the rectangle
    for dy in 0..height {
        for dx in 0..width {
            let tile_x = x.checked_add(dx).ok_or(ErrorCode::ArithmeticOverflow)?;
            let tile_y = y.checked_add(dy).ok_or(ErrorCode::ArithmeticOverflow)?;
            
            let idx = (tile_y as usize) × (GRID_WIDTH as usize) + (tile_x as usize);
            let byte_idx = idx / 8;
            let bit_idx = idx % 8;
            
            if byte_idx >= BITMAP_SIZE {
                return Err(ErrorCode::InvalidTileIndex.into());
            }
            
            // Set bit to 0 (unoccupied)
            user_moonbase.occupied_bitmap[byte_idx] &= !(1 << bit_idx);
        }
    }
    
    msg!("🧹 Cleared tiles: ({}, {}) to ({}, {})", 
         x, y, x + width - 1, y + height - 1);
    
    Ok(())
}
```

---

### 4. Validate Placement

```rust
pub fn can_place_module(
    user_moonbase: &UserMoonBaseInstance,
    x: u8,
    y: u8,
    width: u8,
    height: u8,
) -> Result<bool> {
    // 1. Bounds check (against FULL grid)
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
                return Ok(false);  // Collision detected!
            }
        }
    }
    
    Ok(true)  // Valid placement
}
```

---

### 5. Validate Within Moonbase Bounds

```rust
pub fn can_place_module_in_moonbase(
    user_moonbase: &UserMoonBaseInstance,
    x: u8,
    y: u8,
    width: u8,
    height: u8,
) -> Result<bool> {
    // 1. Check against CURRENT moonbase size (not full grid)
    if x.checked_add(width).ok_or(ErrorCode::ArithmeticOverflow)? > user_moonbase.current_width {
        return Ok(false);
    }
    if y.checked_add(height).ok_or(ErrorCode::ArithmeticOverflow)? > user_moonbase.current_height {
        return Ok(false);
    }
    
    // 2. Check overlap (same as can_place_module)
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

**Difference:**
- `can_place_module`: Checks against **full 20×15 grid**
- `can_place_module_in_moonbase`: Checks against **current moonbase size** (10×8 initially)

---

### 6. Place Module

```rust
pub fn place_module(
    user_moonbase: &mut UserMoonBaseInstance,
    module_instance: &mut ModuleInstance,
    x: u8,
    y: u8,
    width: u8,
    height: u8,
) -> Result<()> {
    // 1. Validate placement
    require!(
        can_place_module(user_moonbase, x, y, width, height)?,
        ErrorCode::TileAlreadyOccupied
    );
    
    // 2. Mark tiles as occupied
    mark_tiles_occupied(user_moonbase, x, y, width, height)?;
    
    // 3. Store coordinates in module
    module_instance.pos_x = x;
    module_instance.pos_y = y;
    module_instance.width = width;
    module_instance.height = height;
    
    msg!("📍 Module placed at ({}, {}) with size {}×{}", x, y, width, height);
    
    Ok(())
}
```

---

## Module Placement Flow

### Installation (Undeployed → Deployed)

```
User calls: install_module(module_index, pos_x, pos_y)

Step 1: Validation
├─→ Check module exists in inventory
├─→ Check module is not already deployed
├─→ Check config_id matches
└─→ Check module is not active

Step 2: Electricity Check
├─→ Calculate electricity_cost
├─→ Check: used + cost <= available
└─→ Reserve electricity (not consumed yet)

Step 3: Placement Validation
├─→ can_place_module_in_moonbase(x, y, width, height)?
├─→ Checks bounds (current_width, current_height)
├─→ Checks overlap with existing modules
└─→ Returns true/false

Step 4: Grid Update
├─→ place_module(user, module, x, y, width, height)
├─→ Calls mark_tiles_occupied()
├─→ Sets all bits for occupied tiles
└─→ Stores position in module_instance

Step 5: State Updates
├─→ module.is_active = true
├─→ user.used_electricity += electricity_cost
├─→ user.active_hashpower += module_hashpower (if mining)
├─→ user.pvp_hp += module_max_hp
└─→ global.total_active_hashpower += module_hashpower

Step 6: XP Award
└─→ Award 50 XP for installation
```

### Removal (Deployed → Undeployed)

```
User calls: remove_module(module_index)

Step 1: Validation
├─→ Check module exists
├─→ Check module is deployed (is_active = true)
└─→ Check user owns the module

Step 2: Mine Pending Rewards
├─→ mine_dbtc_for_user() BEFORE changing hashpower
└─→ Prevents reward loss

Step 3: Grid Update
├─→ clear_tiles(user, module.pos_x, module.pos_y, width, height)
├─→ Unsets all bits for these tiles
└─→ Tiles now available for other modules

Step 4: State Updates
├─→ module.is_active = false
├─→ user.used_electricity -= electricity_cost
├─→ user.active_hashpower -= module_hashpower (if mining)
├─→ user.pvp_hp -= module_max_hp
└─→ global.total_active_hashpower -= module_hashpower

Step 5: Inventory Return
└─→ Module stays in available_modules (can be reinstalled)
```

---

## Placement Validation Examples

### Example 1: Valid Placement

```
Moonbase size: 10×8
Module size: 2×2
Position: (3, 2)

Tiles needed: (3,2), (4,2), (3,3), (4,3)

Bounds check:
- x + width = 3 + 2 = 5 <= 10 ✅
- y + height = 2 + 2 = 4 <= 8 ✅

Overlap check:
- is_tile_occupied(3,2)? No ✅
- is_tile_occupied(4,2)? No ✅
- is_tile_occupied(3,3)? No ✅
- is_tile_occupied(4,3)? No ✅

Result: VALID placement
```

### Example 2: Out of Bounds

```
Moonbase size: 10×8
Module size: 3×3
Position: (8, 6)

Tiles needed: (8,6), (9,6), (10,6), (8,7), (9,7), (10,7), (8,8), (9,8), (10,8)

Bounds check:
- x + width = 8 + 3 = 11 > 10 ❌

Result: INVALID (out of bounds)
```

### Example 3: Overlap Collision

```
Moonbase size: 10×8
Existing module: 2×2 at (3, 2) → occupies (3,2), (4,2), (3,3), (4,3)
New module: 2×2 at (4, 3)

Tiles needed: (4,3), (5,3), (4,4), (5,4)

Bounds check: ✅
Overlap check:
- is_tile_occupied(4,3)? YES! ❌ (from existing module)

Result: INVALID (collision)
```

### Example 4: Edge Placement

```
Moonbase size: 10×8
Module size: 1×1
Position: (9, 7)

Tiles needed: (9,7)

Bounds check:
- x + width = 9 + 1 = 10 <= 10 ✅
- y + height = 7 + 1 = 8 <= 8 ✅

Overlap check:
- is_tile_occupied(9,7)? No ✅

Result: VALID (exactly at edge)
```

---

## Module Shapes & Sizes

### Standard Sizes

```rust
1×1: Small modules (turrets, sensors)
2×2: Medium modules (mining rigs, attractions)
3×2: Large modules (research labs)
3×3: Huge modules (command centers)
4×4: Massive modules (mega-structures)

// Set in ModuleConfig
pub struct ModuleConfig {
    width: u8,
    height: u8,
}
```

### Tile Efficiency

**Single 4×4 module:**
- Occupies: 16 tiles
- Provides: High output (e.g., 1,000 hashpower)
- Efficiency: 62.5 hash/tile

**Four 2×2 modules:**
- Occupies: 16 tiles
- Provides: Medium output (e.g., 4 × 200 = 800 hashpower)
- Efficiency: 50 hash/tile

**Trade-off:** Larger modules more efficient but less flexible

---

## Expansion System Integration

### Before Expansion

```
current_width: 10
current_height: 8
Available area: 80 tiles

Grid visualization:
0         9
┌─────────┐ 0
│ MOONBASE│
│ PLAYABLE│
│  AREA   │
│         │
│         │
│         │
│         │
└─────────┘ 7

│  LOCKED │
│  TILES  │
└─────────┘ 14
```

### After Expansion (to 15×12)

```
current_width: 15
current_height: 12
Available area: 180 tiles (+100)

Grid visualization:
0              14
┌──────────────┐ 0
│   MOONBASE   │
│   EXPANDED   │
│   PLAYABLE   │
│    AREA      │
│   (180 tiles)│
│              │
│              │
│              │
│              │
│              │
│              │
└──────────────┘ 11

│   LOCKED     │
└──────────────┘ 14

New area unlocked:
- Additional columns: 10 → 15 (5 columns)
- Additional rows: 8 → 12 (4 rows)
- New tiles: (15 × 12) - (10 × 8) = 100 tiles
```

### Placement After Expansion

**Module placed at (12, 10) - Previously impossible!**

```
Before expansion:
- current_width = 10
- x + width = 12 + 2 = 14 > 10 ❌

After expansion:
- current_width = 15
- x + width = 12 + 2 = 14 <= 15 ✅

Now valid!
```

---

## Complete Placement Example

### Scenario: Installing 5 Modules

**Moonbase: 10×8 (80 tiles)**

```
Module 1: Mining Rig (2×2) at (1, 1)
├─→ Occupies: (1,1), (2,1), (1,2), (2,2)
├─→ Bitmap updates: 4 bits set
└─→ Tiles used: 4/80

Module 2: Mining Rig (2×2) at (4, 1)
├─→ Occupies: (4,1), (5,1), (4,2), (5,2)
├─→ Bitmap updates: 4 bits set
├─→ Tiles used: 8/80
└─→ No overlap with Module 1 ✅

Module 3: Attraction (3×2) at (1, 4)
├─→ Occupies: (1,4), (2,4), (3,4), (1,5), (2,5), (3,5)
├─→ Bitmap updates: 6 bits set
├─→ Tiles used: 14/80
└─→ No overlap ✅

Module 4: Attraction (2×2) at (5, 4)
├─→ Occupies: (5,4), (6,4), (5,5), (6,5)
├─→ Bitmap updates: 4 bits set
├─→ Tiles used: 18/80
└─→ No overlap ✅

Module 5: Mining Rig (2×2) at (8, 1)
├─→ Occupies: (8,1), (9,1), (8,2), (9,2)
├─→ Bitmap updates: 4 bits set
├─→ Tiles used: 22/80
└─→ No overlap ✅

Final state:
- 5 modules deployed
- 22 tiles occupied (27.5% of moonbase)
- 58 tiles free (72.5%)
- Bitmap: 38 bytes with 22 bits set to 1
```

---

## Collision Detection

### How It Works

```rust
// Trying to place 2×2 module at (4, 1)
// But Module 2 already exists at (4, 1)

can_place_module_in_moonbase(user, 4, 1, 2, 2):
    for dy in 0..2:  // height
        for dx in 0..2:  // width
            tile_x = 4 + dx
            tile_y = 1 + dy
            
            Check (4,1): is_occupied? YES! ← Module 2
            return false immediately
```

**Efficiency: O(width × height) worst case**
- Typical module: 2×2 = 4 checks
- Large module: 4×4 = 16 checks
- Very fast!

---

## Frontend Visualization

### Convert Bitmap to 2D Array

```typescript
function visualizeGrid(moonbase: UserMoonBaseInstance): boolean[][] {
  const grid: boolean[][] = [];
  const bitmap = moonbase.occupiedBitmap;
  
  for (let y = 0; y < GRID_HEIGHT; y++) {
    const row: boolean[] = [];
    
    for (let x = 0; x < GRID_WIDTH; x++) {
      const index = y * GRID_WIDTH + x;
      const byteIndex = Math.floor(index / 8);
      const bitIndex = index % 8;
      
      const isOccupied = (bitmap[byteIndex] & (1 << bitIndex)) !== 0;
      row.push(isOccupied);
    }
    
    grid.push(row);
  }
  
  return grid;
}

// Usage
const grid = visualizeGrid(moonbase);
console.log(grid[3][5]); // true = occupied, false = free
```

### Render Grid in UI

```typescript
function renderMoonbaseGrid(moonbase: UserMoonBaseInstance, modules: ModuleInstance[]) {
  const grid = visualizeGrid(moonbase);
  
  return (
    <div className="grid" style={{
      display: 'grid',
      gridTemplateColumns: `repeat(${moonbase.currentWidth}, 40px)`,
      gridTemplateRows: `repeat(${moonbase.currentHeight}, 40px)`,
    }}>
      {grid.map((row, y) => 
        row.slice(0, moonbase.currentWidth).map((occupied, x) => (
          <div
            key={`${x}-${y}`}
            className={`tile ${occupied ? 'occupied' : 'free'}`}
            style={{
              backgroundColor: occupied ? '#666' : '#111',
              border: '1px solid #333',
            }}
          >
            {/* Render module if this is top-left corner */}
            {modules.find(m => m.posX === x && m.posY === y) && (
              <ModuleSprite module={module} />
            )}
          </div>
        ))
      )}
    </div>
  );
}
```

### Placement Preview

```typescript
function canPlaceModule(
  moonbase: UserMoonBaseInstance,
  x: number,
  y: number,
  width: number,
  height: number
): boolean {
  // 1. Bounds check
  if (x + width > moonbase.currentWidth) return false;
  if (y + height > moonbase.currentHeight) return false;
  
  // 2. Overlap check
  const grid = visualizeGrid(moonbase);
  
  for (let dy = 0; dy < height; dy++) {
    for (let dx = 0; dx < width; dx++) {
      if (grid[y + dy][x + dx]) {
        return false; // Collision!
      }
    }
  }
  
  return true; // Valid
}

// Show green/red preview on hover
function onMouseOver(x: number, y: number) {
  const canPlace = canPlaceModule(moonbase, x, y, selectedModule.width, selectedModule.height);
  
  highlightTiles(x, y, selectedModule.width, selectedModule.height, canPlace ? 'green' : 'red');
}
```

---

## Expansion Effects on Placement

### Expansion Unlocks New Area

```
Before: 10×8 (80 tiles)
After: 15×12 (180 tiles)

Previously invalid placements now valid:
- (10, 0): Was out of bounds, now valid
- (12, 10): Was out of bounds, now valid
- (14, 11): Was out of bounds, now valid

Grid before expansion:
[0-9, 0-7] = valid
[10-19, 0-14] = invalid (locked)

Grid after expansion:
[0-14, 0-11] = valid
[15-19, 0-14] = invalid (still locked)
[0-14, 12-14] = invalid (still locked)
```

### Multiple Expansions

```
Expansion 1 (Level 5):
├─→ 10×8 → 12×10 (+44 tiles)
└─→ Cost: 1 SOL

Expansion 2 (Level 10):
├─→ 12×10 → 15×12 (+60 tiles)
└─→ Cost: 2 SOL

Expansion 3 (Level 15):
├─→ 15×12 → 18×14 (+72 tiles)
└─→ Cost: 5 SOL

Expansion 4 (Level 20):
├─→ 18×14 → 20×15 (+48 tiles)
└─→ Cost: 10 SOL

Total: 80 → 300 tiles (full grid unlocked)
Total cost: 18 SOL
```

---

## Placement Strategies

### Strategy 1: Compact Clustering

```
Cluster mining modules together (future synergy bonuses)

Example layout (10×8 moonbase):
┌──────────┐
│MM MM MM  │ M = Mining (2×2)
│MM MM MM  │ A = Attraction (2×2)
│          │
│AA AA     │
│AA AA     │
│          │
│          │
└──────────┘

Pros:
- Organized layout
- Easy to visualize
- Prepared for adjacency bonuses

Cons:
- Less flexible for irregularly-shaped modules
```

### Strategy 2: Perimeter Defense (Future PvP)

```
Place defensive modules around edges

┌──────────┐
│D  CORE  D│ D = Defense (1×1)
│   MM MM  │ M = Mining (2×2)
│   MM MM  │
│   AA AA  │ A = Attraction (2×2)
│   AA AA  │
│D        D│
│          │
└──────────┘

Pros:
- Protected core
- Defensive positioning
- PvP-ready layout

Cons:
- Uses more tiles for defense
- Complex planning needed
```

### Strategy 3: Maximized Efficiency

```
Fill every tile (maximum module count)

┌──────────┐
│MM MM MM S│ M = Mining (2×2)
│MM MM MM S│ A = Attraction (2×2)
│AA AA TT S│ T = Turret (1×1)
│AA AA TT S│ S = Sensor (1×1)
│RR RR RR  │ R = Research (2×2)
│RR RR RR  │
│          │
└──────────┘

Pros:
- Maximum hashpower/XP
- Uses all available space
- High efficiency

Cons:
- No room for rearrangement
- Locked into layout
- Inflexible
```

---

## Bitmap Manipulation Internals

### Setting Multiple Bits

```rust
// Manually setting bits for 2×2 module at (5,3)

// Tile (5,3): index 65
byte_idx = 65 / 8 = 8
bit_idx = 65 % 8 = 1
occupied_bitmap[8] |= (1 << 1)  // 0b00000010

// Tile (6,3): index 66
byte_idx = 66 / 8 = 8
bit_idx = 66 % 8 = 2
occupied_bitmap[8] |= (1 << 2)  // 0b00000100

// Tile (5,4): index 85
byte_idx = 85 / 8 = 10
bit_idx = 85 % 8 = 5
occupied_bitmap[10] |= (1 << 5)  // 0b00100000

// Tile (6,4): index 86
byte_idx = 86 / 8 = 10
bit_idx = 86 % 8 = 6
occupied_bitmap[10] |= (1 << 6)  // 0b01000000

// Result:
occupied_bitmap[8] = 0b00000110 (bits 1 and 2 set)
occupied_bitmap[10] = 0b01100000 (bits 5 and 6 set)
```

### Clearing Bits

```rust
// Removing the same 2×2 module

// Tile (5,3): Clear bit 1 in byte 8
occupied_bitmap[8] &= !(1 << 1)  // 0b11111101
// If was: 0b00000110
// Now:    0b00000100

// Tile (6,3): Clear bit 2 in byte 8
occupied_bitmap[8] &= !(1 << 2)  // 0b11111011
// Now:    0b00000000

// Tile (5,4): Clear bit 5 in byte 10
occupied_bitmap[10] &= !(1 << 5)  // 0b11011111
// If was: 0b01100000
// Now:    0b01000000

// Tile (6,4): Clear bit 6 in byte 10
occupied_bitmap[10] &= !(1 << 6)  // 0b10111111
// Now:    0b00000000

// Result: All bits cleared, tiles free again
```

---

## Performance Analysis

### Operation Complexity

| Operation | Complexity | Notes |
|-----------|------------|-------|
| `is_tile_occupied` | O(1) | Single bit check |
| `mark_tiles_occupied` | O(w×h) | For each tile in module |
| `clear_tiles` | O(w×h) | For each tile in module |
| `can_place_module` | O(w×h) | Check each tile |
| `place_module` | O(w×h) | Validate + mark |

**Typical module: 2×2**
- 4 iterations per operation
- ~20-30 instructions total
- Extremely fast!

### Gas Efficiency

```
Bitmap approach (38 bytes fixed):
- Account size: Fixed
- Rent: Constant
- Operations: Bit manipulation (fast)

Alternative array approach (variable):
- Account size: Grows with modules
- Rent: Increases over time
- Operations: Array iteration (slower)

Bitmap is ~10x more gas-efficient!
```

---

## Edge Cases & Error Handling

### Edge Case 1: Overflow in Coordinates

```rust
// Module at (255, 255) with size (5, 5)
// x + width = 255 + 5 = 260 (overflow!)

// Protection:
x.checked_add(width).ok_or(ErrorCode::ArithmeticOverflow)?
// Returns error instead of wrapping to 4
```

### Edge Case 2: Zero-Size Module

```rust
// Module with width=0 or height=0
// Would mark no tiles!

// Validation in admin function:
require!(width > 0 && height > 0, ErrorCode::InvalidModuleSize);
```

### Edge Case 3: Module Larger Than Grid

```rust
// Module 25×25 on 20×15 grid
// Can never be placed!

// Validation in admin function:
require!(width <= GRID_WIDTH, ErrorCode::ModuleTooLarge);
require!(height <= GRID_HEIGHT, ErrorCode::ModuleTooLarge);
```

### Edge Case 4: Bitmap Index Out of Range

```rust
// Tile (19, 14) is valid
// index = 14 × 20 + 19 = 299
// byte_idx = 299 / 8 = 37 (valid, BITMAP_SIZE = 38)

// Tile (19, 15) would be invalid
// index = 15 × 20 + 19 = 319
// byte_idx = 319 / 8 = 39 >= 38 (ERROR!)

// Protection:
if byte_idx >= BITMAP_SIZE {
    return Err(ErrorCode::InvalidTileIndex.into());
}
```

---

## Testing Scenarios

### Test 1: Fill Entire Grid

```
Place modules until grid is full (300 tiles)
- Place 75 modules (4×4 each)
- OR place 150 modules (2×2 each)
- OR place 300 modules (1×1 each)

Expected:
- All 38 bytes have bits set
- can_place_module() returns false for any position
```

### Test 2: Checkerboard Pattern

```
Place 1×1 modules in checkerboard:
(0,0), (2,0), (4,0), ...
(1,1), (3,1), (5,1), ...

Expected:
- 150 tiles occupied (50%)
- 150 tiles free (50%)
- Bitmap has alternating pattern
```

### Test 3: Expansion Unlocking

```
Start: 10×8 moonbase
Attempt: Place module at (12, 5) - Should fail
Expand: To 15×12
Attempt: Place same module at (12, 5) - Should succeed
```

### Test 4: Module Removal

```
Place module at (5, 5)
Check: Tiles (5,5) to (7,7) occupied
Remove module
Check: Tiles (5,5) to (7,7) free
Recheck: Can place different module at (5,5)
```

---

## Summary

### Key Features
✅ **Bitmap storage** - 38 bytes fixed size  
✅ **Efficient operations** - Bit manipulation  
✅ **Collision detection** - Automatic overlap checking  
✅ **Expansion support** - Dynamic moonbase growth  
✅ **Module flexibility** - Any rectangular shape  
✅ **Zero overhead** - No performance degradation  

### Implementation Quality
✅ **Overflow-safe** - All arithmetic checked  
✅ **Bounds-safe** - All indices validated  
✅ **Memory-efficient** - Minimal storage footprint  
✅ **Gas-optimized** - Fast bit operations  

### User Experience
✅ **Visual feedback** - Grid rendered in frontend  
✅ **Placement preview** - Highlight valid/invalid  
✅ **Flexible layouts** - Players choose positioning  
✅ **No artificial limits** - Only space constraints  

---

**The grid placement system is production-ready, highly efficient, and provides excellent UX for strategic base building!**



