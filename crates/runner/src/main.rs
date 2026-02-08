#![windows_subsystem = "windows"]

use std::mem::{size_of, zeroed};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::HBRUSH;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Diagnostics::ToolHelp::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::Shell::{ShellExecuteW, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW};
use windows::Win32::UI::WindowsAndMessaging::*;

const TRAY_UID: u32 = 1;
const WM_TRAYICON: u32 = WM_USER + 1;
const ID_TRAY_EXIT: usize = 1001;
const ID_TRAY_TITLE: usize = 1000;
static mut MAIN_HWND: HWND = HWND(0);
static mut FOREGROUND_HWND: HWND = HWND(0);
static mut FORCE_FOREGROUND: bool = false;

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
        MAIN_HWND = hwnd;

        ShowWindow(hwnd, SW_HIDE);

        // Singleton: terminate sibling color EXEs listed in family file
        kill_sibling_processes();

        // Off-mode: if this exe is listed in off.txt, exit immediately (no tray)
        if is_off_exe() {
            let _ = DestroyWindow(hwnd);
            return Ok(());
        }

        // Ensure AWCC is running (optional, based on dist/awcc_*.txt)
        ensure_awcc_running();

        // Experimental: keep a tiny topmost window in the foreground if enabled
        FORCE_FOREGROUND = should_force_foreground();
        if FORCE_FOREGROUND {
            if let Some(fg_hwnd) = create_foreground_window(HINSTANCE(h_instance.0)) {
                FOREGROUND_HWND = fg_hwnd;
                let _ = SetWindowPos(
                    fg_hwnd,
                    HWND_TOPMOST,
                    0,
                    0,
                    1,
                    1,
                    SWP_NOACTIVATE,
                );
                ShowWindow(fg_hwnd, SW_SHOWNA);
            }
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

fn should_force_foreground() -> bool {
    let Ok(path) = std::env::current_exe() else { return false; };
    let dir = match path.parent() { Some(d) => d, None => return false };
    dir.join("keep_foreground.txt").exists()
}

fn ensure_awcc_running() {
    let Ok(path) = std::env::current_exe() else { return; };
    let dir = match path.parent() { Some(d) => d, None => return };
    let awcc_path = dir.join("awcc_path.txt");
    let Ok(raw) = std::fs::read_to_string(&awcc_path) else { return; };
    let awcc_exe = raw.lines().next().unwrap_or("").trim().to_string();
    if awcc_exe.is_empty() {
        return;
    }
    let awcc_name = std::path::Path::new(&awcc_exe)
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_else(|| "awcc.exe".to_string());

    if is_process_running(&awcc_name) {
        return;
    }

    let mut args: Vec<String> = Vec::new();
    let awcc_args = dir.join("awcc_args.txt");
    if let Ok(s) = std::fs::read_to_string(&awcc_args) {
        for l in s.lines() {
            let a = l.trim();
            if !a.is_empty() && !a.starts_with('#') {
                args.push(a.to_string());
            }
        }
    }

    let start_min = {
        let awcc_min = dir.join("awcc_start_minimized.txt");
        if let Ok(s) = std::fs::read_to_string(&awcc_min) {
            s.lines()
                .next()
                .map(|v| v.trim().eq_ignore_ascii_case("true"))
                .unwrap_or(true)
        } else {
            true
        }
    };

    let _ = spawn_background(&awcc_exe, &args, start_min);
}

fn is_process_running(exe_name_lower: &str) -> bool {
    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) => h,
            Err(_) => return false,
        };
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snapshot, &mut entry).is_ok();
        while ok {
            let name = wchar_to_lower_string(&entry.szExeFile);
            if name == exe_name_lower {
                let _ = CloseHandle(snapshot);
                return true;
            }
            ok = Process32NextW(snapshot, &mut entry).is_ok();
        }
        let _ = CloseHandle(snapshot);
    }
    false
}

