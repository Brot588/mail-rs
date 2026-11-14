use std::collections::{HashMap, HashSet};
use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    actions, px, App, ClipboardItem, Context, Entity, FocusHandle, Focusable, Hsla,
    PathPromptOptions, Rgba, ScrollHandle, SharedString, Window,
};

use crate::mail;
use crate::models::{
    build_threads, now_ts, Account, Attachment, Folder, FolderKind, Message, Outgoing, Thread,
};
use crate::store::{self, Store};
use crate::theme::Theme;
use crate::translate;
use crate::ui::input::{TextArea, TextColors, TextInput};

actions!(
    feather,
    [
        NewMail,
        ReplyMail,
        ReplyAllMail,
        ForwardMail,
        ArchiveMail,
        DeleteMail,
        ToggleStar,
        ToggleUnread,
        NextMail,
        PrevMail,
        FocusSearch,
        SyncMail,
        Dismiss,
        SendMail,
        ToggleThemeAction,
        OpenSettings,
        ToggleGrouping,
        TranslateMail,
    ]
);

pub const SIDEBAR_WIDTH: f32 = 234.;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ComposeMode {
    New,
    Reply,
    ReplyAll,
    Forward,
    Draft,
}

impl ComposeMode {
    pub fn title(self) -> &'static str {
        match self {
            ComposeMode::New => "新邮件",
            ComposeMode::Reply => "回复",
            ComposeMode::ReplyAll => "回复全部",
            ComposeMode::Forward => "转发",
            ComposeMode::Draft => "编辑草稿",
        }
    }
}

pub struct Compose {
    pub to: Entity<TextInput>,
    pub cc: Entity<TextInput>,
    pub bcc: Entity<TextInput>,
    pub subject: Entity<TextInput>,
    pub body: Entity<TextArea>,
    pub show_cc: bool,
    pub show_bcc: bool,
    pub in_reply_to: Option<String>,
    pub sending: bool,
    pub mode: ComposeMode,
    /// Local draft row being edited; `None` until the first autosave.
    pub draft_id: Option<i64>,
    /// Fingerprint of the last persisted content, so autosave only writes changes.
    pub last_saved: String,
    pub attachments: Vec<Attachment>,
    /// Stable Message-ID assigned to the local/server draft.
    pub draft_message_id: String,
}

pub struct SettingsPanel {
    pub display_name: Entity<TextInput>,
    pub email: Entity<TextInput>,
    pub password: Entity<TextInput>,
    pub imap_host: Entity<TextInput>,
    pub imap_port: Entity<TextInput>,
    pub smtp_host: Entity<TextInput>,
    pub smtp_port: Entity<TextInput>,
    pub imap_starttls: bool,
    pub smtp_starttls: bool,
    pub editing_id: i64,
    /// Server fields were not customised yet, so changing the address may
    /// auto-fill official endpoints for well-known providers.
    pub auto_servers: bool,
    pub error: Option<String>,
}

/// Official authorization-code / app-password help for common providers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthHelpProvider {
    Qq,
    NetEase,
    Gmail,
    Outlook,
    ICloud,
    Generic,
}

const QQ_STEPS: &[&str] = &[
    "登录 QQ 邮箱网页版，点击顶部「设置」→「账号与安全」。",
    "找到「POP3/IMAP/SMTP/Exchange/CardDAV 服务」，开启 IMAP/SMTP。",
    "按页面提示完成手机验证，会生成一个 16 位授权码。",
    "复制授权码，回到 mail 点击「粘贴授权码」。",
];
const NETEASE_STEPS: &[&str] = &[
    "登录网易邮箱网页版（https://mail.163.com/），点击顶部「设置」→「POP3/SMTP/IMAP」。",
    "开启 IMAP/SMTP 服务，点击「新增授权密码」。",
    "按提示完成手机验证，设置自定义授权密码（建议字母 + 数字）。",
    "复制生成的授权码，回到 mail 点击「粘贴授权码」。",
];
const GMAIL_STEPS: &[&str] = &[
    "先在 Google 账户中开启两步验证（2-Step Verification）。",
    "打开「应用专用密码」页面，创建一个新的应用专用密码。",
    "复制生成的 16 位密码，回到 mail 点击「粘贴授权码」。",
];
const OUTLOOK_STEPS: &[&str] = &[
    "打开 Microsoft 账户的「安全」→「高级安全选项」。",
    "确认已开启双重验证，找到「应用密码」并创建一个。",
    "复制生成的密码，回到 mail 点击「粘贴授权码」。",
];
const ICLOUD_STEPS: &[&str] = &[
    "登录 Apple 账户，进入「登录与安全」。",
    "在「App 专用密码」中生成一个新密码。",
    "复制生成的密码，回到 mail 点击「粘贴授权码」。",
];
const GENERIC_STEPS: &[&str] = &[
    "登录邮箱的网页版，进入「设置」→「账户 / 安全」。",
    "开启 IMAP / SMTP 服务，找到「授权码 / 应用专用密码」。",
    "按服务商提示验证身份并生成密码。",
    "复制后回到 mail 点击「粘贴授权码」。",
];

impl AuthHelpProvider {
    pub const ALL: [AuthHelpProvider; 6] = [
        AuthHelpProvider::Qq,
        AuthHelpProvider::NetEase,
        AuthHelpProvider::Gmail,
        AuthHelpProvider::Outlook,
        AuthHelpProvider::ICloud,
        AuthHelpProvider::Generic,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AuthHelpProvider::Qq => "QQ 邮箱",
            AuthHelpProvider::NetEase => "网易邮箱",
            AuthHelpProvider::Gmail => "Gmail",
            AuthHelpProvider::Outlook => "Outlook",
            AuthHelpProvider::ICloud => "iCloud",
            AuthHelpProvider::Generic => "其他邮箱",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            AuthHelpProvider::Qq => "QQ 邮箱 · 16 位授权码",
            AuthHelpProvider::NetEase => "网易邮箱 · 客户端授权密码",
            AuthHelpProvider::Gmail => "Gmail · 应用专用密码",
            AuthHelpProvider::Outlook => "Outlook / Microsoft 账户",
            AuthHelpProvider::ICloud => "iCloud Mail · App 专用密码",
            AuthHelpProvider::Generic => "其他邮箱 · 授权码",
        }
    }

    pub fn url(self) -> Option<&'static str> {
        match self {
            AuthHelpProvider::Qq => Some("https://help.mail.qq.com/detail/0/1087"),
            AuthHelpProvider::NetEase => Some(
                "https://help.mail.163.com/faqDetail.do?code=d7a5dc8471cd0c0e8b4b8f4f8e49998b374173cfe9171305fa1ce630d7f67ac286624f309a1a7089",
            ),
            AuthHelpProvider::Gmail => Some("https://myaccount.google.com/apppasswords"),
            AuthHelpProvider::Outlook => Some("https://account.microsoft.com/security"),
            AuthHelpProvider::ICloud => Some("https://account.apple.com/account/manage"),
            AuthHelpProvider::Generic => None,
        }
    }

    /// Recommended `(imap_host, imap_port, imap_starttls, smtp_host,
    /// smtp_port, smtp_starttls)` for this provider.
    pub fn servers(self) -> Option<(&'static str, u16, bool, &'static str, u16, bool)> {
        match self {
            AuthHelpProvider::Qq => Some(("imap.qq.com", 993, false, "smtp.qq.com", 465, false)),
            AuthHelpProvider::NetEase => {
                Some(("imap.163.com", 993, false, "smtp.163.com", 465, false))
            }
            AuthHelpProvider::Gmail => {
                Some(("imap.gmail.com", 993, false, "smtp.gmail.com", 465, false))
            }
            AuthHelpProvider::Outlook => Some((
                "outlook.office365.com",
                993,
                false,
                "smtp.office365.com",
                587,
                true,
            )),
            AuthHelpProvider::ICloud => Some((
                "imap.mail.me.com",
                993,
                false,
                "smtp.mail.me.com",
                587,
                true,
            )),
            AuthHelpProvider::Generic => None,
        }
    }

    pub fn servers_for(
        self,
        email: &str,
    ) -> Option<(&'static str, u16, bool, &'static str, u16, bool)> {
        let domain = email
            .rsplit('@')
            .next()
            .unwrap_or("")
            .trim()
            .to_lowercase();
        match self {
            AuthHelpProvider::NetEase => {
                if domain.ends_with("126.com") {
                    Some(("imap.126.com", 993, false, "smtp.126.com", 465, false))
                } else if domain.ends_with("yeah.net") {
                    Some(("imap.yeah.net", 993, false, "smtp.yeah.net", 465, false))
                } else if domain.ends_with("188.com") {
                    Some(("imap.188.com", 993, false, "smtp.188.com", 465, false))
                } else {
                    self.servers()
                }
            }
            AuthHelpProvider::Qq if domain.ends_with("exmail.qq.com") => {
                Some(("imap.exmail.qq.com", 993, false, "smtp.exmail.qq.com", 465, false))
            }
            _ => self.servers(),
        }
    }

    pub fn steps(self) -> &'static [&'static str] {
        match self {
            AuthHelpProvider::Qq => QQ_STEPS,
            AuthHelpProvider::NetEase => NETEASE_STEPS,
            AuthHelpProvider::Gmail => GMAIL_STEPS,
            AuthHelpProvider::Outlook => OUTLOOK_STEPS,
            AuthHelpProvider::ICloud => ICLOUD_STEPS,
            AuthHelpProvider::Generic => GENERIC_STEPS,
        }
    }

    pub fn note(self) -> &'static str {
        match self {
            AuthHelpProvider::Qq => "授权码不是 QQ 密码；修改 QQ 密码后授权码会失效。",
            AuthHelpProvider::NetEase => "网易把它称为「客户端授权密码」，一个账号可设置一个。",
            AuthHelpProvider::Gmail => "没有开启两步验证时，Google 不会显示应用专用密码入口。",
            AuthHelpProvider::Outlook => {
                "工作 / 学校账户可能被管理员关闭应用密码，需要改用 OAuth 登录。"
            }
            AuthHelpProvider::ICloud => "Apple 账户需要先开启双重认证。",
            AuthHelpProvider::Generic => "不同服务商的名称不同，也可能叫「客户端密码」。",
        }
    }

    pub fn detect(email: &str, imap_host: &str) -> Self {
        fn has_domain(domain: &str, suffix: &str) -> bool {
            domain == suffix || domain.ends_with(&format!(".{suffix}"))
        }

        let domain = email
            .rsplit('@')
            .next()
            .unwrap_or("")
            .trim()
            .to_lowercase();
        let host = imap_host.to_lowercase();
        let domain_is = |suffix: &str| has_domain(&domain, suffix);

        if domain_is("qq.com")
            || domain_is("foxmail.com")
            || domain_is("exmail.qq.com")
            || host.contains("imap.qq.com")
        {
            AuthHelpProvider::Qq
        } else if domain_is("163.com")
            || domain_is("126.com")
            || domain_is("yeah.net")
            || domain_is("188.com")
            || host.contains("imap.163.com")
            || host.contains("imap.126.com")
        {
            AuthHelpProvider::NetEase
        } else if domain_is("gmail.com")
            || domain_is("googlemail.com")
            || host.contains("imap.gmail.com")
        {
            AuthHelpProvider::Gmail
        } else if domain_is("outlook.com")
            || domain_is("hotmail.com")
            || domain_is("live.com")
            || domain_is("msn.com")
            || host.contains("outlook.office365.com")
            || host.contains("outlook.com")
        {
            AuthHelpProvider::Outlook
        } else if domain_is("icloud.com")
            || domain_is("me.com")
            || domain_is("mac.com")
            || host.contains("imap.mail.me.com")
        {
            AuthHelpProvider::ICloud
        } else {
            AuthHelpProvider::Generic
        }
    }

}

