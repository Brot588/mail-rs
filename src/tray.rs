#![allow(unsafe_op_in_unsafe_fn)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

pub enum TrayAction {
    Open,
    Sync,
    Quit,
    Unavailable,
}

pub struct Tray {
    receiver: Arc<Mutex<Receiver<TrayAction>>>,
    available: Arc<AtomicBool>,
}

impl Tray {
    pub fn is_available(&self) -> bool {
        self.available.load(Ordering::SeqCst)
    }

    pub fn actions(&self) -> Arc<Mutex<Receiver<TrayAction>>> {
        self.receiver.clone()
    }
}

#[cfg(windows)]
mod platform {
    use std::mem::{size_of, zeroed};
    use std::ptr::null_mut;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{self, Sender};
    use std::sync::{Arc, OnceLock};
    use std::thread;

    use gpui::Window;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Foundation::{GetLastError, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows_sys::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
        use windows_sys::Win32::UI::Shell::{
        Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIN_BALLOONUSERCLICK, NIM_ADD,
        NIM_DELETE, NIM_MODIFY, NIM_SETVERSION, NOTIFYICONDATAW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
        DestroyWindow, DispatchMessageW, GetCursorPos, GetForegroundWindow, GetMessageW,
        HICON, IsIconic, LoadIconW, PostQuitMessage, RegisterClassW, RegisterWindowMessageW,
        SetForegroundWindow, ShowWindow, TrackPopupMenu, TranslateMessage, MSG, MF_SEPARATOR,
        MF_STRING, SW_HIDE, SW_RESTORE, SW_SHOW,
        TPM_BOTTOMALIGN, TPM_RIGHTBUTTON, WM_APP, WM_COMMAND, WM_DESTROY, WM_LBUTTONDBLCLK,
        WM_LBUTTONUP, WM_RBUTTONUP, WNDCLASSW,
    };

    use super::TrayAction;

    const WM_TRAY: u32 = WM_APP + 1;
    const CMD_OPEN: usize = 1;
    const CMD_SYNC: usize = 2;
    const CMD_QUIT: usize = 3;

    struct TrayState {
        hwnd: usize,
        icon: usize,
    }

    static ACTIONS: OnceLock<Sender<TrayAction>> = OnceLock::new();
    static MAIN_HWND: OnceLock<usize> = OnceLock::new();
    static TRAY_STATE: OnceLock<TrayState> = OnceLock::new();
    static TRAY_AVAILABLE: OnceLock<Arc<AtomicBool>> = OnceLock::new();
    static TASKBAR_CREATED: OnceLock<u32> = OnceLock::new();

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn copy_wide(destination: &mut [u16], text: &str) {
        for slot in destination.iter_mut() {
            *slot = 0;
        }
        let encoded: Vec<u16> = text.encode_utf16().collect();
        let count = encoded.len().min(destination.len().saturating_sub(1));
        destination[..count].copy_from_slice(&encoded[..count]);
    }

    fn send(action: TrayAction) {
        if let Some(sender) = ACTIONS.get() {
            let _ = sender.send(action);
        }
    }

    fn load_tray_icon(instance: HINSTANCE) -> HICON {
        unsafe {
            let icon = LoadIconW(instance, 1 as *const u16);
            if !icon.is_null() {
                return icon;
            }
            LoadIconW(null_mut(), 32512 as *const u16)
        }
    }

    unsafe fn show_menu(hwnd: HWND) {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        let open = wide("打开 mail");
        let sync = wide("立即同步");
        let quit = wide("退出 mail");
        AppendMenuW(menu, MF_STRING, CMD_OPEN, open.as_ptr());
        AppendMenuW(menu, MF_STRING, CMD_SYNC, sync.as_ptr());
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(menu, MF_STRING, CMD_QUIT, quit.as_ptr());
        let mut point: POINT = zeroed();
        GetCursorPos(&mut point);
        SetForegroundWindow(hwnd);
        TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            point.x,
            point.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        DestroyMenu(menu);
    }

    unsafe fn re_add_icon(hwnd: HWND) {
        let Some(state) = TRAY_STATE.get() else {
            return;
        };
        let mut data: NOTIFYICONDATAW = zeroed();
        data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = hwnd;
        data.uID = 1;
        data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        data.uCallbackMessage = WM_TRAY;
        data.hIcon = state.icon as HICON;
        copy_wide(&mut data.szTip, "mail · 邮件客户端");
        let added = Shell_NotifyIconW(NIM_ADD, &data) != 0;
        if added {
            data.uFlags = 0;
            data.Anonymous.uVersion = 4;
            let _ = Shell_NotifyIconW(NIM_SETVERSION, &data);
        }
        if let Some(available) = TRAY_AVAILABLE.get() {
            available.store(added, Ordering::SeqCst);
        }
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if TASKBAR_CREATED.get().is_some_and(|created| *created == message) {
            re_add_icon(hwnd);
            return 0;
        }
        match message {
            WM_TRAY => {
                let event = (lparam as u32) & 0xFFFF;
                match event {
                    WM_LBUTTONUP | WM_LBUTTONDBLCLK | NIN_BALLOONUSERCLICK => send(TrayAction::Open),
                    WM_RBUTTONUP => show_menu(hwnd),
                    _ => {}
                }
                0
            }
            WM_COMMAND => {
                let command = wparam & 0xFFFF;
                match command {
                    CMD_OPEN => send(TrayAction::Open),
                    CMD_SYNC => send(TrayAction::Sync),
                    CMD_QUIT => send(TrayAction::Quit),
                    _ => {}
                }
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }

    unsafe fn run_tray(available: Arc<AtomicBool>) {
        let _ = CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
        let taskbar_created = RegisterWindowMessageW(wide("TaskbarCreated").as_ptr());
        if taskbar_created != 0 {
            let _ = TASKBAR_CREATED.set(taskbar_created);
        }
        let instance = GetModuleHandleW(std::ptr::null());
        let class_name = wide("MailTrayHiddenWindow");
        let title = wide("mail");
        let window_class = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: null_mut(),
            hCursor: null_mut(),
            hbrBackground: null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };
        RegisterClassW(&window_class);
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            instance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return;
        }
        let icon = load_tray_icon(instance);
        let mut data: NOTIFYICONDATAW = zeroed();
        data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = hwnd;
        data.uID = 1;
        data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        data.uCallbackMessage = WM_TRAY;
        data.hIcon = icon;
        copy_wide(&mut data.szTip, "mail · 邮件客户端");
        let added = Shell_NotifyIconW(NIM_ADD, &data);
        if added == 0 {
            // A Low-Integrity process (for example one launched directly from a
            // sandboxed workspace) is not allowed to own a tray icon. Copying
            // mail.exe to a normal user folder fixes it.
            eprintln!("mail tray unavailable: error {}", GetLastError());
            send(TrayAction::Unavailable);
        }
        if added != 0 {
            let _ = TRAY_STATE.set(TrayState {
                hwnd: hwnd as usize,
                icon: icon as usize,
            });
            available.store(true, Ordering::SeqCst);
            data.uFlags = 0;
            data.Anonymous.uVersion = 4;
            let _ = Shell_NotifyIconW(NIM_SETVERSION, &data);
        }

        let mut message: MSG = zeroed();
        while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        let _ = Shell_NotifyIconW(NIM_DELETE, &data);
        DestroyWindow(hwnd);
        CoUninitialize();
    }

    pub fn install(window: &Window) -> Option<super::Tray> {
        let main_hwnd = HasWindowHandle::window_handle(window)
            .ok()
            .and_then(|handle| match handle.as_raw() {
                RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as usize),
                _ => None,
            })
            .unwrap_or(0);
        let (sender, receiver) = mpsc::channel();
        let available = Arc::new(AtomicBool::new(false));
        let _ = TRAY_AVAILABLE.set(available.clone());
        let _ = ACTIONS.set(sender);
        let _ = MAIN_HWND.set(main_hwnd);
        let thread_available = available.clone();
        thread::Builder::new()
            .name("mail-tray".to_string())
            .spawn(move || unsafe { run_tray(thread_available) })
            .ok()?;
        Some(super::Tray {
            receiver: Arc::new(std::sync::Mutex::new(receiver)),
            available,
        })
    }

