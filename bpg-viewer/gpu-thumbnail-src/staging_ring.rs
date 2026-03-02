//! Triple-buffered staging ring for CPU → GPU uploads.
//!
//! Each slot is a persistently-mapped UPLOAD-heap buffer.
//! The ring cycles through slots so that one upload can happen
//! while the GPU is still consuming a previous slot's data.

use std::ffi::c_void;
use std::ptr;
use windows::Win32::Graphics::Direct3D12::*;

use crate::dx12_multi_queue::{CommandQueue, MultiQueueContext};
use crate::error::{Result, ThumbnailError};

/// Number of staging slots (triple-buffered).
pub const SLOT_COUNT: usize = 3;

/// Default per-slot capacity: 32 MB (handles images up to ~5800×5800 YCbCr 4:2:0).
pub const DEFAULT_SLOT_CAPACITY: usize = 32 * 1024 * 1024;

// ─── Slot ───────────────────────────────────────────────────────────────────

/// One staging upload buffer with a persistent CPU-mapped pointer.
struct StagingSlot {
    buffer: ID3D12Resource,
    mapped_ptr: *mut u8,
    capacity: usize,
    /// Fence value on the *compute* queue that must complete before this slot
    /// can be reused (the compute shader is the last consumer of this data).
    in_flight_fence: Option<u64>,
}

// SAFETY: The mapped pointer is valid for the lifetime of the resource.
unsafe impl Send for StagingSlot {}
unsafe impl Sync for StagingSlot {}

impl StagingSlot {
    fn new(ctx: &MultiQueueContext, capacity: usize) -> Result<Self> {
        let buffer = ctx.create_upload_buffer(capacity)?;

        // Persistently map — valid until the resource is destroyed.
        let mapped_ptr = unsafe {
            let mut ptr: *mut c_void = ptr::null_mut();
            buffer.Map(0, None, Some(&mut ptr))?;
            ptr as *mut u8
        };

        Ok(Self {
            buffer,
            mapped_ptr,
            capacity,
            in_flight_fence: None,
        })
    }

    /// Copy `data` into the mapped upload buffer.
    fn write(&mut self, data: &[u8]) -> Result<()> {
        if data.len() > self.capacity {
            return Err(ThumbnailError::BufferTooSmall {
                needed: data.len(),
                available: self.capacity,
            });
        }
        unsafe {
            ptr::copy_nonoverlapping(data.as_ptr(), self.mapped_ptr, data.len());
        }
        Ok(())
    }

    /// True if the GPU has finished consuming this slot's data.
    fn is_free(&self, compute_queue: &CommandQueue) -> bool {
        match self.in_flight_fence {
            None => true,
            Some(val) => compute_queue.is_complete(val),
        }
    }
}

// ─── Ring ───────────────────────────────────────────────────────────────────

/// Triple-buffered staging ring.
pub struct StagingRing {
    slots: Vec<StagingSlot>,
    current: usize,
}

impl StagingRing {
    /// Allocate `SLOT_COUNT` staging buffers of `capacity` each.
    pub fn new(ctx: &MultiQueueContext, capacity: usize) -> Result<Self> {
        let mut slots = Vec::with_capacity(SLOT_COUNT);
        for i in 0..SLOT_COUNT {
            let slot = StagingSlot::new(ctx, capacity)?;
            if ctx.verbose {
                eprintln!(
                    "[StagingRing] Slot {} allocated: {} MB",
                    i,
                    capacity / (1024 * 1024)
                );
            }
            slots.push(slot);
        }
        Ok(Self { slots, current: 0 })
    }

    /// Acquire the next free slot, writing `data` into it.
    ///
    /// If all slots are in-flight, waits (with 5 s timeout) on the oldest.
    /// Returns the slot index and a reference to the underlying buffer resource.
    pub fn upload(
        &mut self,
        data: &[u8],
        compute_queue: &mut CommandQueue,
    ) -> Result<(usize, &ID3D12Resource)> {
        // Try each slot starting from `current`.
        for attempt in 0..SLOT_COUNT {
            let idx = (self.current + attempt) % SLOT_COUNT;
            if self.slots[idx].is_free(compute_queue) {
                self.slots[idx].write(data)?;
                self.slots[idx].in_flight_fence = None;
                self.current = (idx + 1) % SLOT_COUNT;
                return Ok((idx, &self.slots[idx].buffer));
            }
        }

        // All slots busy — wait on the current (oldest) slot.
        let idx = self.current;
        if let Some(fence_val) = self.slots[idx].in_flight_fence {
            compute_queue.wait_for_value(fence_val, 5000)?;
        }
        self.slots[idx].write(data)?;
        self.slots[idx].in_flight_fence = None;
        self.current = (idx + 1) % SLOT_COUNT;
        Ok((idx, &self.slots[idx].buffer))
    }

    /// Mark a slot as in-flight with the given compute-queue fence value.
    /// The slot cannot be reused until this fence completes.
    pub fn mark_in_flight(&mut self, slot_index: usize, fence_value: u64) {
        self.slots[slot_index].in_flight_fence = Some(fence_value);
    }

    /// Get the underlying D3D12 resource for slot `index`.
    pub fn buffer(&self, index: usize) -> &ID3D12Resource {
        &self.slots[index].buffer
    }

    /// Capacity of each slot in bytes.
    pub fn slot_capacity(&self) -> usize {
        self.slots[0].capacity
    }
}