#[derive(Clone)]
pub enum ConfirmAction {
    PermanentDelete { ids: Vec<i64> },
    RemoveAccount { id: i64, email: String },
}

#[derive(Clone)]
pub enum TranslationEntry {
    Loading,
    Ready(String),
    Failed(String),
}

#[derive(Default)]
pub struct TranslationState {
    pub visible: bool,
    pub entries: HashMap<i64, TranslationEntry>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

#[derive(Clone)]
pub enum ToastAction {
    UndoMove { items: Vec<(i64, String)> },
}

pub struct Toast {
    pub message: String,
    pub kind: ToastKind,
    pub id: u64,
    pub action: Option<ToastAction>,
}

pub struct Root {
    pub theme: Theme,
    pub store: Store,
    pub accounts: Vec<Account>,
    pub active_account: i64,
    pub folders: Vec<Folder>,
    pub selected_folder: String,
    /// Messages for the selected folder, after the search filter.
    pub messages: Vec<Message>,
    /// Whether another page of the current folder exists.
    pub has_more: bool,
    pub older_loading: bool,
    pub contact_suggestions: Vec<(String, String)>,
    /// All messages of the selected folder.
    pub folder_messages: Vec<Message>,
    pub selected: Option<usize>,
    /// Contact conversations, used when `grouped` is enabled.
    pub threads: Vec<Thread>,
    pub selected_thread: Option<usize>,
    pub grouped: bool,
    pub query: String,
    /// Search all cached folders instead of only the selected folder.
    pub search_all: bool,
    pub search: Entity<TextInput>,
    pub compose: Option<Compose>,
    pub settings: Option<SettingsPanel>,
    pub toast: Option<Toast>,
    pub toast_seq: u64,
    pub translation: TranslationState,
    pub auth_help: Option<AuthHelpProvider>,
    pub confirm: Option<ConfirmAction>,
    pub syncing: bool,
    pub tray: Option<crate::tray::Tray>,
    pub quitting: bool,
    pub welcome: bool,
    pub focus_handle: FocusHandle,
    pub reader_scroll: ScrollHandle,
    pub list_scroll: ScrollHandle,
    pub compose_scroll: ScrollHandle,
    pub compose_seq: u64,
    pub settings_error: Option<SharedString>,
}

impl Focusable for Root {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Root {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = Store::open(store::db_path()).expect("无法打开本地数据库");
        let _ = store.purge_demo_accounts();
        let accounts = store.accounts().unwrap_or_default();
        let active_account = store
            .setting_i64("active_account")
            .filter(|id| accounts.iter().any(|account| account.id == *id))
            .or_else(|| accounts.first().map(|account| account.id))
            .unwrap_or(0);
        let folders = store.folders(active_account).unwrap_or_default();
        let selected_folder = store
            .setting("selected_folder")
            .ok()
            .flatten()
            .filter(|key| folders.iter().any(|folder| folder.key == *key))
            .unwrap_or_else(|| "INBOX".to_string());
        let folder_messages = store
            .messages_page(active_account, &selected_folder, PAGE_SIZE, 0)
            .unwrap_or_default();
        let has_more = folder_messages.len() == PAGE_SIZE;
        let messages = folder_messages.clone();
        let selected = (!messages.is_empty()).then_some(0);
        let grouped = store
            .setting("grouped")
            .ok()
            .flatten()
            .as_deref()
            == Some("1");
        let threads = build_threads(&messages);
        let selected_thread = (!threads.is_empty()).then_some(0);
        let theme = if store.setting("theme").ok().flatten().as_deref() == Some("light") {
            Theme::light()
        } else {
            Theme::dark()
        };

        let search = cx.new(|cx| {
            let mut input = TextInput::new("搜索邮件、联系人或内容…", "", cx);
            input.set_colors(input_colors(&theme));
            input
        });
        cx.observe(&search, |this, _search, cx| {
            this.on_search_changed(cx);
        })
        .detach();

