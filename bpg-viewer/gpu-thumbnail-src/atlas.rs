//! Thumbnail atlas — a GPU buffer divided into fixed-size tiles,
//! with LRU-based eviction for recycling tiles.

use crate::error::{Result, ThumbnailError};

/// Tile index in the atlas (0 .. grid_dim²-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileIndex(pub u16);

/// Per-tile state.
#[derive(Debug, Clone)]
pub enum TileState {
    Free,
    Occupied {
        /// Application-defined id for the source image.
        source_id: u64,
        /// Frame counter at last access (for LRU).
        last_used: u64,
    },
}

/// Atlas layout + tile allocator with LRU eviction and tile recycling.
pub struct ThumbnailAtlas {
    /// Tile states, indexed by tile ordinal.
    pub tiles: Vec<TileState>,
    /// Edge length of each square tile in pixels (e.g. 256).
    pub tile_size: u32,
    /// Edge length of the atlas in pixels (e.g. 4096).
    pub atlas_size: u32,
    /// Number of tiles per row/column.
    pub grid_dim: u32,
    /// Total tile count.
    pub tile_count: u32,
    /// Monotonic frame counter for LRU ordering.
    pub frame: u64,
    /// Free list for recycled tiles (LIFO for GPU cache efficiency).
    free_list: Vec<TileIndex>,
    /// Next fresh tile index (for tiles never released before).
    next_fresh: u16,
}

impl ThumbnailAtlas {
    /// Create a new atlas descriptor (does NOT allocate the GPU buffer — that
    /// is handled by `ThumbnailPipeline`).
    pub fn new(atlas_size: u32, tile_size: u32) -> Self {
        let grid_dim = atlas_size / tile_size;
        let tile_count = grid_dim * grid_dim;
        let tiles = vec![TileState::Free; tile_count as usize];
        Self {
            tiles,
            tile_size,
            atlas_size,
            grid_dim,
            tile_count,
            frame: 0,
            free_list: Vec::with_capacity(tile_count as usize),
            next_fresh: 0,
        }
    }

    /// Advance the frame counter.  Call once per "processing round".
    pub fn tick(&mut self) {
        self.frame += 1;
    }

    /// Look up a tile by `source_id`.  Returns the index and touches LRU.
    pub fn lookup(&mut self, source_id: u64) -> Option<TileIndex> {
        for (i, tile) in self.tiles.iter_mut().enumerate() {
            if let TileState::Occupied {
                source_id: sid,
                last_used,
            } = tile
            {
                if *sid == source_id {
                    *last_used = self.frame;
                    return Some(TileIndex(i as u16));
                }
            }
        }
        None
    }

    /// Allocate a tile for `source_id` with recycling support.
    ///
    /// 1. Reuses an existing tile with the same `source_id` if present.
    /// 2. Pops from the free list (LIFO for GPU cache efficiency).
    /// 3. Allocates a fresh tile if available.
    /// 4. Evicts the least-recently-used tile as last resort.
    pub fn allocate_tile(&mut self, source_id: u64) -> Result<TileIndex> {
        // CRITICAL: Advance frame counter for LRU to work correctly
        self.tick();
        
        // Already cached?
        if let Some(idx) = self.lookup(source_id) {
            return Ok(idx);
        }

        // Try recycled tile first (LIFO keeps L2 cache hot)
        if let Some(idx) = self.free_list.pop() {
            self.tiles[idx.0 as usize] = TileState::Occupied {
                source_id,
                last_used: self.frame,
            };
            return Ok(idx);
        }

        // Try fresh tile
        if (self.next_fresh as u32) < self.tile_count {
            let idx = TileIndex(self.next_fresh);
            self.next_fresh += 1;
            self.tiles[idx.0 as usize] = TileState::Occupied {
                source_id,
                last_used: self.frame,
            };
            return Ok(idx);
        }

        // Last resort: evict LRU (should rarely happen with proper release_tile usage)
        let lru_idx = self.find_lru().ok_or(ThumbnailError::AtlasFull(self.tile_count))?;
        eprintln!("[Atlas] WARNING: LRU eviction forced for source_id {} - consider calling release_tile() earlier", source_id);
        self.tiles[lru_idx as usize] = TileState::Occupied {
            source_id,
            last_used: self.frame,
        };
        Ok(TileIndex(lru_idx as u16))
    }

    /// Release a tile back to the free list for recycling.
    /// 
    /// Call this after encoding the thumbnail to allow unlimited throughput.
    /// Tiles are recycled LIFO to keep GPU L2 cache hot.
    pub fn release_tile(&mut self, idx: TileIndex) {
        if (idx.0 as u32) >= self.tile_count {
            return;
        }
        
        self.tiles[idx.0 as usize] = TileState::Free;
        self.free_list.push(idx);
    }

    /// Free a specific tile.
    pub fn free_tile(&mut self, idx: TileIndex) {
        if (idx.0 as u32) < self.tile_count {
            self.tiles[idx.0 as usize] = TileState::Free;
        }
    }

    /// Pixel offset (x, y) in the atlas for the given tile.
    pub fn tile_pixel_offset(&self, idx: TileIndex) -> (u32, u32) {
        let col = idx.0 as u32 % self.grid_dim;
        let row = idx.0 as u32 / self.grid_dim;
        (col * self.tile_size, row * self.tile_size)
    }

    /// Number of currently occupied tiles.
    pub fn occupied_count(&self) -> u32 {
        self.tiles
            .iter()
            .filter(|t| matches!(t, TileState::Occupied { .. }))
            .count() as u32
    }

    /// Number of free tiles.
    pub fn free_count(&self) -> u32 {
        self.tile_count - self.occupied_count()
    }

    // ── Internal ────────────────────────────────────────────────────────

    /// Find the tile with the smallest `last_used` value.
    /// Returns None only if ALL tiles are Free (should never happen when called from allocate_tile).
    fn find_lru(&self) -> Option<u16> {
        let mut best_idx: Option<u16> = None;
        let mut best_frame = u64::MAX;

        for (i, tile) in self.tiles.iter().enumerate() {
            if let TileState::Occupied { last_used, .. } = tile {
                if *last_used < best_frame {
                    best_frame = *last_used;
                    best_idx = Some(i as u16);
                }
            }
        }

        if best_idx.is_none() {
            eprintln!("[Atlas] WARNING: find_lru() returned None - no occupied tiles found!");
        }
        best_idx
    }
}