    pub fn show_main_window() {
        if let Some(&hwnd) = MAIN_HWND.get()
            && hwnd != 0
        {
            unsafe {
                let hwnd = hwnd as HWND;
                if IsIconic(hwnd) != 0 {
                    ShowWindow(hwnd, SW_RESTORE);
                } else {
                    ShowWindow(hwnd, SW_SHOW);
                }
                SetForegroundWindow(hwnd);
            }
        }
    }

    pub fn hide_main_window() {
        std::thread::sleep(std::time::Duration::from_millis(80));
        if let Some(&hwnd) = MAIN_HWND.get()
            && hwnd != 0
        {
            unsafe {
                ShowWindow(hwnd as HWND, SW_HIDE);
            }
        }
    }

    pub fn notify(title: &str, body: &str) {
        let Some(state) = TRAY_STATE.get() else {
            return;
        };
        unsafe {
            if let Some(&main) = MAIN_HWND.get()
                && main != 0
                && GetForegroundWindow() as usize == main
            {
                return;
            }
            let mut data: NOTIFYICONDATAW = zeroed();
            data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
            data.hWnd = state.hwnd as HWND;
            data.uID = 1;
            data.uFlags = NIF_INFO;
            data.hIcon = state.icon as HICON;
            copy_wide(&mut data.szInfoTitle, title);
            copy_wide(&mut data.szInfo, body);
            data.dwInfoFlags = NIIF_INFO;
            data.Anonymous.uTimeout = 6000;
            let _ = Shell_NotifyIconW(NIM_MODIFY, &data);
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use super::Tray;
    use gpui::Window;

    pub fn install(_window: &Window) -> Option<Tray> {
        None
    }

    pub fn show_main_window() {}

    pub fn hide_main_window() {}

    pub fn notify(_title: &str, _body: &str) {}
}

pub use platform::{hide_main_window, install, notify, show_main_window};

