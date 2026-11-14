#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod assets;
mod mail;
mod models;
mod store;
mod theme;
mod translate;
mod tray;
mod ui;

use gpui::{
    px, size, App, AppContext, Application, Bounds, KeyBinding, TitlebarOptions, WindowBounds,
    WindowOptions,
};

use crate::app::{
    ArchiveMail, DeleteMail, Dismiss, FocusSearch, ForwardMail, NewMail, NextMail, PrevMail,
    OpenSettings, ReplyAllMail, ReplyMail, SendMail, SyncMail, ToggleGrouping, ToggleStar,
    ToggleThemeAction, ToggleUnread, TranslateMail,
};
use crate::ui::input::{
    Backspace, Copy, Cut, Delete, Down, End, Home, Left, Paste, Right, SelectAll, SelectLeft,
    SelectRight, Up,
};

/// Only one copy may own the SQLite database at a time. The handle is
/// intentionally leaked for the lifetime of the process.
#[cfg(windows)]
fn acquire_single_instance() -> bool {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONINFORMATION, MB_OK,
    };

    let name: Vec<u16> = "Local\\mail-single-instance-v1\0".encode_utf16().collect();
    unsafe {
        let handle = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        if handle.is_null() {
            return true;
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let text: Vec<u16> = "mail 已经在运行了，请切换到已打开的窗口。\0"
                .encode_utf16()
                .collect();
            let title: Vec<u16> = "mail\0".encode_utf16().collect();
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                title.as_ptr(),
                MB_OK | MB_ICONINFORMATION,
            );
            return false;
        }
        let _ = handle;
        true
    }
}

#[cfg(not(windows))]
fn acquire_single_instance() -> bool {
    true
}

fn main() {
    if !acquire_single_instance() {
        return;
    }
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .try_init()
        .ok();
    Application::new()
        .with_assets(assets::Assets)
        .run(|cx: &mut App| {
            cx.bind_keys([
                KeyBinding::new("backspace", Backspace, Some("TextInput")),
                KeyBinding::new("delete", Delete, Some("TextInput")),
                KeyBinding::new("left", Left, Some("TextInput")),
                KeyBinding::new("right", Right, Some("TextInput")),
                KeyBinding::new("shift-left", SelectLeft, Some("TextInput")),
                KeyBinding::new("shift-right", SelectRight, Some("TextInput")),
                KeyBinding::new("ctrl-a", SelectAll, Some("TextInput")),
                KeyBinding::new("home", Home, Some("TextInput")),
                KeyBinding::new("end", End, Some("TextInput")),
                KeyBinding::new("ctrl-v", Paste, Some("TextInput")),
                KeyBinding::new("ctrl-c", Copy, Some("TextInput")),
                KeyBinding::new("ctrl-x", Cut, Some("TextInput")),
                KeyBinding::new("backspace", Backspace, Some("TextArea")),
                KeyBinding::new("delete", Delete, Some("TextArea")),
                KeyBinding::new("left", Left, Some("TextArea")),
                KeyBinding::new("right", Right, Some("TextArea")),
                KeyBinding::new("up", Up, Some("TextArea")),
                KeyBinding::new("down", Down, Some("TextArea")),
                KeyBinding::new("shift-left", SelectLeft, Some("TextArea")),
                KeyBinding::new("shift-right", SelectRight, Some("TextArea")),
                KeyBinding::new("ctrl-a", SelectAll, Some("TextArea")),
                KeyBinding::new("home", Home, Some("TextArea")),
                KeyBinding::new("end", End, Some("TextArea")),
                KeyBinding::new("ctrl-v", Paste, Some("TextArea")),
                KeyBinding::new("ctrl-c", Copy, Some("TextArea")),
                KeyBinding::new("ctrl-x", Cut, Some("TextArea")),
                KeyBinding::new("ctrl-n", NewMail, None),
                KeyBinding::new("ctrl-r", ReplyMail, None),
                KeyBinding::new("ctrl-shift-r", ReplyAllMail, None),
                KeyBinding::new("ctrl-shift-f", ForwardMail, None),
                KeyBinding::new("ctrl-shift-e", ArchiveMail, None),
                KeyBinding::new("ctrl-shift-d", DeleteMail, None),
                KeyBinding::new("ctrl-shift-s", ToggleStar, None),
                KeyBinding::new("ctrl-shift-u", ToggleUnread, None),
                KeyBinding::new("ctrl-j", NextMail, None),
                KeyBinding::new("ctrl-k", PrevMail, None),
                KeyBinding::new("ctrl-f", FocusSearch, None),
                KeyBinding::new("f5", SyncMail, None),
                KeyBinding::new("ctrl-enter", SendMail, None),
                KeyBinding::new("ctrl-shift-t", ToggleThemeAction, None),
                KeyBinding::new("ctrl-g", ToggleGrouping, None),
                KeyBinding::new("ctrl-t", TranslateMail, None),
                KeyBinding::new("ctrl-,", OpenSettings, None),
                KeyBinding::new("escape", Dismiss, None),
            ]);

            let bounds = Bounds::centered(None, size(px(1360.), px(860.)), cx);
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Maximized(bounds)),
                        titlebar: Some(TitlebarOptions {
                            title: Some("mail".into()),
                            appears_transparent: true,
                            traffic_light_position: None,
                        }),
                        window_min_size: Some(size(px(980.), px(640.))),
                        ..Default::default()
                    },
                    |window, cx| cx.new(|cx| app::Root::new(window, cx)),
                )
                .expect("无法创建窗口");

            window
                .update(cx, |root, window, cx| {
                    window.focus(&root.focus_handle);
                    cx.notify();
                })
                .ok();

            cx.on_window_closed(|cx| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            cx.activate(true);
        });
}

#[cfg(test)]
mod tests {
    #[test]
    fn logo_decodes() {
        let bytes = include_bytes!("../assets/logo.png");
        let image = image::load_from_memory(bytes).expect("logo should decode");
        assert_eq!(image.width(), 256);
        assert_eq!(image.height(), 256);
    }
}






