//! Multi-queue D3D12 context with separate COPY and COMPUTE queues.
//!
//! The COPY queue handles staging-buffer uploads (CPU → GPU).
//! The COMPUTE queue dispatches shaders (YCbCr resize, etc.).
//! Cross-queue synchronization uses GPU fences so the compute queue
//! can wait for a copy to complete without stalling the CPU.

use std::sync::atomic::{AtomicU64, Ordering};
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::Graphics::{
        Direct3D::*,
        Direct3D12::*,
        Dxgi::{*, Common::*},
    },
    Win32::System::Threading::*,
};

use crate::error::{Result, ThumbnailError};

/// Raw value of WAIT_TIMEOUT (0x102 = 258).
/// Defined here for portability across windows-rs versions.
const WAIT_TIMEOUT_RAW: u32 = 258;

// ─── Queue type ─────────────────────────────────────────────────────────────

/// Identifies which hardware queue an operation targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueType {
    /// COPY queue — staging ↔ GPU transfers only.
    Copy,
    /// COMPUTE queue — shader dispatches + resource barriers.
    Compute,
}

// ─── Per-queue state ────────────────────────────────────────────────────────

/// Command recording + fence synchronization for one hardware queue.
pub struct CommandQueue {
    pub queue: ID3D12CommandQueue,
    pub allocator: ID3D12CommandAllocator,
    pub list: ID3D12GraphicsCommandList,
    pub fence: ID3D12Fence,
    pub fence_event: HANDLE,
    pub fence_value: u64,
    pub queue_type: QueueType,
}

impl CommandQueue {
    /// Wait for all previously submitted work on this queue, with timeout.
    pub fn wait_for_gpu(&mut self, timeout_ms: u32) -> Result<()> {
        unsafe {
            let target = self.fence_value;
            self.queue.Signal(&self.fence, target)?;
            self.fence_value += 1;

            if self.fence.GetCompletedValue() < target {
                self.fence.SetEventOnCompletion(target, self.fence_event)?;
                let result = WaitForSingleObject(self.fence_event, timeout_ms);
                if result.0 == WAIT_TIMEOUT_RAW {
                    return Err(ThumbnailError::GpuTimeout(timeout_ms));
                }
            }
        }
        Ok(())
    }

    /// Reset command allocator + list for new recording.
    pub fn reset(&self) -> Result<()> {
        unsafe {
            self.allocator.Reset()?;
            self.list.Reset(&self.allocator, None)?;
        }
        Ok(())
    }

    /// Close list → submit → wait with timeout.
    pub fn execute_and_wait(&mut self, timeout_ms: u32) -> Result<()> {
        unsafe {
            self.list.Close()?;
            let lists = [Some(self.list.cast::<ID3D12CommandList>()?)];
            self.queue.ExecuteCommandLists(&lists);
        }
        self.wait_for_gpu(timeout_ms)
    }

    /// Close list → submit → return fence value (no CPU wait).
    pub fn execute_async(&mut self) -> Result<u64> {
        unsafe {
            self.list.Close()?;
            let lists = [Some(self.list.cast::<ID3D12CommandList>()?)];
            self.queue.ExecuteCommandLists(&lists);
            let signal_value = self.fence_value;
            self.queue.Signal(&self.fence, signal_value)?;
            self.fence_value += 1;
            Ok(signal_value)
        }
    }

    /// True if `fence_value` has been reached by the GPU.
    pub fn is_complete(&self, fence_value: u64) -> bool {
        unsafe { self.fence.GetCompletedValue() >= fence_value }
    }

    /// Block until `fence_value` is reached, or timeout.
    pub fn wait_for_value(&self, fence_value: u64, timeout_ms: u32) -> Result<()> {
        unsafe {
            if self.fence.GetCompletedValue() >= fence_value {
                return Ok(());
            }
            self.fence.SetEventOnCompletion(fence_value, self.fence_event)?;
            let result = WaitForSingleObject(self.fence_event, timeout_ms);
            if result.0 == WAIT_TIMEOUT_RAW {
                return Err(ThumbnailError::GpuTimeout(timeout_ms));
            }
        }
        Ok(())
    }
}

impl Drop for CommandQueue {
    fn drop(&mut self) {
        // Best-effort drain before tear-down.
        let _ = self.wait_for_gpu(5000);
        unsafe {
            if !self.fence_event.is_invalid() {
                let _ = CloseHandle(self.fence_event);
            }
        }
    }
}

// SAFETY: D3D12 COM objects are thread-safe; external locking is the caller's
// responsibility when recording commands concurrently.
unsafe impl Send for CommandQueue {}
unsafe impl Sync for CommandQueue {}

// ─── Multi-queue context ────────────────────────────────────────────────────

