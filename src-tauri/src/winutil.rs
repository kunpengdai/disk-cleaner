//! Windows 系统接口：磁盘列表、管理员权限、回收站。

use serde::Serialize;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::{
    FileOperation, IFileOperation, IShellItem, SHCreateItemFromParsingName, SHEmptyRecycleBinW, ShellExecuteW,
    FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_WANTNUKEWARNING,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(std::iter::once(0)).collect()
}

#[derive(Serialize)]
pub struct DriveInfo {
    pub root: String,
    pub label: String,
    pub fs: String,
    pub kind: &'static str,
    pub total: u64,
    pub free: u64,
}

pub fn list_drives() -> Vec<DriveInfo> {
    let mut out = Vec::new();
    let mask = unsafe { GetLogicalDrives() };
    for i in 0..26u32 {
        if mask & (1 << i) == 0 {
            continue;
        }
        let root = format!("{}:\\", (b'A' + i as u8) as char);
        let w = wide(&root);
        let kind = match unsafe { GetDriveTypeW(PCWSTR(w.as_ptr())) } {
            2 => "可移动磁盘",
            3 => "本地磁盘",
            4 => "网络驱动器",
            _ => continue, // 光驱、虚拟盘等
        };
        let (mut free, mut total) = (0u64, 0u64);
        if unsafe { GetDiskFreeSpaceExW(PCWSTR(w.as_ptr()), Some(&mut free), Some(&mut total), None) }.is_err() {
            continue;
        }
        let mut label = [0u16; 261];
        let mut fs = [0u16; 261];
        let _ = unsafe { GetVolumeInformationW(PCWSTR(w.as_ptr()), Some(&mut label), None, None, None, Some(&mut fs)) };
        let s = |b: &[u16]| String::from_utf16_lossy(&b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]);
        out.push(DriveInfo { root, label: s(&label), fs: s(&fs), kind, total, free });
    }
    out
}

pub fn is_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elev = TOKEN_ELEVATION::default();
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elev as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && elev.TokenIsElevated != 0
    }
}

/// 以管理员身份重新启动本程序。用户在 UAC 里点“否”时返回错误。
pub fn relaunch_as_admin() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let w = wide(&exe);
    let r = unsafe { ShellExecuteW(None, w!("runas"), PCWSTR(w.as_ptr()), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
    if r.0 as isize > 32 {
        Ok(())
    } else {
        Err("没有获得管理员权限（可能在系统提示中点了“否”）".into())
    }
}

pub fn empty_recycle_bin(drive_root: &str) -> Result<(), String> {
    const SHERB_NOCONFIRMATION: u32 = 1;
    const SHERB_NOPROGRESSUI: u32 = 2;
    const SHERB_NOSOUND: u32 = 4;
    let w = wide(drive_root);
    unsafe {
        SHEmptyRecycleBinW(None, PCWSTR(w.as_ptr()), SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND)
            .map_err(|e| format!("清空回收站失败：{e}"))
    }
}

/// 移到回收站。
/// 和常见做法不同，这里保留了 Windows 的“无法放入回收站，将永久删除”警告，
/// 由用户自己决定是否永久删除，避免文件被静默永久删除。
/// 返回 true 表示有操作被取消或失败（调用方需要逐个检查结果）。
pub fn recycle(paths: &[PathBuf], owner: Option<isize>) -> Result<bool, String> {
    if paths.is_empty() {
        return Ok(false);
    }
    unsafe {
        let init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let result = (|| -> windows::core::Result<bool> {
            let op: IFileOperation = CoCreateInstance(&FileOperation, None, CLSCTX_ALL)?;
            op.SetOperationFlags(FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_WANTNUKEWARNING | FOF_NOERRORUI)?;
            if let Some(h) = owner {
                let _ = op.SetOwnerWindow(HWND(h as *mut _));
            }
            for p in paths {
                let w = wide(p);
                let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(w.as_ptr()), None)?;
                op.DeleteItem(&item, None)?;
            }
            let perform = op.PerformOperations();
            let aborted = op.GetAnyOperationsAborted().map(|b| b.as_bool()).unwrap_or(true);
            Ok(perform.is_err() || aborted)
        })();
        if init.is_ok() {
            CoUninitialize();
        }
        result.map_err(|e| format!("移到回收站失败：{e}"))
    }
}

/// 系统记录的“文档 / 下载”等文件夹的真实位置（用户可能把它们移到了其他盘）
pub fn known_folder(name: &str) -> Option<String> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::*;
    let id = match name {
        "Documents" => FOLDERID_Documents,
        "Downloads" => FOLDERID_Downloads,
        "Pictures" => FOLDERID_Pictures,
        "Videos" => FOLDERID_Videos,
        "Music" => FOLDERID_Music,
        "Desktop" => FOLDERID_Desktop,
        _ => return None,
    };
    unsafe {
        let p = SHGetKnownFolderPath(&id, KF_FLAG_DEFAULT, None).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as _));
        s
    }
}

pub fn open_in_explorer(path: &Path) {
    let _ = std::process::Command::new("explorer").arg(format!("/select,{}", path.display())).spawn();
}