        let tray = crate::tray::install(window);
        let tray_actions = tray.as_ref().map(|tray| tray.actions());
        let mut root = Self {
            theme,
            store,
            accounts,
            active_account,
            folders,
            selected_folder,
            messages,
            has_more,
            older_loading: false,
            contact_suggestions: Vec::new(),
            folder_messages,
            selected,
            threads,
            selected_thread,
            grouped,
            query: String::new(),
            search_all: false,
            search,
            compose: None,
            settings: None,
            toast: None,
            toast_seq: 0,
            translation: TranslationState::default(),
            auth_help: None,
            confirm: None,
            syncing: false,
            tray,
            quitting: false,
            welcome: false,
            focus_handle: cx.focus_handle(),
            reader_scroll: ScrollHandle::new(),
            list_scroll: ScrollHandle::new(),
            compose_scroll: ScrollHandle::new(),
            compose_seq: 0,
            settings_error: None,
        };
        window.focus(&root.focus_handle);
        let weak = cx.weak_entity();
        window.on_window_should_close(cx, move |_window, cx| {
            weak.update(cx, |this, cx| this.should_close_window(cx))
                .unwrap_or(true)
        });
        // Tray actions arrive from the Win32 tray thread. Poll the channel
        // outside the entity so idle ticks never borrow Root while GPUI is
        // rendering; only touch the entity when an action is pending.
        if let Some(actions) = tray_actions {
            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(300))
                        .await;
                    let mut pending: Vec<crate::tray::TrayAction> = Vec::new();
                    if let Ok(receiver) = actions.lock() {
                        while let Ok(action) = receiver.try_recv() {
                            pending.push(action);
                        }
                    }
                    if pending.is_empty() {
                        continue;
                    }
                    for action in pending {
                        match action {
                            crate::tray::TrayAction::Open => {
                                // Keep Win32 window activation out of Root's
                                // entity update: ShowWindow can dispatch messages
                                // back into GPUI and double-borrow the entity.
                                std::thread::spawn(crate::tray::show_main_window);
                            }
                            crate::tray::TrayAction::Sync => {
                                this.update(cx, |this, cx| this.sync(cx)).ok();
                            }
                            crate::tray::TrayAction::Quit => {
                                this.update(cx, |this, cx| {
                                    this.quitting = true;
                                    this.save_draft_on_close(cx);
                                    cx.quit();
                                })
                                .ok();
                            }
                            crate::tray::TrayAction::Unavailable => {
                                this.update(cx, |this, cx| {
                                    this.notify(
                                        "系统托盘不可用：请把 mail.exe 复制到桌面后运行，或运行 scripts/launch-mail.ps1",
                                        ToastKind::Info,
                                        cx,
                                    );
                                })
                                .ok();
                            }
                        }
                    }
                }
            })
            .detach();
        }
        // Automatic incremental refresh for real accounts. Keep it gentle:
        // one startup sync, then every five minutes, never while a modal is open.
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_secs(8))
                .await;
            loop {
                this.update(cx, |this, cx| {
                    if this.syncing
                        || this.settings.is_some()
                        || this.compose.is_some()
                        || this.confirm.is_some()
                    {
                        return;
                    }
                    if this
                        .account()
                        .is_some_and(|account| !account.password.is_empty())
                    {
                        this.sync_inner(false, cx);
                    }
                })
                .ok();
                cx.background_executor()
                    .timer(Duration::from_secs(300))
                    .await;
            }
        })
        .detach();
        if root.accounts.is_empty() {
            root.welcome = true;
            root.begin_welcome(cx);
            if let Some(settings) = root.settings.as_ref() {
                let focus = settings.email.read(cx).handle();
                window.focus(&focus);
            }
        }
        root
    }

    fn begin_welcome(&mut self, cx: &mut Context<Self>) {
        self.welcome = true;
        self.add_account(cx);
        self.settings_error = None;
        cx.notify();
    }

    pub fn account(&self) -> Option<&Account> {
        self.accounts
            .iter()
            .find(|account| account.id == self.active_account)
            .or_else(|| self.accounts.first())
    }

    pub fn modal_open(&self) -> bool {
        self.compose.is_some() || self.settings.is_some() || self.confirm.is_some()
    }

    pub fn current_message(&self) -> Option<&Message> {
        if self.grouped {
            self.current_thread().map(|thread| thread.latest())
        } else {
            self.selected.and_then(|index| self.messages.get(index))
        }
    }

    pub fn current_thread(&self) -> Option<&Thread> {
        self.selected_thread
            .and_then(|index| self.threads.get(index))
    }

    pub fn folder(&self, key: &str) -> Option<&Folder> {
        self.folders.iter().find(|folder| folder.key == key)
    }

    pub fn selected_folder_label(&self) -> String {
        self.folder(&self.selected_folder)
            .map(|folder| folder.label().to_string())
            .unwrap_or_else(|| folder_label(&self.selected_folder))
    }

    pub fn notify(&mut self, message: impl Into<String>, kind: ToastKind, cx: &mut Context<Self>) {
        self.show_toast(message, kind, None, cx);
    }

    pub fn show_toast(
        &mut self,
        message: impl Into<String>,
        kind: ToastKind,
        action: Option<ToastAction>,
        cx: &mut Context<Self>,
    ) {
        self.toast_seq += 1;
        let id = self.toast_seq;
        self.toast = Some(Toast {
            message: message.into(),
            kind,
            id,
            action,
        });
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(3600))
                .await;
            this.update(cx, |this, cx| {
                if this.toast.as_ref().map(|toast| toast.id) == Some(id) {
                    this.toast = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub fn undo_move(&mut self, cx: &mut Context<Self>) {
        let Some(ToastAction::UndoMove { items }) =
            self.toast.as_ref().and_then(|toast| toast.action.clone())
        else {
            return;
        };
        for (id, folder) in &items {
            let _ = self.store.move_message(*id, folder);
        }
        self.toast = None;
        self.reload_messages(cx);
        self.refresh_folders();
        self.notify("已撤销上一步", ToastKind::Info, cx);
    }

    fn reload_messages(&mut self, cx: &mut Context<Self>) {
        self.folder_messages = self
            .store
            .messages_page(self.active_account, &self.selected_folder, PAGE_SIZE, 0)
            .unwrap_or_default();
        self.has_more = self.folder_messages.len() == PAGE_SIZE;
        self.apply_query(cx);
    }

    pub fn load_more_messages(&mut self, cx: &mut Context<Self>) {
        if !self.has_more || !self.query.is_empty() {
            return;
        }
        let offset = self.folder_messages.len();
        let page = self
            .store
            .messages_page(
                self.active_account,
                &self.selected_folder,
                PAGE_SIZE,
                offset,
            )
            .unwrap_or_default();
        self.has_more = page.len() == PAGE_SIZE;
        if page.is_empty() {
            cx.notify();
            return;
        }
        let existing: HashSet<i64> = self.folder_messages.iter().map(|message| message.id).collect();
        self.folder_messages
            .extend(page.into_iter().filter(|message| !existing.contains(&message.id)));
        let offset = self.list_scroll.offset();
        self.apply_query(cx);
        self.list_scroll.set_offset(offset);
        cx.notify();
    }

    pub fn load_older_from_server(&mut self, cx: &mut Context<Self>) {
        if self.older_loading || self.syncing || !self.query.is_empty() {
            return;
        }
        let Some(account) = self.account().cloned() else {
            return;
        };
        let Some(folder) = self.folder(&self.selected_folder).cloned() else {
            return;
        };
        let Some(server_name) = folder.server_name.clone() else {
            return;
        };
        let folder_key = folder.key.clone();
        let before_uid = if folder.oldest_uid > 0 {
            folder.oldest_uid
        } else {
            self.store
                .uids(account.id, &folder_key)
                .unwrap_or_default()
                .into_iter()
                .map(|(_, uid)| uid)
                .min()
                .unwrap_or(0)
        };
        self.older_loading = true;
        cx.notify();
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let account_for_fetch = account.clone();
            let db_path = store::db_path();
            let result = executor
                .spawn(async move {
                    let (messages, oldest) = mail::imap::fetch_older(
                        &account_for_fetch,
                        &folder_key,
                        &server_name,
                        before_uid,
                        200,
                    )?;
                    let store = Store::open(&db_path)?;
                    let mut added = 0usize;
                    for message in &messages {
                        if let Some(existing) = store
                            .folder_for_message_id(account_for_fetch.id, &message.message_id)
                            && existing != folder_key
                        {
                            continue;
                        }
                        if store.upsert(message).is_ok() {
                            added += 1;
                        }
                    }
                    if oldest > 0 {
                        let _ = store.update_folder_oldest(
                            account_for_fetch.id,
                            &folder_key,
                            oldest,
                        );
                    }
                    Ok::<_, anyhow::Error>((added, oldest))
                })
                .await;
            this.update(cx, |this, cx| {
                this.older_loading = false;
                match result {
                    Ok((added, _oldest)) if added > 0 => {
                        this.folders = this.store.folders(this.active_account).unwrap_or_default();
                        this.refresh_folders();
                        this.reload_messages(cx);
                        this.notify(
                            format!("已从服务器加载 {added} 封更早邮件"),
                            ToastKind::Success,
                            cx,
                        );
                    }
                    Ok(_) => this.notify("没有更早的邮件了", ToastKind::Info, cx),
                    Err(err) => this.notify(
                        format!("加载更早邮件失败：{err}"),
                        ToastKind::Error,
                        cx,
                    ),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn refresh_folders(&mut self) {
        self.folders = self.store.folders(self.active_account).unwrap_or_default();
    }

    fn apply_query(&mut self, cx: &mut Context<Self>) {
        // Keep translation in sync when a search changes the visible message.

        let query = self.query.trim().to_lowercase();
        if !query.is_empty() && self.search_all {
            self.messages = self
                .store
                .search_messages(self.active_account, &query, 500)
                .unwrap_or_default();
            self.has_more = false;
        } else if query.is_empty() {
            self.messages = self.folder_messages.clone();
        } else {
            self.messages = self
                .folder_messages
                .iter()
                .filter(|message| {
                    message.subject.to_lowercase().contains(&query)
                        || message.from_name.to_lowercase().contains(&query)
                        || message.from_email.to_lowercase().contains(&query)
                        || message.preview.to_lowercase().contains(&query)
                        || message.body.to_lowercase().contains(&query)
                })
                .cloned()
                .collect();
        }
        self.threads = build_threads(&self.messages);
        self.selected = (!self.messages.is_empty()).then_some(0);
        self.selected_thread = (!self.threads.is_empty()).then_some(0);
        self.reader_scroll = ScrollHandle::new();
        self.list_scroll = ScrollHandle::new();
        // Translation is opt-in per view: never send mail content to the online
        // service just because the user changed folders or searched.
        self.translation.visible = false;
        cx.notify();
    }

    fn on_search_changed(&mut self, cx: &mut Context<Self>) {
        self.query = self.search.read(cx).value().to_string();
        if self.query.trim().is_empty() {
            self.reload_messages(cx);
        } else {
            self.apply_query(cx);
        }
    }

    pub fn switch_account(&mut self, id: i64, cx: &mut Context<Self>) {
        if id == self.active_account {
            return;
        }
        self.active_account = id;
        let _ = self.store.set_setting("active_account", &id.to_string());
        self.folders = self.store.folders(id).unwrap_or_default();
        self.selected_folder = "INBOX".to_string();
        let _ = self.store.set_setting("selected_folder", "INBOX");
        self.query.clear();
        self.search.update(cx, |input, cx| input.clear(cx));
        self.reload_messages(cx);
        self.reader_scroll = ScrollHandle::new();
        cx.notify();
    }

    pub fn select_folder(&mut self, key: String, cx: &mut Context<Self>) {
        if self.selected_folder == key && self.query.is_empty() {
            return;
        }
        self.selected_folder = key;
        let _ = self.store.set_setting("selected_folder", &self.selected_folder);
        self.query.clear();
        self.search.update(cx, |input, cx| input.clear(cx));
        self.reload_messages(cx);
        self.translation.visible = false;
        self.reader_scroll = ScrollHandle::new();
    }

    pub fn select_message(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.messages.len() {
            return;
        }
        self.selected = Some(index);
        let id = self.messages[index].id;
        let uid = self.messages[index].uid;
        let folder = self.messages[index].folder.clone();
        let was_unread = !self.messages[index].is_read;
        if was_unread {
            self.messages[index].is_read = true;
            if let Some(message) = self.folder_messages.iter_mut().find(|m| m.id == id) {
                message.is_read = true;
            }
            let _ = self.store.set_read(id, true);
            self.threads = build_threads(&self.messages);
            self.refresh_folders();
            self.persist_flag(&folder, uid, Some(true), None, cx);
        }
        self.translation.visible = false;
        window.focus(&self.focus_handle);
        self.reader_scroll = ScrollHandle::new();
        cx.notify();
    }

    pub fn toggle_search_scope(&mut self, cx: &mut Context<Self>) {
        self.search_all = !self.search_all;
        self.apply_query(cx);
    }

    pub fn toggle_grouping(&mut self, cx: &mut Context<Self>) {
        self.grouped = !self.grouped;
        let _ = self.store.set_setting(
            "grouped",
            if self.grouped { "1" } else { "0" },
        );
        self.threads = build_threads(&self.messages);
        self.selected = (!self.messages.is_empty()).then_some(0);
        self.selected_thread = (!self.threads.is_empty()).then_some(0);
        self.translation.visible = false;
        self.reader_scroll = ScrollHandle::new();
        let text = if self.grouped {
            "已按联系人聚合会话"
        } else {
            "已切换为平铺列表"
        };
        self.notify(text, ToastKind::Info, cx);
    }

    pub fn select_thread(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.threads.len() {
            return;
        }
        self.selected_thread = Some(index);
        let thread = self.threads[index].clone();
        let unread_uids: Vec<u32> = thread
            .messages
            .iter()
            .filter(|message| !message.is_read && message.folder == "INBOX")
            .map(|message| message.uid)
            .collect();
        let had_unread = thread.messages.iter().any(|message| !message.is_read);
        if had_unread {
            for message in &thread.messages {
                if !message.is_read {
                    let _ = self.store.set_read(message.id, true);
                }
            }
            let key = thread.key.clone();
            for message in self.messages.iter_mut() {
                if message.from_email.trim().to_lowercase() == key {
                    message.is_read = true;
                }
            }
            if let Some(thread) = self.threads.get_mut(index) {
                for message in thread.messages.iter_mut() {
                    message.is_read = true;
                }
            }
            self.refresh_folders();
        }
        if !unread_uids.is_empty() {
            self.persist_seen_batch(unread_uids, true, cx);
        }
        self.translation.visible = false;
        window.focus(&self.focus_handle);
        self.reader_scroll = ScrollHandle::new();
        // Conversations are rendered oldest first; open them at the newest mail.
        if self.threads.get(index).is_some_and(|thread| thread.count() > 1) {
            self.reader_scroll.scroll_to_bottom();
        }
        cx.notify();
    }

    fn persist_flag(
        &self,
        folder: &str,
        uid: u32,
        seen: Option<bool>,
        flagged: Option<bool>,
        cx: &mut Context<Self>,
    ) {
        if folder != "INBOX" {
            return;
        }
        let Some(account) = self.account().cloned() else {
            return;
        };
        cx.background_executor()
            .spawn(async move {
                let _ = mail::imap::set_system_flag(&account, "INBOX", uid, seen, flagged);
            })
            .detach();
    }

    pub fn toggle_star(&mut self, cx: &mut Context<Self>) {
        if self.grouped {
            let Some(thread) = self.current_thread().cloned() else {
                return;
            };
            let starred = !thread.starred();
            let items: Vec<(i64, u32)> = thread
                .messages
                .iter()
                .map(|message| (message.id, message.uid))
                .collect();
            for (id, _) in &items {
                let _ = self.store.set_starred(*id, starred);
            }
            for item in self.messages.iter_mut() {
                if items.iter().any(|(id, _)| id == &item.id) {
                    item.is_starred = starred;
                }
            }
            for item in self.folder_messages.iter_mut() {
                if items.iter().any(|(id, _)| id == &item.id) {
                    item.is_starred = starred;
                }
            }
            for thread in self.threads.iter_mut() {
                for item in thread.messages.iter_mut() {
                    if items.iter().any(|(id, _)| id == &item.id) {
                        item.is_starred = starred;
                    }
                }
            }
            self.refresh_folders();
            let inbox_uids: Vec<u32> = self
                .messages
                .iter()
                .filter(|item| {
                    items.iter().any(|(id, _)| id == &item.id) && item.folder == "INBOX"
                })
                .map(|item| item.uid)
                .collect();
            self.persist_flagged_batch(inbox_uids, starred, cx);
            if self.selected_folder == "STARRED" && !starred {
                self.reload_messages(cx);
            }
            let text = if starred {
                "已为整段会话加星标"
            } else {
                "已取消整段会话星标"
            };
            self.notify(text, ToastKind::Info, cx);
            return;
        }
        let Some(message) = self.current_message().cloned() else {
            return;
        };
        let id = message.id;
        let uid = message.uid;
        let starred = !message.is_starred;
        for item in self.messages.iter_mut().filter(|item| item.id == id) {
            item.is_starred = starred;
        }
        for item in self.folder_messages.iter_mut().filter(|item| item.id == id) {
            item.is_starred = starred;
        }
        for thread in self.threads.iter_mut() {
            for item in thread.messages.iter_mut().filter(|item| item.id == id) {
                item.is_starred = starred;
            }
        }
        let _ = self.store.set_starred(id, starred);
        self.refresh_folders();
        self.persist_flag(&message.folder, uid, None, Some(starred), cx);
        if self.selected_folder == "STARRED" && !starred {
            self.reload_messages(cx);
        }
        let text = if starred { "已加星标" } else { "已取消星标" };
        self.notify(text, ToastKind::Info, cx);
    }

    pub fn toggle_unread(&mut self, cx: &mut Context<Self>) {
        let Some(message) = self.current_message().cloned() else {
            return;
        };
        let read = !message.is_read;
        let ids: Vec<(i64, u32)> = if self.grouped {
            self.current_thread()
                .map(|thread| {
                    thread
                        .messages
                        .iter()
                        .map(|item| (item.id, item.uid))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            vec![(message.id, message.uid)]
        };
        for (id, _) in &ids {
            let _ = self.store.set_read(*id, read);
        }
        for item in self.messages.iter_mut() {
            if ids.iter().any(|(id, _)| id == &item.id) {
                item.is_read = read;
            }
        }
        for item in self.folder_messages.iter_mut() {
            if ids.iter().any(|(id, _)| id == &item.id) {
                item.is_read = read;
            }
        }
        for thread in self.threads.iter_mut() {
            for item in thread.messages.iter_mut() {
                if ids.iter().any(|(id, _)| id == &item.id) {
                    item.is_read = read;
                }
            }
        }
        self.refresh_folders();
        let uids: Vec<u32> = self
            .messages
            .iter()
            .filter(|item| ids.iter().any(|(id, _)| id == &item.id) && item.folder == "INBOX")
            .map(|item| item.uid)
            .collect();
        self.persist_seen_batch(uids, read, cx);
        cx.notify();
    }

    fn current_scope_ids(&self) -> Vec<i64> {
        if self.grouped {
            self.current_thread()
                .map(|thread| thread.messages.iter().map(|item| item.id).collect())
                .unwrap_or_default()
        } else {
            self.current_message()
                .map(|message| vec![message.id])
                .unwrap_or_default()
        }
    }

    fn persist_flagged_batch(&self, uids: Vec<u32>, flagged: bool, cx: &mut Context<Self>) {
        if uids.is_empty() {
            return;
        }
        let Some(account) = self.account().cloned() else {
            return;
        };
        cx.background_executor()
            .spawn(async move {
                let _ = mail::imap::set_flagged_batch(&account, "INBOX", &uids, flagged);
            })
            .detach();
    }

    fn persist_seen_batch(&self, uids: Vec<u32>, seen: bool, cx: &mut Context<Self>) {
        if uids.is_empty() {
            return;
        }
        let Some(account) = self.account().cloned() else {
            return;
        };
        cx.background_executor()
            .spawn(async move {
                let _ = mail::imap::set_seen_batch(&account, "INBOX", &uids, seen);
            })
            .detach();
    }

    pub fn mark_all_read(&mut self, cx: &mut Context<Self>) {
        let unread: Vec<(i64, u32, String)> = self
            .folder_messages
            .iter()
            .filter(|message| !message.is_read)
            .map(|message| (message.id, message.uid, message.folder.clone()))
            .collect();
        if unread.is_empty() {
            return;
        }
        for (id, _, _) in &unread {
            let _ = self.store.set_read(*id, true);
        }
        for message in self.folder_messages.iter_mut() {
            message.is_read = true;
        }
        // Starred/Search views can contain messages from several local folders;
        // only INBOX has server-side flags to update.
        let inbox_uids: Vec<u32> = unread
            .iter()
            .filter(|(_, _, folder)| folder == "INBOX")
            .map(|(_, uid, _)| *uid)
            .collect();
        if !inbox_uids.is_empty() {
            self.persist_seen_batch(inbox_uids, true, cx);
        }
        self.apply_query(cx);
        self.refresh_folders();
        self.notify("已将当前文件夹全部标为已读", ToastKind::Info, cx);
    }

    pub fn archive_current(&mut self, cx: &mut Context<Self>) {
        self.move_current("ARCHIVE", "已归档", cx);
    }

    pub fn restore_current(&mut self, cx: &mut Context<Self>) {
        self.move_current("INBOX", "已移回收件箱", cx);
    }

    pub fn delete_current(&mut self, cx: &mut Context<Self>) {
        if self.selected_folder == "TRASH" {
            let ids = self.current_scope_ids();
            if ids.is_empty() {
                return;
            }
            self.confirm = Some(ConfirmAction::PermanentDelete { ids });
            cx.notify();
            return;
        }
        self.move_current("TRASH", "已移至已删除", cx);
    }

    pub fn cancel_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm = None;
        window.focus(&self.focus_handle);
        cx.notify();
    }

    pub fn confirm_destructive_action(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(action) = self.confirm.take() else {
            return;
        };
        match action {
            ConfirmAction::PermanentDelete { ids } => {
                let account = self.account().cloned();
                let mut remote_deletes: Vec<(String, u32)> = Vec::new();
                if account.is_some() {
                    for id in &ids {
                        if let Some(item) = self.message_by_id(*id)
                            && let Some(mailbox) = self.server_mailbox(&item.folder)
                        {
                            remote_deletes.push((mailbox, item.uid));
                        }
                    }
                }
                for id in &ids {
                    let _ = self.store.delete_message(*id);
                }
                self.folder_messages.retain(|message| !ids.contains(&message.id));
                self.apply_query(cx);
                self.refresh_folders();
                let text = if ids.len() > 1 {
                    format!("已永久删除 {} 封邮件", ids.len())
                } else {
                    "邮件已永久删除".to_string()
                };
                self.notify(text, ToastKind::Info, cx);
                if let Some(account) = account
                    && !remote_deletes.is_empty()
                {
                    cx.background_executor()
                        .spawn(async move {
                            for (mailbox, uid) in remote_deletes {
                                let _ = mail::imap::delete_remote(&account, &mailbox, uid);
                            }
                        })
                        .detach();
                }
            }
            ConfirmAction::RemoveAccount { id, email } => {
                self.remove_account_now(id, window, cx);
                self.notify(format!("账户 {email} 已移除"), ToastKind::Info, cx);
            }
        }
        window.focus(&self.focus_handle);
        cx.notify();
    }

    fn server_mailbox(&self, key: &str) -> Option<String> {
        self.folder(key)
            .and_then(|folder| folder.server_name.clone())
            .or_else(|| (key == "INBOX").then(|| "INBOX".to_string()))
    }

    fn move_current(&mut self, folder: &str, message: &str, cx: &mut Context<Self>) {
        let ids = self.current_scope_ids();
        if ids.is_empty() {
            return;
        }
        let originals: Vec<(i64, String)> = ids
            .iter()
            .filter_map(|id| {
                self.message_by_id(*id)
                    .map(|item| (*id, item.folder.clone()))
            })
            .collect();
        let account = self.account().cloned();
        let mut remote_moves: Vec<(String, u32, String)> = Vec::new();
        if account.is_some() {
            let target_mailbox = self.server_mailbox(folder);
            for id in &ids {
                if let Some(item) = self.message_by_id(*id)
                    && let Some(source_mailbox) = self.server_mailbox(&item.folder)
                    && let Some(target_mailbox) = target_mailbox.clone()
                    && source_mailbox != target_mailbox
                {
                    remote_moves.push((source_mailbox, item.uid, target_mailbox));
                }
            }
        }
        for id in &ids {
            let _ = self.store.move_message(*id, folder);
        }
        let keep_thread = self.selected_thread.unwrap_or(0);
        let keep_message = self.selected.unwrap_or(0);
        self.folder_messages.retain(|item| !ids.contains(&item.id));
        self.apply_query(cx);
        if self.grouped {
            if !self.threads.is_empty() {
                self.selected_thread = Some(keep_thread.min(self.threads.len() - 1));
            }
        } else if !self.messages.is_empty() {
            self.selected = Some(keep_message.min(self.messages.len() - 1));
        }
        self.refresh_folders();
        let text = if ids.len() > 1 {
            format!("{message} · {} 封", ids.len())
        } else {
            message.to_string()
        };
        if originals.is_empty() {
            self.notify(text, ToastKind::Info, cx);
        } else {
            self.show_toast(
                text,
                ToastKind::Info,
                Some(ToastAction::UndoMove { items: originals }),
                cx,
            );
        }
        if let Some(account) = account
            && !remote_moves.is_empty()
        {
            cx.background_executor()
                .spawn(async move {
                    for (source, uid, target) in remote_moves {
                        let _ = mail::imap::move_remote(&account, &source, uid, &target);
                    }
                })
                .detach();
        }
    }

    pub fn step_selection(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        if self.grouped {
            if self.threads.is_empty() {
                return;
            }
            let current = self.selected_thread.unwrap_or(0) as isize;
            let next = (current + delta).clamp(0, self.threads.len() as isize - 1) as usize;
            self.select_thread(next, window, cx);
            return;
        }
        if self.messages.is_empty() {
            return;
        }
        let current = self.selected.unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, self.messages.len() as isize - 1) as usize;
        self.select_message(next, window, cx);
    }

    fn translation_targets(&self) -> Vec<(i64, String)> {
        if self.grouped {
            self.current_thread()
                .map(|thread| {
                    thread
                        .messages
                        .iter()
                        .map(|message| (message.id, message.body.clone()))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            self.current_message()
                .map(|message| vec![(message.id, message.body.clone())])
                .unwrap_or_default()
        }
    }

    pub fn message_by_id(&self, id: i64) -> Option<&Message> {
        self.messages
            .iter()
            .chain(self.folder_messages.iter())
            .chain(self.threads.iter().flat_map(|thread| thread.messages.iter()))
            .find(|message| message.id == id)
    }

    pub fn toggle_translation(&mut self, cx: &mut Context<Self>) {
        if self.translation.visible {
            self.translation.visible = false;
            cx.notify();
            return;
        }

        let targets = self.translation_targets();
        if targets.is_empty() {
            self.notify("没有可翻译的正文", ToastKind::Info, cx);
            return;
        }

        self.translation.visible = true;
        for (id, body) in targets {
            if matches!(
                self.translation.entries.get(&id),
                Some(TranslationEntry::Failed(_))
            ) {
                self.translation.entries.remove(&id);
            }
            if !self.translation.entries.contains_key(&id) {
                self.request_translation(id, body, cx);
            }
        }
        cx.notify();
    }

    fn request_translation(&mut self, id: i64, body: String, cx: &mut Context<Self>) {
        self.translation
            .entries
            .insert(id, TranslationEntry::Loading);
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { translate::translate(&body) })
                .await;
            this.update(cx, |this, cx| {
                let entry = match result {
                    Ok(translation) => TranslationEntry::Ready(translation.text),
                    Err(err) => TranslationEntry::Failed(err.to_string()),
                };
                this.translation.entries.insert(id, entry);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn retry_translation(&mut self, id: i64, cx: &mut Context<Self>) {
        let Some(body) = self.message_by_id(id).map(|message| message.body.clone()) else {
            return;
        };
        self.translation.entries.remove(&id);
        self.request_translation(id, body, cx);
    }

    pub fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        self.theme = self.theme.toggle();
        let _ = self.store.set_setting(
            "theme",
            if self.theme.dark { "dark" } else { "light" },
        );
        self.apply_input_colors(cx);
        cx.notify();
    }

    fn apply_input_colors(&mut self, cx: &mut Context<Self>) {
        let colors = input_colors(&self.theme);
        self.search.update(cx, |input, cx| {
            input.set_colors(colors);
            cx.notify();
        });
        if let Some(compose) = &self.compose {
            for field in [&compose.to, &compose.cc, &compose.subject] {
                field.update(cx, |input, cx| {
                    input.set_colors(colors);
                    cx.notify();
                });
            }
            compose.body.update(cx, |area, cx| {
                area.set_colors(colors);
                cx.notify();
            });
        }
        if let Some(settings) = &self.settings {
            for field in [
                &settings.display_name,
                &settings.email,
                &settings.password,
                &settings.imap_host,
                &settings.imap_port,
                &settings.smtp_host,
                &settings.smtp_port,
            ] {
                field.update(cx, |input, cx| {
                    input.set_colors(colors);
                    cx.notify();
                });
            }
        }
    }

    fn text_input(
        &self,
        cx: &mut Context<Self>,
        placeholder: &str,
        value: &str,
    ) -> Entity<TextInput> {
        let colors = input_colors(&self.theme);
        cx.new(|cx| {
            let mut input = TextInput::new(placeholder, value, cx);
            input.set_colors(colors);
            input
        })
    }

    /// Deterministic views used for documentation screenshots.
    pub fn open_compose(
        &mut self,
        mode: ComposeMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let account = self.account().cloned().unwrap_or_else(Account::empty);
        let mut to = String::new();
        let mut cc = String::new();
        let mut subject = String::new();
        let mut body = String::new();
        let mut in_reply_to = None;
        let mut focus_body = false;

        if let Some(message) = self.current_message().cloned() {
            match mode {
                ComposeMode::Reply | ComposeMode::ReplyAll => {
                    to = format_addrs(&[format!("{} <{}>", message.from_name, message.from_email)]);
                    subject = message.reply_subject();
                    in_reply_to = Some(message.message_id.clone());
                    body = quoted_body(&message);
                    if mode == ComposeMode::ReplyAll {
                        let mut extras: Vec<String> = Vec::new();
                        for addr in message.to.iter().chain(message.cc.iter()) {
                            if !addr.contains(&account.email) && !addr.contains(&message.from_email) {
                                extras.push(addr.clone());
                            }
                        }
                        cc = format_addrs(&extras);
                    }
                    focus_body = true;
                }
                ComposeMode::Forward => {
                    subject = message.forward_subject();
                    body = quoted_body(&message);
                    focus_body = true;
                }
                ComposeMode::New | ComposeMode::Draft => {}
            }
        }

        let to_input = self.text_input(cx, "name@example.com", &to);
        let cc_input = self.text_input(cx, "抄送地址（可选）", &cc);
        let bcc_input = self.text_input(cx, "密送地址（可选）", "");
        let subject_input = self.text_input(cx, "填写主题", &subject);
        let body_width = px(664.);
        let colors = input_colors(&self.theme);
        let body_input = cx.new(|cx| {
            let mut area = TextArea::new("写点什么…", &body, body_width, cx);
            area.set_colors(colors);
            area
        });

        let focus_handle = if focus_body {
            body_input.read(cx).handle()
        } else {
            to_input.read(cx).handle()
        };
        let initial_fingerprint = draft_fingerprint(&to, &cc, "", &subject, &body, &[]);
        self.compose = Some(Compose {
            to: to_input,
            cc: cc_input,
            bcc: bcc_input,
            subject: subject_input,
            body: body_input,
            show_cc: !cc.is_empty(),
            show_bcc: false,
            in_reply_to,
            sending: false,
            mode,
            draft_id: None,
            last_saved: initial_fingerprint,
            attachments: Vec::new(),
            draft_message_id: String::new(),
        });
        self.compose_scroll = ScrollHandle::new();
        self.contact_suggestions.clear();
        self.schedule_draft_autosave(cx);
        let to_watch = self.compose.as_ref().unwrap().to.clone();
        cx.observe(&to_watch, |this, _input, cx| {
            this.refresh_contact_suggestions(cx);
        })
        .detach();
        window.focus(&focus_handle);
        cx.notify();
    }

    pub fn open_draft(&mut self, message_id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(message) = self.message_by_id(message_id).cloned() else {
            return;
        };
        if !message.is_draft {
            return;
        }
        let to = format_addrs(&message.to);
        let cc = format_addrs(&message.cc);
        let bcc = format_addrs(&message.bcc);
        let subject = message.subject.clone();
        let body = message.body.clone();
        let attachments = message.attachments.clone();
        let initial_fingerprint =
            draft_fingerprint(&to, &cc, &bcc, &subject, &body, &attachments);

        let to_input = self.text_input(cx, "name@example.com", &to);
        let cc_input = self.text_input(cx, "抄送地址（可选）", &cc);
        let bcc_input = self.text_input(cx, "密送地址（可选）", &bcc);
        let subject_input = self.text_input(cx, "填写主题", &subject);
        let colors = input_colors(&self.theme);
        let body_width = px(664.);
        let body_input = cx.new(|cx| {
            let mut area = TextArea::new("写点什么…", &body, body_width, cx);
            area.set_colors(colors);
            area
        });
        let focus_handle = body_input.read(cx).handle();
        self.compose = Some(Compose {
            to: to_input,
            cc: cc_input,
            bcc: bcc_input,
            subject: subject_input,
            body: body_input,
            show_cc: !cc.is_empty(),
            show_bcc: !bcc.is_empty(),
            in_reply_to: None,
            sending: false,
            mode: ComposeMode::Draft,
            draft_id: Some(message.id),
            last_saved: initial_fingerprint,
            attachments,
            draft_message_id: message.message_id.clone(),
        });
        self.compose_scroll = ScrollHandle::new();
        self.contact_suggestions.clear();
        self.schedule_draft_autosave(cx);
        let to_watch = self.compose.as_ref().unwrap().to.clone();
        cx.observe(&to_watch, |this, _input, cx| {
            this.refresh_contact_suggestions(cx);
        })
        .detach();
        window.focus(&focus_handle);
        cx.notify();
    }

    fn refresh_contact_suggestions(&mut self, cx: &mut Context<Self>) {
        let Some(compose) = self.compose.as_ref() else {
            self.contact_suggestions.clear();
            cx.notify();
            return;
        };
        let value = compose.to.read(cx).value().to_string();
        let token = value
            .rsplit([',', ';'])
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if token.is_empty() {
            self.contact_suggestions.clear();
            cx.notify();
            return;
        }
        let account_email = self
            .account()
            .map(|account| account.email.to_lowercase())
            .unwrap_or_default();
        let existing: Vec<String> = split_addresses(&value)
            .into_iter()
            .map(|address| address.to_lowercase())
            .collect();
        let mut suggestions = Vec::new();
        if let Ok(contacts) = self.store.contacts(self.active_account, &token, 12) {
            for (name, email) in contacts {
                let lower = email.to_lowercase();
                if lower == account_email
                    || existing.iter().any(|item| item.contains(&lower))
                    || suggestions.iter().any(|(_, existing_email): &(String, String)| {
                        existing_email.eq_ignore_ascii_case(&email)
                    })
                {
                    continue;
                }
                suggestions.push((name, email));
                if suggestions.len() >= 5 {
                    break;
                }
            }
        }
        self.contact_suggestions = suggestions;
        cx.notify();
    }

    pub fn apply_contact_suggestion(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some((name, email)) = self.contact_suggestions.get(index).cloned() else {
            return;
        };
        let Some(compose) = self.compose.as_ref() else {
            return;
        };
        let current = compose.to.read(cx).value().to_string();
        let (head, _) = match current.rfind([',', ';']) {
            Some(position) => (&current[..=position], &current[position + 1..]),
            None => ("", current.as_str()),
        };
        let formatted = if name.trim().is_empty() {
            email
        } else {
            format!("{} <{}>", name.trim(), email)
        };
        let mut next = String::new();
        next.push_str(head.trim_end());
        if !next.is_empty() && !next.ends_with(',') {
            next.push(',');
        }
        if !next.is_empty() {
            next.push(' ');
        }
        next.push_str(&formatted);
        next.push_str(", ");
        compose.to.update(cx, |input, cx| input.set_value(next, cx));
        let focus = compose.to.read(cx).handle();
        window.focus(&focus);
        self.contact_suggestions.clear();
        cx.notify();
    }

    fn schedule_draft_autosave(&mut self, cx: &mut Context<Self>) {
        self.compose_seq = self.compose_seq.wrapping_add(1);
        let seq = self.compose_seq;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(6))
                    .await;
                let alive = this
                    .update(cx, |this, cx| {
                        if this.compose.is_none() || this.compose_seq != seq {
                            return false;
                        }
                        this.save_draft_now(false, cx);
                        true
                    })
                    .unwrap_or(false);
                if !alive {
                    break;
                }
            }
        })
        .detach();
    }

    pub fn close_compose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .compose
            .as_ref()
            .is_some_and(|compose| compose.sending)
        {
            self.notify("正在发送，请稍候再关闭", ToastKind::Info, cx);
            return;
        }
        // Closing with unsent content should not silently throw it away.
        let dirty = self.compose.as_ref().is_some_and(|compose| {
            !compose.to.read(cx).value().trim().is_empty()
                || !compose.cc.read(cx).value().trim().is_empty()
                || !compose.bcc.read(cx).value().trim().is_empty()
                || !compose.subject.read(cx).value().trim().is_empty()
                || !compose.body.read(cx).content().trim().is_empty()
        });
        if dirty {
            self.save_draft(window, cx);
            return;
        }
        self.compose = None;
        window.focus(&self.focus_handle);
        cx.notify();
    }

    pub fn send_current(&mut self, cx: &mut Context<Self>) {
        if self
            .compose
            .as_ref()
            .is_some_and(|compose| compose.sending)
        {
            return;
        }
        let account = self.account().cloned().unwrap_or_else(Account::empty);
        let draft_id = self.compose.as_ref().and_then(|compose| compose.draft_id);
        let draft_message_id = self
            .compose
            .as_ref()
            .map(|compose| compose.draft_message_id.clone())
            .filter(|message_id| !message_id.is_empty());
        let Some((
            to,
            cc,
            bcc,
            subject,
            body,
            in_reply_to,
            attachments,
            total_attachment_size,
        )) = self.compose.as_ref().map(|compose| {
                (
                    compose.to.read(cx).value().trim().to_string(),
                    compose.cc.read(cx).value().trim().to_string(),
                    compose.bcc.read(cx).value().trim().to_string(),
                    compose.subject.read(cx).value().trim().to_string(),
                    compose.body.read(cx).content().to_string(),
                    compose.in_reply_to.clone(),
                    compose.attachments.clone(),
                    compose
                        .attachments
                        .iter()
                        .map(|attachment| attachment.size)
                        .sum::<u64>(),
                )
            })
        else {
            return;
        };

        if to.is_empty() {
            self.notify("请填写收件人", ToastKind::Error, cx);
            return;
        }
        if subject.is_empty() && body.trim().is_empty() && attachments.is_empty() {
            self.notify("邮件内容不能为空", ToastKind::Error, cx);
            return;
        }
        if total_attachment_size > 25 * 1024 * 1024 {
            self.notify("附件总大小超过 25 MB，请减少附件后重试", ToastKind::Error, cx);
            return;
        }

        let outgoing = Outgoing {
            from_name: account.display_name.clone(),
            from_email: account.email.clone(),
            to: split_addresses(&to),
            cc: split_addresses(&cc),
            subject: if subject.is_empty() {
                "(无主题)".to_string()
            } else {
                subject.clone()
            },
            body: body.clone(),
            in_reply_to,
            message_id: new_message_id(account.id),
            bcc: split_addresses(&bcc),
            attachments,
        };

        if account.password.is_empty() {
            self.notify("请先在设置里填写邮箱授权码", ToastKind::Error, cx);
            return;
        }

        if let Some(compose) = self.compose.as_mut() {
            compose.sending = true;
        }
        cx.notify();

        let executor = cx.background_executor().clone();
        let mail = outgoing.clone();
        let account_for_send = account.clone();
        cx.spawn(async move |this, cx| {
            let result = executor
                .spawn(async move { mail::smtp::send(&account_for_send, &mail) })
                .await;
            this.update(cx, |this, cx| {
                if let Some(compose) = this.compose.as_mut() {
                    compose.sending = false;
                }
                match result {
                    Ok(raw) => this.finish_send(
                        outgoing,
                        account,
                        draft_id,
                        draft_message_id,
                        Some(raw),
                        cx,
                    ),
                    Err(err) => {
                        this.notify(format!("发送失败：{err}"), ToastKind::Error, cx);
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    fn finish_send(
        &mut self,
        outgoing: Outgoing,
        account: Account,
        draft_id: Option<i64>,
        draft_message_id: Option<String>,
        raw: Option<Vec<u8>>,
        cx: &mut Context<Self>,
    ) {
        let message = Message {
            id: 0,
            account_id: account.id,
            folder: "SENT".to_string(),
            uid: self.store.next_local_uid(account.id, "SENT"),
            from_name: account.display_name.clone(),
            from_email: account.email.clone(),
            to: outgoing.to.clone(),
            cc: outgoing.cc.clone(),
            bcc: outgoing.bcc.clone(),
            subject: outgoing.subject.clone(),
            preview: outgoing
                .body
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(160)
                .collect(),
            body: outgoing.body.clone(),
            attachments: outgoing.attachments.clone(),
            timestamp: now_ts(),
            is_read: true,
            is_starred: false,
            is_draft: false,
            message_id: if outgoing.message_id.trim().is_empty() {
                new_message_id(account.id)
            } else {
                outgoing.message_id.clone()
            },
        };
        let _ = self.store.upsert(&message);
        if let Some(id) = draft_id {
            let _ = self.store.delete_message(id);
        }
        if let Some(message_id) = draft_message_id
            && let Some(mailbox) = self.server_mailbox("DRAFTS")
        {
            let account = account.clone();
            cx.background_executor()
                .spawn(async move {
                    let _ = mail::imap::delete_by_message_id(
                        &account,
                        &mailbox,
                        &message_id,
                    );
                })
                .detach();
        }
        if let Some(raw) = raw
            && let Some(mailbox) = self.server_mailbox("SENT")
        {
            let account = account.clone();
            cx.background_executor()
                .spawn(async move {
                    let _ = mail::imap::append_message(&account, &mailbox, &raw, false);
                })
                .detach();
        }
        self.refresh_folders();
        if matches!(self.selected_folder.as_str(), "SENT" | "DRAFTS") {
            self.reload_messages(cx);
        }
        self.compose = None;
        self.compose_scroll = ScrollHandle::new();
        self.notify("邮件已发送", ToastKind::Success, cx);
    }

    fn save_draft_now(&mut self, force: bool, cx: &mut Context<Self>) -> bool {
        let Some(compose) = self.compose.as_ref() else {
            return false;
        };
        if compose.sending {
            return false;
        }
        let to = compose.to.read(cx).value().trim().to_string();
        let cc = compose.cc.read(cx).value().trim().to_string();
        let bcc = compose.bcc.read(cx).value().trim().to_string();
        let subject = compose.subject.read(cx).value().trim().to_string();
        let body = compose.body.read(cx).content().to_string();
        let attachments = compose.attachments.clone();
        let draft_id = compose.draft_id;
        let mut draft_message_id = compose.draft_message_id.clone();
        if to.is_empty()
            && cc.is_empty()
            && bcc.is_empty()
            && subject.is_empty()
            && body.trim().is_empty()
        {
            return false;
        }
        let fingerprint = draft_fingerprint(&to, &cc, &bcc, &subject, &body, &attachments);
        if !force && fingerprint == compose.last_saved {
            return false;
        }
        let account = self.account().cloned().unwrap_or_else(Account::empty);
        let mut message = Message {
            id: draft_id.unwrap_or(0),
            account_id: account.id,
            folder: "DRAFTS".to_string(),
            uid: 0,
            from_name: account.display_name.clone(),
            from_email: account.email.clone(),
            to: split_addresses(&to),
            cc: split_addresses(&cc),
            bcc: split_addresses(&bcc),
            subject,
            preview: body.chars().take(120).collect(),
            body,
            attachments,
            timestamp: now_ts(),
            is_read: true,
            is_starred: false,
            is_draft: true,
            message_id: draft_message_id.clone(),
        };

        if draft_message_id.is_empty() {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0);
            draft_message_id =
                format!("<draft-{}-{nanos}@mail.app>", account.id);
            message.message_id = draft_message_id.clone();
        }
        let mut saved_id = None;
        if let Some(id) = draft_id {
            if self.store.update_draft(id, &message).unwrap_or(false) {
                saved_id = Some(id);
            }
        }
        if saved_id.is_none() {
            message.uid = self.store.next_local_uid(account.id, "DRAFTS");
            saved_id = self.store.upsert(&message).ok();
        }
        if let Some(id) = saved_id {
            if let Some(compose) = self.compose.as_mut() {
                compose.draft_id = Some(id);
                compose.last_saved = fingerprint;
                compose.draft_message_id = draft_message_id;
            }
            self.refresh_folders();
            cx.notify();
            true
        } else {
            false
        }
    }

    fn should_close_window(&mut self, cx: &mut Context<Self>) -> bool {
        self.save_draft_on_close(cx);
        if self.quitting {
            return true;
        }
        if self.tray.as_ref().is_some_and(|tray| tray.is_available()) {
            // Hide asynchronously so the Win32 call cannot re-enter GPUI while
            // the close callback still borrows this entity.
            std::thread::spawn(crate::tray::hide_main_window);
            false
        } else {
            true
        }
    }

    fn save_draft_on_close(&mut self, cx: &mut Context<Self>) {
        let _ = self.save_draft_now(false, cx);
    }

    pub fn save_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(compose) = self.compose.as_ref() else {
            return;
        };
        if compose.sending {
            self.notify("正在发送，请稍候再保存草稿", ToastKind::Info, cx);
            return;
        }
        let to = compose.to.read(cx).value().trim().to_string();
        let cc = compose.cc.read(cx).value().trim().to_string();
        let bcc = compose.bcc.read(cx).value().trim().to_string();
        let subject = compose.subject.read(cx).value().trim().to_string();
        let body = compose.body.read(cx).content().to_string();
        if to.is_empty()
            && cc.is_empty()
            && bcc.is_empty()
            && subject.is_empty()
            && body.trim().is_empty()
        {
            self.notify("没有可保存的草稿内容", ToastKind::Info, cx);
            return;
        }
        let _ = self.save_draft_now(true, cx);
        self.append_current_draft_to_server(cx);
        self.compose = None;
        self.compose_scroll = ScrollHandle::new();
        window.focus(&self.focus_handle);
        self.notify("草稿已保存", ToastKind::Success, cx);
    }

    fn append_current_draft_to_server(&mut self, cx: &mut Context<Self>) {
        let Some(compose) = self.compose.as_ref() else {
            return;
        };
        let account = self.account().cloned().unwrap_or_else(Account::empty);
        let Some(mailbox) = self.server_mailbox("DRAFTS") else {
            return;
        };
        let message_id = compose.draft_message_id.clone();
        let to = compose.to.read(cx).value().trim().to_string();
        let cc = compose.cc.read(cx).value().trim().to_string();
        let bcc = compose.bcc.read(cx).value().trim().to_string();
        let subject = compose.subject.read(cx).value().trim().to_string();
        let body = compose.body.read(cx).content().to_string();
        let attachments = compose.attachments.clone();
        if to.is_empty()
            && cc.is_empty()
            && bcc.is_empty()
            && subject.is_empty()
            && body.trim().is_empty()
        {
            return;
        }
        let outgoing = Outgoing {
            from_name: account.display_name.clone(),
            from_email: account.email.clone(),
            to: split_addresses(&to),
            cc: split_addresses(&cc),
            bcc: split_addresses(&bcc),
            subject: if subject.is_empty() {
                "(无主题)".to_string()
            } else {
                subject
            },
            body,
            in_reply_to: None,
            message_id: message_id.clone(),
            attachments,
        };
        let Ok(raw) = mail::smtp::build_message(&outgoing).map(|message| message.formatted())
        else {
            return;
        };
        let account = account.clone();
        cx.background_executor()
            .spawn(async move {
                if !message_id.is_empty() {
                    let _ = mail::imap::delete_by_message_id(&account, &mailbox, &message_id);
                }
                let _ = mail::imap::append_message(&account, &mailbox, &raw, true);
            })
            .detach();
    }

    pub fn add_compose_attachments(&mut self, cx: &mut Context<Self>) {
        if self.compose.is_none() {
            return;
        }
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(SharedString::from("选择附件")),
        });
        cx.spawn(async move |this, cx| {
            let result = receiver.await;
            this.update(cx, |this, cx| match result {
                Ok(Ok(Some(paths))) => this.attach_paths(paths, cx),
                Ok(Ok(None)) => {}
                Ok(Err(err)) => this.notify(
                    format!("打开文件选择器失败：{err}"),
                    ToastKind::Error,
                    cx,
                ),
                Err(_) => this.notify("文件选择已取消", ToastKind::Info, cx),
            })
            .ok();
        })
        .detach();
    }

    fn attach_paths(&mut self, paths: Vec<std::path::PathBuf>, cx: &mut Context<Self>) {
        let mut added = 0;
        let mut skipped = 0;
        let mut total = self
            .compose
            .as_ref()
            .map(|compose| {
                compose
                    .attachments
                    .iter()
                    .map(|attachment| attachment.size)
                    .sum::<u64>()
            })
            .unwrap_or(0);
        if let Some(compose) = self.compose.as_mut() {
            for path in paths {
                let Ok(metadata) = std::fs::metadata(&path) else {
                    skipped += 1;
                    continue;
                };
                if !metadata.is_file() || metadata.len() > 25 * 1024 * 1024 {
                    skipped += 1;
                    continue;
                }
                if compose.attachments.iter().any(|attachment| {
                    attachment.path.as_ref().is_some_and(|existing| existing == &path)
                }) {
                    continue;
                }
                if total + metadata.len() > 25 * 1024 * 1024 {
                    skipped += 1;
                    continue;
                }
                total += metadata.len();
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| "attachment".to_string());
                let mime = guess_mime(&name);
                compose.attachments.push(Attachment {
                    name,
                    size: metadata.len(),
                    mime,
                    path: Some(path),
                });
                added += 1;
            }
        }
        if added > 0 {
            self.notify(
                format!("已添加 {added} 个附件"),
                ToastKind::Success,
                cx,
            );
        } else if skipped > 0 {
            self.notify(
                "没有添加附件：文件不存在或超过 25 MB 限制",
                ToastKind::Error,
                cx,
            );
        }
        cx.notify();
    }

    pub fn remove_compose_attachment(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(compose) = self.compose.as_mut()
            && index < compose.attachments.len()
        {
            compose.attachments.remove(index);
            cx.notify();
        }
    }

    pub fn copy_current_body(&mut self, cx: &mut Context<Self>) {
        let Some(message) = self.current_message().cloned() else {
            return;
        };
        if message.body.trim().is_empty() {
            self.notify("这封邮件没有可复制的正文", ToastKind::Info, cx);
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(message.body));
        self.notify("正文已复制到剪贴板", ToastKind::Success, cx);
    }

    pub fn open_attachment_of(
        &mut self,
        message_id: i64,
        attachment_index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(message) = self
            .messages
            .iter()
            .chain(self.folder_messages.iter())
            .chain(self.threads.iter().flat_map(|thread| thread.messages.iter()))
            .find(|message| message.id == message_id)
            .cloned()
        else {
            return;
        };
        let Some(attachment) = message.attachments.get(attachment_index).cloned() else {
            return;
        };
        let Some(path) = attachment.path else {
            self.notify("附件尚未缓存，请先同步这封邮件", ToastKind::Info, cx);
            return;
        };
        if is_dangerous_attachment(&attachment.name) {
            self.notify(
                "出于安全考虑，不直接打开可执行文件。请先保存，确认来源后再手动打开。",
                ToastKind::Error,
                cx,
            );
            return;
        }
        cx.open_with_system(&path);
    }

    pub fn save_attachment(&mut self, attachment_index: usize, cx: &mut Context<Self>) {
        let Some(message) = self.current_message() else {
            return;
        };
        let message_id = message.id;
        self.save_attachment_of(message_id, attachment_index, cx);
    }

    pub fn save_attachment_of(
        &mut self,
        message_id: i64,
        attachment_index: usize,
        cx: &mut Context<Self>,
    ) {
        let message = self
            .messages
            .iter()
            .chain(self.folder_messages.iter())
            .chain(self.threads.iter().flat_map(|thread| thread.messages.iter()))
            .find(|message| message.id == message_id)
            .cloned();
        let Some(message) = message else {
            return;
        };
        let Some(attachment) = message.attachments.get(attachment_index).cloned() else {
            return;
        };
        let Some(source) = attachment.path.clone() else {
            self.notify(
                "附件尚未缓存（可能超过 8 MB 或同步未完成）",
                ToastKind::Error,
                cx,
            );
            return;
        };
        let downloads = std::env::var("USERPROFILE")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("Downloads");
        std::fs::create_dir_all(&downloads).ok();
        let target = downloads.join(format!(
            "{}-{}",
            message.id,
            safe_file_name(&attachment.name)
        ));
        match std::fs::copy(&source, &target) {
            Ok(_) => self.notify(
                format!("已保存到 {}", target.display()),
                ToastKind::Success,
                cx,
            ),
            Err(err) => self.notify(format!("保存附件失败：{err}"), ToastKind::Error, cx),
        }
    }

    pub fn request_remove_account(&mut self, cx: &mut Context<Self>) {
        let Some(account) = self.account().cloned() else {
            return;
        };
        self.confirm = Some(ConfirmAction::RemoveAccount {
            id: account.id,
            email: account.email.clone(),
        });
        cx.notify();
    }

    fn remove_account_now(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let _ = self.store.delete_account(id);
        self.accounts = self.store.accounts().unwrap_or_default();
        if let Some(next) = self.accounts.first().cloned() {
            self.active_account = next.id;
        }
        let _ = self
            .store
            .set_setting("active_account", &self.active_account.to_string());
        self.folders = self.store.folders(self.active_account).unwrap_or_default();
        self.selected_folder = "INBOX".to_string();
        self.settings = None;
        self.auth_help = None;
        if self.accounts.is_empty() {
            self.active_account = 0;
            self.welcome = true;
            self.begin_welcome(cx);
        } else {
            self.reload_messages(cx);
        }
        window.focus(&self.focus_handle);
    }

    pub fn open_settings(&mut self, cx: &mut Context<Self>) {
        if self.welcome {
            return;
        }
        if self.compose.is_some() {
            self.notify("请先完成或关闭正在写的邮件", ToastKind::Info, cx);
            return;
        }
        let account = self.account().cloned().unwrap_or_else(Account::empty);
        self.auth_help = None;
        let display_name = self.text_input(cx, "你的名字", &account.display_name);
        let email = self.text_input(cx, "you@example.com", &account.email);
        let password = self.text_input(cx, "邮箱授权码 / 应用专用密码", &account.password);
        let imap_host = self.text_input(cx, "imap.example.com", &account.imap_host);
        let imap_port = self.text_input(cx, "993", &account.imap_port.to_string());
        let smtp_host = self.text_input(cx, "smtp.example.com", &account.smtp_host);
        let smtp_port = self.text_input(cx, "465", &account.smtp_port.to_string());

        self.settings = Some(SettingsPanel {
            display_name,
            email,
            password,
            imap_host,
            imap_port,
            smtp_host,
            smtp_port,
            imap_starttls: account.imap_starttls,
            smtp_starttls: account.smtp_starttls,
            editing_id: account.id,
            auto_servers: true,
            error: None,
        });
        self.settings_error = None;
        self.watch_settings_email(cx);
        cx.notify();
    }

    fn watch_settings_email(&mut self, cx: &mut Context<Self>) {
        let Some(settings) = self.settings.as_ref() else {
            return;
        };
        let email = settings.email.clone();
        cx.observe(&email, |this, _input, cx| {
            this.auto_servers_if_needed(cx);
        })
        .detach();
    }

    fn auto_servers_if_needed(&mut self, cx: &mut Context<Self>) {
        let Some(settings) = self.settings.as_ref() else {
            return;
        };
        if !settings.auto_servers {
            return;
        }
        let email = settings.email.read(cx).value().to_string();
        let provider = AuthHelpProvider::detect(&email, "");
        if provider.servers_for(&email).is_some() {
            self.fill_servers(provider, &email, cx);
        }
    }

    pub fn open_auth_help(&mut self, cx: &mut Context<Self>) {
        let Some(settings) = self.settings.as_ref() else {
            return;
        };
        let email = settings.email.read(cx).value().to_string();
        let host = settings.imap_host.read(cx).value().to_string();
        self.auth_help = Some(AuthHelpProvider::detect(&email, &host));
        cx.notify();
    }

    pub fn close_auth_help(&mut self, cx: &mut Context<Self>) {
        self.auth_help = None;
        cx.notify();
    }

    pub fn select_auth_provider(&mut self, provider: AuthHelpProvider, cx: &mut Context<Self>) {
        self.auth_help = Some(provider);
        cx.notify();
    }

    pub fn open_auth_help_page(&mut self, cx: &mut Context<Self>) {
        let Some(url) = self.auth_help.and_then(|provider| provider.url()) else {
            self.notify("请参考邮箱服务商的官方帮助中心", ToastKind::Info, cx);
            return;
        };
        cx.open_url(url);
    }

    pub fn copy_auth_help_link(&mut self, cx: &mut Context<Self>) {
        let Some(url) = self.auth_help.and_then(|provider| provider.url()) else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(url.to_string()));
        self.notify("申请页面链接已复制", ToastKind::Success, cx);
    }

    fn fill_servers(
        &mut self,
        provider: AuthHelpProvider,
        email: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some((imap_host, imap_port, imap_tls, smtp_host, smtp_port, smtp_tls)) =
            provider.servers_for(email)
        else {
            return false;
        };
        let Some(settings) = self.settings.as_ref() else {
            return false;
        };
        let imap_host_input = settings.imap_host.clone();
        let imap_port_input = settings.imap_port.clone();
        let smtp_host_input = settings.smtp_host.clone();
        let smtp_port_input = settings.smtp_port.clone();
        imap_host_input.update(cx, |input, cx| input.set_value(imap_host, cx));
        imap_port_input.update(cx, |input, cx| input.set_value(imap_port.to_string(), cx));
        smtp_host_input.update(cx, |input, cx| input.set_value(smtp_host, cx));
        smtp_port_input.update(cx, |input, cx| input.set_value(smtp_port.to_string(), cx));
        if let Some(settings) = self.settings.as_mut() {
            settings.imap_starttls = imap_tls;
            settings.smtp_starttls = smtp_tls;
        }
        cx.notify();
        true
    }

    /// Paste an authorization code copied from the provider's website into the
    /// password field. This is the only clipboard read in the app and it is
    /// always triggered by an explicit click.
    pub fn paste_auth_code(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(password) = self.settings.as_ref().map(|settings| settings.password.clone()) else {
            return;
        };
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            self.notify("剪贴板里没有文本，请先在官网复制授权码", ToastKind::Error, cx);
            return;
        };
        let code = text.trim().replace(['\r', '\n'], "");
        if code.is_empty() {
            self.notify("剪贴板内容为空", ToastKind::Error, cx);
            return;
        }
        password.update(cx, |input, cx| input.set_value(code, cx));
        let handle = password.read(cx).handle();
        window.focus(&handle);
        self.auth_help = None;
        self.notify("授权码已粘贴，点击「保存并连接」生效", ToastKind::Success, cx);
        cx.notify();
    }

    pub fn save_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(settings) = self.settings.as_ref() else {
            return;
        };
        let display_name = settings.display_name.read(cx).value().trim().to_string();
        let email = settings.email.read(cx).value().trim().to_string();
        let password = settings.password.read(cx).value().to_string();
        let mut imap_host = settings.imap_host.read(cx).value().trim().to_string();
        let mut imap_port = settings
            .imap_port
            .read(cx)
            .value()
            .trim()
            .parse::<u16>()
            .unwrap_or(993);
        let mut smtp_host = settings.smtp_host.read(cx).value().trim().to_string();
        let mut smtp_port = settings
            .smtp_port
            .read(cx)
            .value()
            .trim()
            .parse::<u16>()
            .unwrap_or(465);
        let mut imap_starttls = settings.imap_starttls;
        let mut smtp_starttls = settings.smtp_starttls;
        let editing_id = settings.editing_id;

        if email.is_empty() {
            if let Some(settings) = self.settings.as_mut() {
                settings.error = Some("邮箱地址不能为空".to_string());
            }
            cx.notify();
            return;
        }
        if self.welcome && password.trim().is_empty() {
            if let Some(settings) = self.settings.as_mut() {
                settings.error = Some("请填写邮箱授权码".to_string());
            }
            cx.notify();
            return;
        }

        // Servers are derived from the address suffix; no server fields are
        // shown in the UI. Known providers always override stale values.
        let provider = AuthHelpProvider::detect(&email, "");
        if self.welcome && matches!(provider, AuthHelpProvider::Generic) {
            if let Some(settings) = self.settings.as_mut() {
                settings.error = Some(
                    "无法根据邮箱后缀识别服务器。目前支持 QQ、163、126、yeah.net、188、Gmail、Outlook、iCloud 邮箱。"
                        .to_string(),
                );
            }
            cx.notify();
            return;
        }
        let mut auto_filled = false;
        if let Some((imap, imap_p, imap_tls, smtp, smtp_p, smtp_tls)) = provider.servers_for(&email)
        {
            imap_host = imap.to_string();
            imap_port = imap_p;
            imap_starttls = imap_tls;
            smtp_host = smtp.to_string();
            smtp_port = smtp_p;
            smtp_starttls = smtp_tls;
            auto_filled = true;
        }
        if imap_host.is_empty() || smtp_host.is_empty() {
            if let Some(settings) = self.settings.as_mut() {
                settings.error = Some(
                    "无法根据邮箱后缀识别服务器。目前支持 QQ、163、126、yeah.net、188、Gmail、Outlook、iCloud 邮箱。"
                        .to_string(),
                );
            }
            cx.notify();
            return;
        }

        let account = Account {
            id: editing_id,
            display_name: display_name.clone(),
            email: email.clone(),
            imap_host: imap_host.clone(),
            imap_port,
            imap_starttls,
            smtp_host: smtp_host.clone(),
            smtp_port,
            smtp_starttls,
            password,
            is_demo: false,
        };
        match self.store.save_account(&account) {
            Ok(id) => {
                self.active_account = id;
                let _ = self.store.set_setting("active_account", &id.to_string());
                if self.store.folders(id).unwrap_or_default().is_empty() {
                    for kind in [
                        crate::models::FolderKind::Inbox,
                        crate::models::FolderKind::Starred,
                        crate::models::FolderKind::Sent,
                        crate::models::FolderKind::Drafts,
                        crate::models::FolderKind::Archive,
                        crate::models::FolderKind::Trash,
                    ] {
                        let _ = self.store.save_folder(&Folder {
                            account_id: id,
                            key: kind.key().to_string(),
                            name: kind.key().to_string(),
                            kind,
                            unread: 0,
                            total: 0,
                            server_name: None,
                            uid_validity: 0,
                            last_uid: 0,
                            oldest_uid: 0,
                        });
                    }
                }
                self.accounts = self.store.accounts().unwrap_or_default();
                self.folders = self.store.folders(id).unwrap_or_default();
                self.selected_folder = "INBOX".to_string();
                self.welcome = false;
                self.settings = None;
                self.auth_help = None;
                if auto_filled {
                    self.notify(
                        format!("账户设置已保存，已自动识别 {} 服务器", provider.label()),
                        ToastKind::Success,
                        cx,
                    );
                } else {
                    self.notify("账户设置已保存", ToastKind::Success, cx);
                }
                self.reload_messages(cx);
                window.focus(&self.focus_handle);
                self.sync(cx);
            }
            Err(err) => {
                if let Some(settings) = self.settings.as_mut() {
                    settings.error = Some(format!("保存失败：{err}"));
                }
                cx.notify();
            }
        }
    }

    pub fn add_account(&mut self, cx: &mut Context<Self>) {
        self.auth_help = None;
        let display_name = self.text_input(cx, "你的名字", "");
        let email = self.text_input(cx, "you@example.com", "");
        let password = self.text_input(cx, "邮箱授权码 / 应用专用密码", "");
        let imap_host = self.text_input(cx, "imap.example.com", "");
        let imap_port = self.text_input(cx, "993", "993");
        let smtp_host = self.text_input(cx, "smtp.example.com", "");
        let smtp_port = self.text_input(cx, "465", "465");
        self.settings = Some(SettingsPanel {
            display_name,
            email,
            password,
            imap_host,
            imap_port,
            smtp_host,
            smtp_port,
            imap_starttls: false,
            smtp_starttls: false,
            editing_id: 0,
            auto_servers: true,
            error: None,
        });
        self.watch_settings_email(cx);
        cx.notify();
    }

    pub fn sync(&mut self, cx: &mut Context<Self>) {
        self.sync_inner(true, cx);
    }

    fn sync_inner(&mut self, feedback: bool, cx: &mut Context<Self>) {
        if self.syncing {
            return;
        }
        let Some(account) = self.account().cloned() else {
            return;
        };
        if account.password.is_empty() {
            self.notify("请先在设置里填写授权码", ToastKind::Error, cx);
            return;
        }
        let states: Vec<mail::imap::FolderState> = self
            .folders
            .iter()
            .filter_map(|folder| {
                folder.server_name.clone()?;
                let cached_uids = self
                    .store
                    .uids(account.id, &folder.key)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(_, uid)| uid)
                    .collect();
                Some(mail::imap::FolderState {
                    key: folder.key.clone(),
                    uid_validity: folder.uid_validity,
                    last_uid: folder.last_uid,
                    oldest_uid: folder.oldest_uid,
                    cached_uids,
                })
            })
            .collect();
        let active_account = self.active_account;
        self.syncing = true;
        cx.notify();
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let account_for_fetch = account.clone();
            let db_path = store::db_path();
            let result = executor
                .spawn(async move {
                    let sync = mail::imap::sync_account(&account_for_fetch, &states, 200)?;
                    let store = Store::open(&db_path)?;
                    let mut added = 0usize;
                    let mut removed = 0usize;
                    let mut skipped = 0usize;
                    let mut total = 0usize;
                    for folder in &sync.folders {
                        total += folder.total;
                        let kind = FolderKind::from_key(&folder.key);
                        store.save_folder(&Folder {
                            account_id: account_for_fetch.id,
                            key: folder.key.clone(),
                            name: kind.label().to_string(),
                            kind,
                            unread: 0,
                            total: 0,
                            server_name: Some(folder.server_name.clone()),
                            uid_validity: folder.uid_validity,
                            last_uid: folder.last_uid,
                            oldest_uid: folder.oldest_uid,
                        })?;
                        if folder.reset {
                            store.delete_folder_messages(account_for_fetch.id, &folder.key)?;
                        }
                        let local = store.uids(account_for_fetch.id, &folder.key)?;
                        let local_by_uid: HashMap<u32, i64> =
                            local.iter().map(|(id, uid)| (*uid, *id)).collect();
                        for (uid, read, starred) in &folder.flags {
                            if let Some(id) = local_by_uid.get(uid) {
                                let _ = store.set_read(*id, *read);
                                let _ = store.set_starred(*id, *starred);
                            }
                        }
                        let server_set: HashSet<u32> = folder.server_uids.iter().copied().collect();
                        for (id, uid) in &local {
                            if !server_set.contains(uid) && store.delete_message(*id).is_ok() {
                                removed += 1;
                            }
                        }
                        for message in &folder.messages {
                            if let Some(existing) = store.folder_for_message_id(
                                account_for_fetch.id,
                                &message.message_id,
                            ) {
                                if existing != folder.key {
                                    skipped += 1;
                                    continue;
                                }
                                // Server-side move or IMAP APPEND gave the same
                                // message a new UID; keep one local row and adopt it.
                                if !message.message_id.is_empty()
                                    && let Some((id, old_uid)) = store
                                        .message_ref_by_message_id(
                                            account_for_fetch.id,
                                            &message.message_id,
                                        )
                                    && old_uid != message.uid
                                {
                                    let _ = store.set_uid(id, message.uid);
                                }
                            }
                            if store.upsert(message).is_ok() {
                                added += 1;
                            }
                        }
                    }
                    Ok::<_, anyhow::Error>((added, removed, skipped, total))
                })
                .await;
            this.update(cx, |this, cx| {
                this.syncing = false;
                match result {
                    Ok((added, removed, skipped, total)) => {
                        let selected_id = this.current_message().map(|message| message.id);
                        let selected_thread = this
                            .current_thread()
                            .map(|thread| thread.key.clone());
                        let list_offset = this.list_scroll.offset();
                        let reader_offset = this.reader_scroll.offset();
                        this.folders = this.store.folders(active_account).unwrap_or_default();
                        this.refresh_folders();
                        this.reload_messages(cx);
                        if let Some(id) = selected_id
                            && let Some(index) =
                                this.messages.iter().position(|message| message.id == id)
                        {
                            this.selected = Some(index);
                        }
                        if let Some(key) = selected_thread
                            && let Some(index) =
                                this.threads.iter().position(|thread| thread.key == key)
                        {
                            this.selected_thread = Some(index);
                        }
                        this.list_scroll.set_offset(list_offset);
                        this.reader_scroll.set_offset(reader_offset);
                        let _ = this
                            .store
                            .set_setting("last_sync", &now_ts().to_string());
                        if feedback || added > 0 || removed > 0 {
                            let mut text =
                                format!("同步完成：服务器共 {} 封，更新 {} 封", total, added);
                            if removed > 0 {
                                text.push_str(&format!("，移除 {} 封", removed));
                            }
                            if skipped > 0 {
                                text.push_str(&format!("，保留本地整理 {}", skipped));
                            }
                            this.notify(text, ToastKind::Success, cx);
                        }
                        if !feedback && added > 0 {
                            let body = format!("收到 {added} 封新邮件，点击托盘图标查看");
                            // Shell_NotifyIcon can dispatch messages; keep it out
                            // of the entity update to avoid re-entering GPUI.
                            std::thread::spawn(move || crate::tray::notify("mail", &body));
                        }
                    }
                    Err(err) => {
                        if feedback {
                            this.notify(format!("同步失败：{err}"), ToastKind::Error, cx);
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }


}


const PAGE_SIZE: usize = 150;

fn input_colors(theme: &Theme) -> TextColors {
    TextColors {
        caret: theme.accent.into(),
        selection: Hsla::from(theme.accent).opacity(0.24),
        placeholder: theme.text_muted.into(),
    }
}

fn folder_label(key: &str) -> String {
    match key {
        "INBOX" => "收件箱",
        "STARRED" => "星标邮件",
        "SENT" => "已发送",
        "DRAFTS" => "草稿箱",
        "ARCHIVE" => "归档",
        "TRASH" => "已删除",
        "JUNK" => "垃圾邮件",
        other => other,
    }
    .to_string()
}

fn safe_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || matches!(ch, '.' | '-' | '_' | ' ') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches(['.', ' ']).to_string();
    if cleaned.is_empty() {
        "attachment".to_string()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod provider_tests {
    use super::AuthHelpProvider;

    #[test]
    fn detects_common_mail_providers() {
        assert_eq!(
            AuthHelpProvider::detect("someone@163.com", ""),
            AuthHelpProvider::NetEase
        );
        assert_eq!(
            AuthHelpProvider::detect("someone@qq.com", ""),
            AuthHelpProvider::Qq
        );
        assert_eq!(
            AuthHelpProvider::detect("someone@gmail.com", ""),
            AuthHelpProvider::Gmail
        );
        assert_eq!(
            AuthHelpProvider::detect("someone@outlook.com", ""),
            AuthHelpProvider::Outlook
        );
        assert_eq!(
            AuthHelpProvider::detect("someone@icloud.com", ""),
            AuthHelpProvider::ICloud
        );
        assert_eq!(
            AuthHelpProvider::detect("someone@example.com", ""),
            AuthHelpProvider::Generic
        );
        assert_eq!(
            AuthHelpProvider::detect("someone@live.com", ""),
            AuthHelpProvider::Outlook
        );
        assert_eq!(
            AuthHelpProvider::detect("someone@olive.com", ""),
            AuthHelpProvider::Generic
        );
        assert_eq!(
            AuthHelpProvider::detect("someone@163.com", "imap.163.com"),
            AuthHelpProvider::NetEase
        );
        assert!(AuthHelpProvider::NetEase.url().is_some());
        assert!(AuthHelpProvider::Generic.url().is_none());
        assert_eq!(
            AuthHelpProvider::NetEase.servers().map(|servers| servers.0),
            Some("imap.163.com")
        );
        assert_eq!(
            AuthHelpProvider::Outlook
                .servers()
                .map(|servers| (servers.3, servers.4, servers.5)),
            Some(("smtp.office365.com", 587, true))
        );
        assert!(AuthHelpProvider::Generic.servers().is_none());
        assert_eq!(
            AuthHelpProvider::NetEase
                .servers_for("someone@126.com")
                .map(|servers| servers.0),
            Some("imap.126.com")
        );
        assert_eq!(
            AuthHelpProvider::Qq
                .servers_for("someone@exmail.qq.com")
                .map(|servers| servers.0),
            Some("imap.exmail.qq.com")
        );
        assert!(
            AuthHelpProvider::NetEase
                .url()
                .is_some_and(|url| url.contains("286624f309a1a7089"))
        );
    }
}

fn new_message_id(account_id: i64) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("<mail-{account_id}-{nanos}@mail.app>")
}

fn is_dangerous_attachment(name: &str) -> bool {
    let extension = name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(
        extension.as_str(),
        "exe" | "msi" | "bat" | "cmd" | "com" | "scr" | "ps1" | "vbs" | "js" | "jse"
            | "wsf" | "wsh" | "lnk" | "reg" | "hta" | "pif" | "cpl" | "jar"
    )
}

fn guess_mime(name: &str) -> String {
    let extension = name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let mime = match extension.as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "txt" | "log" => "text/plain",
        "md" => "text/markdown",
        "csv" => "text/csv",
        "json" => "application/json",
        "html" | "htm" => "text/html",
        "zip" => "application/zip",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        _ => "application/octet-stream",
    };
    mime.to_string()
}

fn draft_fingerprint(
    to: &str,
    cc: &str,
    bcc: &str,
    subject: &str,
    body: &str,
    attachments: &[Attachment],
) -> String {
    let mut out = format!("{to}\u{1f}{cc}\u{1f}{bcc}\u{1f}{subject}\u{1f}{body}");
    for attachment in attachments {
        out.push('\u{1f}');
        out.push_str(&attachment.name);
        if let Some(path) = &attachment.path {
            out.push(':');
            out.push_str(&path.to_string_lossy());
        }
    }
    out
}

fn split_addresses(raw: &str) -> Vec<String> {
    raw.split([',', ';'])
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .map(|part| part.to_string())
        .collect()
}

fn format_addrs(addrs: &[String]) -> String {
    addrs.join(", ")
}

fn quoted_body(message: &Message) -> String {
    let mut quoted = String::new();
    quoted.push_str("\n\n");
    quoted.push_str(&format!(
        "在 {}，{} 写道：\n",
        message.full_date_label(),
        message.sender_label()
    ));
    for line in message.body.lines().take(40) {
        quoted.push_str("> ");
        quoted.push_str(line);
        quoted.push('\n');
    }
    quoted
}


pub fn toast_colors(kind: ToastKind, theme: &Theme) -> (Rgba, &'static str) {
    match kind {
        ToastKind::Info => (theme.accent, "sparkles"),
        ToastKind::Success => (theme.success, "check"),
        ToastKind::Error => (theme.danger, "close"),
    }
}
