/// Owns the D3D12 device, two hardware queues (COPY + COMPUTE),
/// and a shader-visible descriptor heap.
pub struct MultiQueueContext {
    pub device: ID3D12Device,
    pub copy_queue: CommandQueue,
    pub compute_queue: CommandQueue,
    pub descriptor_heap: ID3D12DescriptorHeap,
    pub cbv_srv_uav_size: u32,
    pub dedicated_vram: u64,
    pub allocated_bytes: AtomicU64,
    pub verbose: bool,
}

impl MultiQueueContext {
    /// Create the context, auto-selecting the best GPU adapter.
    pub fn new() -> Result<Self> {
        let verbose = std::env::var("THUMB_GPU_VERBOSE").ok().as_deref() == Some("1");
        Self::new_with_verbose(verbose)
    }

    pub fn new_with_verbose(verbose: bool) -> Result<Self> {
        unsafe {
            // ── Debug layer (opt-in) ──
            #[cfg(feature = "debug-layers")]
            {
                if let Ok(debug) = D3D12GetDebugInterface::<ID3D12Debug>() {
                    debug.EnableDebugLayer();
                    if verbose {
                        eprintln!("[MultiQueue] D3D12 debug layer enabled");
                    }
                }
            }

            let dxgi_factory: IDXGIFactory4 = CreateDXGIFactory1()?;

            // ── Adapter selection ──
            let mut adapter: Option<IDXGIAdapter1> = None;
            let mut dedicated_vram: u64 = 0;
            for i in 0.. {
                match dxgi_factory.EnumAdapters1(i) {
                    Ok(candidate) => {
                        let desc = candidate.GetDesc1()?;
                        if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
                            continue;
                        }
                        let mut test_device: Option<ID3D12Device> = None;
                        if D3D12CreateDevice(
                            &candidate,
                            D3D_FEATURE_LEVEL_11_0,
                            &mut test_device,
                        )
                        .is_ok()
                        {
                            dedicated_vram = desc.DedicatedVideoMemory as u64;
                            if verbose {
                                let name = desc_name(&desc.Description);
                                eprintln!(
                                    "[MultiQueue] Adapter: {} ({} MB VRAM)",
                                    name,
                                    dedicated_vram / (1024 * 1024)
                                );
                            }
                            adapter = Some(candidate);
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }

            let adapter = adapter.ok_or_else(|| {
                ThumbnailError::InitFailed("No compatible D3D12 adapter found".into())
            })?;

            // ── Device ──
            let mut device_opt: Option<ID3D12Device> = None;
            D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device_opt)?;
            let device = device_opt.ok_or_else(|| {
                ThumbnailError::InitFailed("D3D12CreateDevice returned None".into())
            })?;

            // ── Queues ──
            let copy_queue = Self::create_command_queue(
                &device,
                D3D12_COMMAND_LIST_TYPE_COPY,
                QueueType::Copy,
                verbose,
            )?;
            let compute_queue = Self::create_command_queue(
                &device,
                D3D12_COMMAND_LIST_TYPE_COMPUTE,
                QueueType::Compute,
                verbose,
            )?;

            // ── Descriptor heap (shader-visible CBV/SRV/UAV) ──
            let heap_desc = D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                NumDescriptors: 16,
                Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
                NodeMask: 0,
            };
            let descriptor_heap: ID3D12DescriptorHeap =
                device.CreateDescriptorHeap(&heap_desc)?;
            let cbv_srv_uav_size = device
                .GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);

            if verbose {
                eprintln!(
                    "[MultiQueue] Initialized: COPY + COMPUTE queues, {} descriptor slots",
                    16
                );
            }

            Ok(Self {
                device,
                copy_queue,
                compute_queue,
                descriptor_heap,
                cbv_srv_uav_size,
                dedicated_vram,
                allocated_bytes: AtomicU64::new(0),
                verbose,
            })
        }
    }

    // ── Internal: create one queue + allocator + list + fence ──

    fn create_command_queue(
        device: &ID3D12Device,
        list_type: D3D12_COMMAND_LIST_TYPE,
        queue_type: QueueType,
        verbose: bool,
    ) -> Result<CommandQueue> {
        unsafe {
            let queue_desc = D3D12_COMMAND_QUEUE_DESC {
                Type: list_type,
                Priority: D3D12_COMMAND_QUEUE_PRIORITY_NORMAL.0,
                Flags: D3D12_COMMAND_QUEUE_FLAG_NONE,
                NodeMask: 0,
            };
            let queue: ID3D12CommandQueue = device.CreateCommandQueue(&queue_desc)?;
            let allocator: ID3D12CommandAllocator =
                device.CreateCommandAllocator(list_type)?;
            let list: ID3D12GraphicsCommandList =
                device.CreateCommandList(0, list_type, &allocator, None)?;
            list.Close()?; // start closed — caller resets before recording

            let fence: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;
            let fence_event = CreateEventW(None, false, false, PCWSTR::null())?;
            if fence_event.is_invalid() {
                return Err(ThumbnailError::InitFailed(format!(
                    "{:?} CreateEventW failed",
                    queue_type
                )));
            }

            if verbose {
                eprintln!("[MultiQueue] Created {:?} queue", queue_type);
            }

            Ok(CommandQueue {
                queue,
                allocator,
                list,
                fence,
                fence_event,
                fence_value: 1,
                queue_type,
            })
        }
    }

    // ── Buffer / texture creation ───────────────────────────────────────

    /// GPU-local buffer (DEFAULT heap, UAV-capable).
    pub fn create_gpu_buffer(&self, size: usize) -> Result<ID3D12Resource> {
        self.create_buffer_inner(
            size,
            D3D12_HEAP_TYPE_DEFAULT,
            D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS,
        )
    }

    /// Upload buffer (CPU-writable, SRV-readable by GPU).
    pub fn create_upload_buffer(&self, size: usize) -> Result<ID3D12Resource> {
        self.create_buffer_inner(size, D3D12_HEAP_TYPE_UPLOAD, D3D12_RESOURCE_FLAG_NONE)
    }

    /// Readback buffer (GPU → CPU).
    pub fn create_readback_buffer(&self, size: usize) -> Result<ID3D12Resource> {
        self.create_buffer_inner(
            size,
            D3D12_HEAP_TYPE_READBACK,
            D3D12_RESOURCE_FLAG_NONE,
        )
    }

    fn create_buffer_inner(
        &self,
        size: usize,
        heap_type: D3D12_HEAP_TYPE,
        flags: D3D12_RESOURCE_FLAGS,
    ) -> Result<ID3D12Resource> {
        unsafe {
            let aligned = (size + 255) & !255; // 256-byte align

            let heap_props = D3D12_HEAP_PROPERTIES {
                Type: heap_type,
                CPUPageProperty: D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
                MemoryPoolPreference: D3D12_MEMORY_POOL_UNKNOWN,
                CreationNodeMask: 0,
                VisibleNodeMask: 0,
            };

            let desc = D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                Alignment: 0,
                Width: aligned as u64,
                Height: 1,
                DepthOrArraySize: 1,
                MipLevels: 1,
                Format: DXGI_FORMAT_UNKNOWN,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                Flags: flags,
            };

            let initial_state = match heap_type {
                D3D12_HEAP_TYPE_UPLOAD => D3D12_RESOURCE_STATE_GENERIC_READ,
                D3D12_HEAP_TYPE_READBACK => D3D12_RESOURCE_STATE_COPY_DEST,
                _ => D3D12_RESOURCE_STATE_COMMON,
            };

            let mut resource: Option<ID3D12Resource> = None;
            self.device.CreateCommittedResource(
                &heap_props,
                D3D12_HEAP_FLAG_NONE,
                &desc,
                initial_state,
                None,
                &mut resource,
            )?;

            let resource = resource.ok_or_else(|| {
                ThumbnailError::ResourceFailed(format!(
                    "Buffer returned None (size={})",
                    size
                ))
            })?;

            self.allocated_bytes
                .fetch_add(aligned as u64, Ordering::Relaxed);

            if self.verbose {
                eprintln!(
                    "[MultiQueue] Buffer: {} bytes (heap={:?}, total={} MB)",
                    aligned,
                    heap_type,
                    self.allocated_bytes.load(Ordering::Relaxed) / (1024 * 1024)
                );
            }

            Ok(resource)
        }
    }

    // ── Descriptor helpers ──────────────────────────────────────────────

    /// CPU descriptor handle at `index` in the shader-visible heap.
    pub fn cpu_descriptor(&self, index: u32) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        unsafe {
            let mut h = self.descriptor_heap.GetCPUDescriptorHandleForHeapStart();
            h.ptr += (index * self.cbv_srv_uav_size) as usize;
            h
        }
    }

    /// GPU descriptor handle at `index`.
    pub fn gpu_descriptor(&self, index: u32) -> D3D12_GPU_DESCRIPTOR_HANDLE {
        unsafe {
            let mut h = self.descriptor_heap.GetGPUDescriptorHandleForHeapStart();
            h.ptr += (index * self.cbv_srv_uav_size) as u64;
            h
        }
    }

    // ── Cross-queue sync ────────────────────────────────────────────────

    /// Make the compute queue wait until the copy queue reaches `copy_fence_value`.
    /// This is the core pipelining primitive: copy finishes → compute starts.
    pub fn compute_waits_for_copy(&mut self, copy_fence_value: u64) -> Result<()> {
        unsafe {
            self.compute_queue
                .queue
                .Wait(&self.copy_queue.fence, copy_fence_value)?;
        }
        Ok(())
    }
}

// SAFETY: see CommandQueue
unsafe impl Send for MultiQueueContext {}
unsafe impl Sync for MultiQueueContext {}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn desc_name(desc: &[u16; 128]) -> String {
    let len = desc.iter().position(|&c| c == 0).unwrap_or(desc.len());
    String::from_utf16_lossy(&desc[..len])
}
