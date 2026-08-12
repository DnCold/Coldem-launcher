use serde::{Deserialize, Serialize};
use std::{ffi::c_void, fs, path::PathBuf, ptr};
use tauri::{AppHandle, Manager};

const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct DiscordCredentials {
    pub(crate) access_token: String,
    pub(crate) refresh_token: String,
    pub(crate) token_type: i32,
}

pub(crate) fn load(app: &AppHandle) -> Result<Option<DiscordCredentials>, String> {
    let path = credentials_path(app)?;
    if !path.exists() {
        return Ok(None);
    }

    let encrypted = fs::read(&path)
        .map_err(|error| format!("Could not read the saved Discord session: {error}"))?;
    let plain = unprotect(&encrypted)?;
    serde_json::from_slice(&plain)
        .map(Some)
        .map_err(|error| format!("Could not read the saved Discord session: {error}"))
}

pub(crate) fn save(app: &AppHandle, credentials: &DiscordCredentials) -> Result<(), String> {
    if credentials.access_token.is_empty() || credentials.refresh_token.is_empty() {
        return Err("Discord did not return credentials that can be remembered.".into());
    }
    let path = credentials_path(app)?;
    let plain = serde_json::to_vec(credentials)
        .map_err(|error| format!("Could not prepare the Discord session for storage: {error}"))?;
    let encrypted = protect(&plain)?;
    let parent = path
        .parent()
        .ok_or_else(|| "Could not resolve the Coldem data directory.".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create the Coldem data directory: {error}"))?;
    fs::write(path, encrypted)
        .map_err(|error| format!("Could not save the encrypted Discord session: {error}"))
}

pub(crate) fn clear(app: &AppHandle) -> Result<(), String> {
    let path = credentials_path(app)?;
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Could not remove the saved Discord session: {error}")),
    }
}

fn credentials_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|path| path.join("discord-session.dpapi"))
        .map_err(|error| format!("Could not resolve the Coldem data directory: {error}"))
}

fn protect(plain: &[u8]) -> Result<Vec<u8>, String> {
    crypt(plain, true)
}

fn unprotect(encrypted: &[u8]) -> Result<Vec<u8>, String> {
    crypt(encrypted, false)
}

fn crypt(input: &[u8], protect_data: bool) -> Result<Vec<u8>, String> {
    if input.is_empty() || input.len() > u32::MAX as usize {
        return Err("The Discord session data is invalid.".into());
    }
    let mut input_blob = DataBlob {
        cb_data: input.len() as u32,
        pb_data: input.as_ptr() as *mut u8,
    };
    let mut output_blob = DataBlob {
        cb_data: 0,
        pb_data: ptr::null_mut(),
    };
    let succeeded = unsafe {
        if protect_data {
            CryptProtectData(
                &mut input_blob,
                ptr::null(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output_blob,
            )
        } else {
            CryptUnprotectData(
                &mut input_blob,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output_blob,
            )
        }
    };
    if succeeded == 0 || output_blob.pb_data.is_null() {
        return Err("Windows could not access the encrypted Discord session for this user account.".into());
    }
    let output = unsafe {
        let bytes = std::slice::from_raw_parts(output_blob.pb_data, output_blob.cb_data as usize).to_vec();
        LocalFree(output_blob.pb_data.cast());
        bytes
    };
    Ok(output)
}

#[repr(C)]
struct DataBlob {
    cb_data: u32,
    pb_data: *mut u8,
}

#[link(name = "Crypt32")]
extern "system" {
    fn CryptProtectData(
        data_in: *mut DataBlob,
        description: *const u16,
        optional_entropy: *mut DataBlob,
        reserved: *mut c_void,
        prompt: *mut c_void,
        flags: u32,
        data_out: *mut DataBlob,
    ) -> i32;
    fn CryptUnprotectData(
        data_in: *mut DataBlob,
        description: *mut *mut u16,
        optional_entropy: *mut DataBlob,
        reserved: *mut c_void,
        prompt: *mut c_void,
        flags: u32,
        data_out: *mut DataBlob,
    ) -> i32;
}

#[link(name = "Kernel32")]
extern "system" {
    fn LocalFree(memory: *mut c_void) -> *mut c_void;
}
