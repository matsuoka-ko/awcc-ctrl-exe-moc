#![windows_subsystem = "windows"]

use std::mem::{size_of, zeroed};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::HBRUSH;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Diagnostics::ToolHelp::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::Shell::{Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW};
use windows::Win32::UI::WindowsAndMessaging::*;

const TRAY_UID: u32 = 1;
const WM_TRAYICON: u32 = WM_USER + 1;
const ID_TRAY_EXIT: usize = 1001;
const ID_TRAY_TITLE: usize = 1000;

fn to_wstr(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

fn current_exe_stem() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string()))
        .unwrap_or_else(|| "runner".to_string())
}

fn main() -> windows::core::Result<()> {
    unsafe {
        let h_instance = GetModuleHandleW(None)?;

        let class_name = to_wstr("AwccCtrlRunnerHiddenWindow");
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: HINSTANCE(h_instance.0),
            hIcon: LoadIconW(HINSTANCE(0), IDI_APPLICATION)?,
            hCursor: LoadCursorW(HINSTANCE(0), IDC_ARROW)?,
            hbrBackground: HBRUSH(0),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..zeroed()
        };
        let atom = RegisterClassW(&wc);
        if atom == 0 {
            return Err(windows::core::Error::from_win32());
        }

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class_name.as_ptr()),
            PCWSTR(to_wstr("AwccCtrlRunner").as_ptr()),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            0,
            0,
            HWND(0),
            HMENU(0),
            h_instance,
            None,
        );
        if hwnd.0 == 0 {
            return Err(windows::core::Error::from_win32());
        }

        ShowWindow(hwnd, SW_HIDE);

        // Singleton: terminate sibling color EXEs listed in family file
        kill_sibling_processes();

        // Off-mode: if this exe is listed in off.txt, exit immediately (no tray)
        if is_off_exe() {
            let _ = DestroyWindow(hwnd);
            return Ok(());
        }

        add_tray_icon(hwnd)?;

        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, HWND(0), 0, 0).into() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}

unsafe fn add_tray_icon(hwnd: HWND) -> windows::core::Result<()> {
    let mut nid: NOTIFYICONDATAW = zeroed();
    nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_UID;
    nid.uFlags = NIF_MESSAGE | NIF_ICON;
    nid.uCallbackMessage = WM_TRAYICON;
    nid.hIcon = LoadIconW(HINSTANCE(0), IDI_APPLICATION)?;
    let ok = Shell_NotifyIconW(NIM_ADD, &mut nid);
    if !ok.as_bool() {
        return Err(windows::core::Error::from_win32());
    }
    Ok(())
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_DESTROY => {
            let mut nid: NOTIFYICONDATAW = zeroed();
            nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = TRAY_UID;
            let _ = Shell_NotifyIconW(NIM_DELETE, &mut nid);
            PostQuitMessage(0);
            return LRESULT(0);
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as usize;
            if id == ID_TRAY_EXIT {
                let _ = DestroyWindow(hwnd);
                return LRESULT(0);
            } else if id == ID_TRAY_TITLE {
                // No-op for title click
                return LRESULT(0);
            }
        }
        _ => {}
    }

    if msg == WM_TRAYICON {
        let event = lparam.0 as u32;
        if event == WM_CONTEXTMENU as u32 || event == WM_RBUTTONUP {
            let hmenu = match CreatePopupMenu() {
                Ok(h) => h,
                Err(_) => return LRESULT(0),
            };
            // Title item (disabled/non-clickable) - keep buffers alive until after TrackPopupMenu
            let title_w = to_wstr(&current_exe_stem());
            let exit_w = to_wstr("Exit");
            let _ = AppendMenuW(
                hmenu,
                MF_STRING | MF_DISABLED | MF_GRAYED,
                0,
                PCWSTR(title_w.as_ptr()),
            );
            let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(hmenu, MF_STRING, ID_TRAY_EXIT, PCWSTR(exit_w.as_ptr()));

            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            SetForegroundWindow(hwnd);
            let _ = TrackPopupMenu(
                hmenu,
                TPM_LEFTALIGN | TPM_RIGHTBUTTON,
                pt.x,
                pt.y,
                0,
                hwnd,
                None,
            );
            let _ = DestroyMenu(hmenu);
            return LRESULT(0);
        }
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn kill_sibling_processes() {
    // Read family file from the exe directory: family.txt (one name per line)
    let Ok(exe_path) = std::env::current_exe() else { return; };
    let exe_stem = exe_path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
    let dir = exe_path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let family_path = dir.join("family.txt");
    let Ok(text) = std::fs::read_to_string(&family_path) else { return; };
    let mut targets: Vec<String> = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let mut name = l.to_string();
            if !name.to_ascii_lowercase().ends_with(".exe") {
                name.push_str(".exe");
            }
            name.to_ascii_lowercase()
        })
        .collect();
    if targets.is_empty() { return; }

    // Do not target self
    targets.retain(|n| n.strip_suffix(".exe").unwrap_or(n) != exe_stem);
    if targets.is_empty() { return; }

    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) => h,
            Err(_) => return,
        };
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snapshot, &mut entry).is_ok();
        let self_pid = GetCurrentProcessId();
        while ok {
            let name = wchar_to_lower_string(&entry.szExeFile);
            if targets.iter().any(|t| *t == name) && entry.th32ProcessID != self_pid {
                if let Ok(h) = OpenProcess(PROCESS_TERMINATE, false, entry.th32ProcessID) {
                    let _ = TerminateProcess(h, 0);
                    let _ = CloseHandle(h);
                }
            }
            ok = Process32NextW(snapshot, &mut entry).is_ok();
        }
        let _ = CloseHandle(snapshot);
    }
}

fn wchar_to_lower_string(buf: &[u16]) -> String {
    // Convert up to NUL terminator
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let s = String::from_utf16_lossy(&buf[..len]);
    s.to_ascii_lowercase()
}

fn is_off_exe() -> bool {
    let Ok(path) = std::env::current_exe() else { return false; };
    let dir = match path.parent() { Some(d) => d, None => return false };
    let off_path = dir.join("off.txt");
    let Ok(text) = std::fs::read_to_string(off_path) else { return false; };
    let my_name = path.file_name().and_then(|s| s.to_str()).map(|s| s.to_ascii_lowercase());
    let Some(my_name) = my_name else { return false; };
    for line in text.lines() {
        let name = line.trim();
        if name.is_empty() || name.starts_with('#') { continue; }
        if name.eq_ignore_ascii_case(&my_name) {
            return true;
        }
    }
    false
}