fn spawn_background(exe_path: &str, args: &[String], start_minimized: bool) -> bool {
    unsafe {
        let op = to_wstr("open");
        let file = to_wstr(exe_path);
        let params = build_params(args);
        let show = if start_minimized { SW_SHOWMINNOACTIVE } else { SW_SHOWNORMAL };
        let h = if params.is_empty() {
            ShellExecuteW(HWND(0), PCWSTR(op.as_ptr()), PCWSTR(file.as_ptr()), PCWSTR::null(), PCWSTR::null(), show)
        } else {
            ShellExecuteW(
                HWND(0),
                PCWSTR(op.as_ptr()),
                PCWSTR(file.as_ptr()),
                PCWSTR(params.as_ptr()),
                PCWSTR::null(),
                show,
            )
        };
        h.0 > 32
    }
}

fn build_params(args: &[String]) -> Vec<u16> {
    if args.is_empty() {
        return Vec::new();
    }
    let mut s = String::new();
    for a in args {
        if !s.is_empty() {
            s.push(' ');
        }
        s.push_str(&quote_arg(a));
    }
    to_wstr(&s)
}

fn quote_arg(arg: &str) -> String {
    if arg.contains(' ') || arg.contains('\t') || arg.contains('"') {
        let escaped = arg.replace('"', "\\\"");
        format!("\"{}\"", escaped)
    } else {
        arg.to_string()
    }
}

unsafe fn create_foreground_window(h_instance: HINSTANCE) -> Option<HWND> {
    let class_name = to_wstr("AwccCtrlRunnerForegroundWindow");
    let wc = WNDCLASSW {
        style: WNDCLASS_STYLES(0),
        lpfnWndProc: Some(wndproc),
        hInstance: HINSTANCE(h_instance.0),
        hIcon: LoadIconW(HINSTANCE(0), IDI_APPLICATION).ok()?,
        hCursor: LoadCursorW(HINSTANCE(0), IDC_ARROW).ok()?,
        hbrBackground: HBRUSH(0),
        lpszClassName: PCWSTR(class_name.as_ptr()),
        ..zeroed()
    };
    let atom = RegisterClassW(&wc);
    if atom == 0 {
        return None;
    }

    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE(WS_EX_TOOLWINDOW.0 | WS_EX_TOPMOST.0),
        PCWSTR(class_name.as_ptr()),
        PCWSTR(to_wstr("AwccCtrlRunnerForeground").as_ptr()),
        WS_POPUP,
        0,
        0,
        1,
        1,
        HWND(0),
        HMENU(0),
        h_instance,
        None,
    );
    if hwnd.0 == 0 {
        return None;
    }
    Some(hwnd)
}

unsafe fn add_tray_icon(hwnd: HWND) -> windows::core::Result<()> {
    let mut nid: NOTIFYICONDATAW = zeroed();
    nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_UID;
    // Show tooltip with current exe name on mouseover
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    nid.uCallbackMessage = WM_TRAYICON;
    nid.hIcon = LoadIconW(HINSTANCE(0), IDI_APPLICATION)?;
    // Tooltip text: project name + exe name (stem)
    let tip_text = format!("awcc-ctrl-exe-moc - {}", current_exe_stem());
    let tip_w = to_wstr(&tip_text);
    let max = nid.szTip.len().saturating_sub(1);
    for (i, ch) in tip_w.iter().take(max).enumerate() {
        nid.szTip[i] = *ch;
    }
    let ok = Shell_NotifyIconW(NIM_ADD, &mut nid);
    if !ok.as_bool() {
        return Err(windows::core::Error::from_win32());
    }
    Ok(())
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_DESTROY => {
            if hwnd == MAIN_HWND {
                let mut nid: NOTIFYICONDATAW = zeroed();
                nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
                nid.hWnd = hwnd;
                nid.uID = TRAY_UID;
                let _ = Shell_NotifyIconW(NIM_DELETE, &mut nid);
                if FOREGROUND_HWND.0 != 0 {
                    let _ = DestroyWindow(FOREGROUND_HWND);
                }
                PostQuitMessage(0);
            }
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
            let title_text = format!("awcc-ctrl-exe-moc - {}", current_exe_stem());
            let title_w = to_wstr(&title_text);
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
