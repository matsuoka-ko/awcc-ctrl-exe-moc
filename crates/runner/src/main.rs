#![windows_subsystem = "windows"]

use std::mem::{size_of, zeroed};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::HBRUSH;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
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
