use windows::core::{GUID, PWSTR, PCWSTR} ;
use windows::Win32::System::Com::{CoInitializeEx, CoCreateInstance, CoTaskMemFree, CLSCTX_ALL, COINIT_DISABLE_OLE1DDE, COINIT_MULTITHREADED};
use windows::Win32::Devices::PortableDevices::IPortableDeviceManager;
use widestring::U16CString;

use crate::device::BasicDevice;
use crate::error::MtpError;

pub struct Provider {}

impl Provider {
    /// Entry point of this crate.
    ///
    /// It internally inits the underlying Windows API.
    /// 
    /// Note: This will attempt to initialize COM with COINIT_MULTITHREADED.
    /// If COM is already initialized with a different threading model, it will
    /// still succeed and use the existing COM initialization. This makes the
    /// library more robust when called from environments that have already
    /// initialized COM (like PowerShell, C# GUI apps, etc).
    pub fn new() -> crate::WindowsResult<Self> {
        unsafe {
            let hr = CoInitializeEx(
                None,
                COINIT_MULTITHREADED
                | COINIT_DISABLE_OLE1DDE, // Setting this flag avoids some overhead associated with Object Linking and Embedding (OLE) 1.0, an obsolete technology. (see https://learn.microsoft.com/en-us/windows/win32/learnwin32/initializing-the-com-library)
            );
            
            // Check the result:
            // - S_OK (0x00000000): COM initialized successfully
            // - S_FALSE (0x00000001): COM was already initialized on this thread
            // - RPC_E_CHANGED_MODE (0x80010106): COM already initialized with different threading model
            //
            // We treat S_FALSE and RPC_E_CHANGED_MODE as acceptable - if COM is already
            // initialized (even with a different threading model), we can still use it.
            // Only fail on other errors.
            match hr {
                Ok(_) => {}, // S_OK or S_FALSE - success
                Err(e) => {
                    // Check if this is the "already initialized with different mode" error
                    const RPC_E_CHANGED_MODE: i32 = -2147417850i32; // 0x80010106 as signed
                    if e.code().0 != RPC_E_CHANGED_MODE {
                        // Some other error - propagate it
                        return Err(e);
                    }
                    // RPC_E_CHANGED_MODE - that's OK, COM is already initialized
                }
            }
        }

        Ok(Self{})
    }

    pub fn enumerate_devices(&self) -> Result<Vec<BasicDevice>, MtpError> {
        let device_mgr: IPortableDeviceManager = unsafe {
            CoCreateInstance(
                &windows::Win32::Devices::PortableDevices::PortableDeviceManager as *const GUID,
                None,
                CLSCTX_ALL,
            )
        }?;

        // How many devices are there?
        let mut dev_count: u32 = 0;
        unsafe {
            device_mgr.RefreshDeviceList()?;
            device_mgr.GetDevices(
                std::ptr::null_mut(),
                &mut dev_count as *mut _
            )?;
        }

        if dev_count == 0 {
            return Ok(Vec::new());
        }

        // Get their IDs
        let mut dev_ids: Vec<PWSTR> = Vec::with_capacity(dev_count as usize);
        let mut fetched_devices: u32 = dev_count;
        unsafe {
            device_mgr.GetDevices(
                dev_ids.as_mut_ptr(),
                &mut fetched_devices as *mut _,
            )
        }?;
        if fetched_devices != dev_count {
            return Err(MtpError::ChangedConditions);
        }
        unsafe { dev_ids.set_len(dev_count as usize) };

        // Build a Rust result type
        let mut devices = Vec::new();
        for dev_id in &dev_ids {
            let dev_id_const = PCWSTR::from_raw(dev_id.as_ptr());
            let friendly_name = get_friendly_name(&device_mgr, dev_id_const)?;
            let string_id = U16CString::from_vec_truncate(unsafe{ dev_id.as_wide() });
            devices.push(BasicDevice::new(string_id, friendly_name));
        }

        // Free memory allocated by the COM API
        for dev_id in dev_ids {
            unsafe{ CoTaskMemFree(Some(dev_id.as_ptr() as *const _)) };
        }

        Ok(devices)
    }
}

fn get_friendly_name(mgr: &IPortableDeviceManager, dev_id: PCWSTR) -> Result<String, MtpError> {
    // How long is the name?
    let mut required_len: u32 = 0;
    unsafe {
        mgr.GetDeviceFriendlyName(
            dev_id,
            PWSTR::null(),
            &mut required_len as *mut u32,
        )
    }?;

    if required_len == 0 {
        return Ok(String::new());
    }

    let mut friendly_name: Vec<u16> = Vec::with_capacity(required_len as usize);
    let p_friendly_name = PWSTR::from_raw(friendly_name.as_mut_ptr());
    let mut retrieved_len: u32 = required_len;
    unsafe {
        mgr.GetDeviceFriendlyName(
            dev_id,
            p_friendly_name,
            &mut retrieved_len as *mut u32,
        )
    }?;
    if retrieved_len != required_len {
        return Err(MtpError::ChangedConditions);
    }
    unsafe { friendly_name.set_len(required_len as usize) };

    Ok(unsafe { p_friendly_name.to_string() }?)
}
