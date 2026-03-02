//! Compute pipeline state creation (root signature + PSO) and the
//! constant-buffer struct shared between Rust and HLSL.

use std::mem;
use std::ptr;

use windows::Win32::Graphics::{
    Direct3D::*,
    Direct3D12::*,
};

use crate::error::{Result, ThumbnailError};

// ─── Constant buffer layout (must match HLSL `YCbCrResizeParams`) ───────────

/// Layout: 12 × u32 = 48 bytes (3 × 16-byte rows).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct YCbCrResizeParams {
    pub y_width: u32,
    pub y_height: u32,
    pub uv_width: u32,
    pub uv_height: u32,
    pub tile_x: u32,
    pub tile_y: u32,
    pub tile_size: u32,
    pub atlas_stride: u32,
    pub y_offset: u32,
    pub cb_offset: u32,
    pub cr_offset: u32,
    pub _pad: u32,
}

impl YCbCrResizeParams {
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self as *const Self as *const u8,
                mem::size_of::<Self>(),
            )
        }
    }
}

// ─── Pipeline state ─────────────────────────────────────────────────────────

/// Compiled root signature + pipeline state object for one compute shader.
pub struct ComputePipelineState {
    pub pipeline_state: ID3D12PipelineState,
    pub root_signature: ID3D12RootSignature,
}

impl ComputePipelineState {
    /// Build a compute PSO from pre-compiled shader bytecode.
    ///
    /// Root signature layout (matches existing resize shaders):
    /// - Param 0 — root CBV at b0   (constants)
    /// - Param 1 — descriptor table: 1 SRV at t0  (input buffer)
    /// - Param 2 — descriptor table: 1 UAV at u0  (output buffer)
    pub fn new(device: &ID3D12Device, shader_bytecode: &[u8]) -> Result<Self> {
        if shader_bytecode.is_empty() {
            return Err(ThumbnailError::ShaderNotCompiled);
        }

        unsafe {
            // Descriptor ranges — need stable addresses for the pointers below.
            let srv_range = [D3D12_DESCRIPTOR_RANGE {
                RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
                NumDescriptors: 1,
                BaseShaderRegister: 0,
                RegisterSpace: 0,
                OffsetInDescriptorsFromTableStart: D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND,
            }];
            let uav_range = [D3D12_DESCRIPTOR_RANGE {
                RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_UAV,
                NumDescriptors: 1,
                BaseShaderRegister: 0,
                RegisterSpace: 0,
                OffsetInDescriptorsFromTableStart: D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND,
            }];

            let root_parameters = [
                // 0: root CBV at b0
                D3D12_ROOT_PARAMETER {
                    ParameterType: D3D12_ROOT_PARAMETER_TYPE_CBV,
                    Anonymous: D3D12_ROOT_PARAMETER_0 {
                        Descriptor: D3D12_ROOT_DESCRIPTOR {
                            ShaderRegister: 0,
                            RegisterSpace: 0,
                        },
                    },
                    ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
                },
                // 1: SRV table (t0)
                D3D12_ROOT_PARAMETER {
                    ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
                    Anonymous: D3D12_ROOT_PARAMETER_0 {
                        DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE {
                            NumDescriptorRanges: srv_range.len() as u32,
                            pDescriptorRanges: srv_range.as_ptr(),
                        },
                    },
                    ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
                },
                // 2: UAV table (u0)
                D3D12_ROOT_PARAMETER {
                    ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
                    Anonymous: D3D12_ROOT_PARAMETER_0 {
                        DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE {
                            NumDescriptorRanges: uav_range.len() as u32,
                            pDescriptorRanges: uav_range.as_ptr(),
                        },
                    },
                    ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
                },
            ];

            let root_sig_desc = D3D12_ROOT_SIGNATURE_DESC {
                NumParameters: root_parameters.len() as u32,
                pParameters: root_parameters.as_ptr(),
                NumStaticSamplers: 0,
                pStaticSamplers: ptr::null(),
                Flags: D3D12_ROOT_SIGNATURE_FLAG_NONE,
            };

            let mut sig_blob: Option<ID3DBlob> = None;
            let mut err_blob: Option<ID3DBlob> = None;

            D3D12SerializeRootSignature(
                &root_sig_desc,
                D3D_ROOT_SIGNATURE_VERSION_1,
                &mut sig_blob,
                Some(&mut err_blob),
            )?;

            let sig_blob = sig_blob.ok_or_else(|| {
                ThumbnailError::ResourceFailed("Root signature blob is None".into())
            })?;

            let root_signature: ID3D12RootSignature = device.CreateRootSignature(
                0,
                std::slice::from_raw_parts(
                    sig_blob.GetBufferPointer() as *const u8,
                    sig_blob.GetBufferSize(),
                ),
            )?;

            // Compute pipeline state
            let pso_desc = D3D12_COMPUTE_PIPELINE_STATE_DESC {
                pRootSignature: mem::transmute_copy(&root_signature),
                CS: D3D12_SHADER_BYTECODE {
                    pShaderBytecode: shader_bytecode.as_ptr() as *const std::ffi::c_void,
                    BytecodeLength: shader_bytecode.len(),
                },
                NodeMask: 0,
                CachedPSO: D3D12_CACHED_PIPELINE_STATE {
                    pCachedBlob: ptr::null(),
                    CachedBlobSizeInBytes: 0,
                },
                Flags: D3D12_PIPELINE_STATE_FLAG_NONE,
            };

            let pipeline_state: ID3D12PipelineState =
                device.CreateComputePipelineState(&pso_desc)?;

            Ok(Self {
                pipeline_state,
                root_signature,
            })
        }
    }
}

// SAFETY: COM pipeline state objects are thread-safe.
unsafe impl Send for ComputePipelineState {}
unsafe impl Sync for ComputePipelineState {}
