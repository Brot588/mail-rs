use gpui::prelude::*;
use gpui::{
    div, px, relative, AnyElement, ClickEvent, Context, Div, Entity, FontWeight, Hsla, IntoElement,
    MouseButton, Render, SharedString, Window, WindowControlArea,
};
use crate::ui::input::TextInput;

use crate::app::{
    toast_colors, ArchiveMail, AuthHelpProvider, ComposeMode, ConfirmAction, DeleteMail, Dismiss,
    FocusSearch, ForwardMail, NewMail, NextMail, OpenSettings, PrevMail, ReplyAllMail, ReplyMail,
    Root, SendMail, SyncMail, ToggleGrouping, ToggleStar, ToggleThemeAction, ToggleUnread,
    TranslateMail, SIDEBAR_WIDTH,
};
use crate::models::{Message, Thread};
use crate::theme::Theme;
use crate::ui::widgets::{
    avatar, horizontal_divider, icon, icon_button, keycap, logo, pill, section_label,
    vertical_divider,
};

impl Root {
    // -----------------------------------------------------------------
    // Title bar
    // -----------------------------------------------------------------

    fn render_titlebar(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let search = self.search.clone();
        let search_focus = search.clone();
        let theme_icon = if theme.dark { "sun" } else { "moon" };

        div()
            .flex_none()
            .h(px(52.))
            .w_full()
            .flex()
            .items_center()
            .bg(theme.bg)
            .border_b_1()
            .border_color(theme.border_soft)
            .child(
                div()
                    .flex_none()
                    .w(px(220.))
                    .h_full()
                    .flex()
                    .items_center()
                    .gap_2()
                    .pl(px(18.))
                    .window_control_area(WindowControlArea::Drag)
                    .child(logo(26.))
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child("mail"),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .w(px(400.))
                            .h(px(32.))
                            .flex()
                            .items_center()
                            .gap_2()
                            .px(px(10.))
                            .rounded_lg()
                            .bg(theme.card)
                            .border_1()
                            .border_color(theme.border_soft)
                            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                window.focus(&search_focus.read(cx).handle());
                            })
                            .child(icon("search", 14., theme.text_muted))
                            .child(div().flex_1().child(search))
                            .child(keycap("Ctrl F", &theme)),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px(px(10.))
                    .child(
                        icon_button(
                            SharedString::from("theme-toggle"),
                            theme_icon,
                            16.,
                            theme.text_soft,
                            theme.card,
                            30.,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                            this.toggle_theme(cx);
                        })),
                    )
                    .child(
                        icon_button(
                            SharedString::from("open-settings"),
                            "settings",
                            16.,
                            theme.text_soft,
                            theme.card,
                            30.,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                            this.open_settings(cx);
                        })),
                    )
                    .child(vertical_divider(&theme, 18.))
                    .child(window_button(
                        "win-min",
                        "minimize",
                        12.,
                        theme.card,
                        WindowControlArea::Min,
                        &theme,
                    ))
                    .child(window_button(
                        "win-max",
                        "maximize",
                        11.,
                        theme.card,
                        WindowControlArea::Max,
                        &theme,
                    ))
                    .child(
                        div()
                            .id("win-close")
                            .w(px(46.))
                            .h(px(52.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|style| style.bg(gpui::rgb(0xD94A4A)))
                            .window_control_area(WindowControlArea::Close)
                            .child(icon("close", 12., theme.text_muted)),
                    ),
            )
            .into_any_element()
    }

    // -----------------------------------------------------------------
    // Sidebar
    // -----------------------------------------------------------------

    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let account = self
            .account()
            .cloned()
            .unwrap_or_else(crate::models::Account::empty);
        let entity = cx.entity();

        let mut items: Vec<AnyElement> = Vec::new();
        for folder in self.folders.clone() {
            let selected = folder.key == self.selected_folder;
            let key = folder.key.clone();
            let click_entity = entity.clone();
            let unread = folder.unread;
            items.push(
                div()
                    .id(SharedString::from(format!("folder-{}", folder.key)))
                    .h(px(33.))
                    .w_full()
                    .px(px(9.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded_md()
                    .cursor_pointer()
                    .text_color(if selected {
                        theme.text
                    } else {
                        theme.text_soft
                    })
                    .when(selected, |this| this.bg(theme.selected_row))
                    .when(!selected, |this| {
                        this.hover(move |style| style.bg(theme.card))
                    })
                    .on_click(move |_, _, cx| {
                        click_entity.update(cx, |this, cx| {
                            this.select_folder(key.clone(), cx);
                        });
                    })
                    .child(icon(
                        folder.kind.icon(),
                        15.,
                        if selected {
                            theme.accent
                        } else {
                            theme.text_muted
                        },
                    ))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(12.8))
                            .when(selected, |this| this.font_weight(FontWeight::MEDIUM))
                            .child(folder.label().to_string()),
                    )
                    .when(unread > 0, |this| {
                        this.child(pill(
                            unread.to_string(),
                            if selected {
                                theme.accent
                            } else {
                                theme.border
                            },
                            if selected {
                                Hsla::from(gpui::white())
                            } else {
                                Hsla::from(theme.text_soft)
                            },
                        ))
                    })
                    .into_any_element(),
            );
        }

        let mut account_rows: Vec<AnyElement> = Vec::new();
        for account_item in self.accounts.clone() {
            let active = account_item.id == self.active_account;
            let id = account_item.id;
            let switch_entity = entity.clone();
            account_rows.push(
                div()
                    .id(SharedString::from(format!("account-{id}")))
                    .h(px(30.))
                    .px(px(9.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded_md()
                    .cursor_pointer()
                    .when(active, |this| this.bg(theme.selected_row))
                    .when(!active, |this| {
                        this.hover(move |style| style.bg(theme.card))
                    })
                    .on_click(move |_, _, cx| {
                        switch_entity.update(cx, |this, cx| this.switch_account(id, cx));
                    })
                    .child(avatar(
                        &account_item.email,
                        &account_item.avatar_text(),
                        18.,
                        8.5,
                        &theme,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_size(px(11.5))
                            .text_color(if active {
                                theme.text
                            } else {
                                theme.text_muted
                            })
                            .child(account_item.display_name.clone()),
                    )
                    .when(active, |this| this.child(icon("check", 12., theme.accent)))
                    .into_any_element(),
            );
        }

        let compose_entity = entity.clone();
        let settings_entity = entity.clone();
        let add_account_entity = entity.clone();
        let sync_entity = entity.clone();
        let syncing = self.syncing;
        let account_count = self.accounts.len();
        let total_unread: usize = self
            .folders
            .iter()
            .filter(|folder| folder.kind == crate::models::FolderKind::Inbox)
            .map(|folder| folder.unread)
            .sum();

        div()
            .flex_none()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.sidebar)
            .border_r_1()
            .border_color(theme.border_soft)
            .child(
                div()
                    .id("account-card")
                    .mx(px(10.))
                    .mt(px(12.))
                    .p(px(10.))
                    .rounded_lg()
                    .bg(theme.card)
                    .border_1()
                    .border_color(theme.border_soft)
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.card_hover))
                    .on_click(move |_, _, cx| {
                        settings_entity.update(cx, |this, cx| this.open_settings(cx));
                    })
                    .child(avatar(
                        &account.email,
                        &account.avatar_text(),
                        30.,
                        12.5,
                        &theme,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text)
                                    .truncate()
                                    .child(account.display_name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(theme.text_muted)
                                    .truncate()
                                    .child(account.email.clone()),
                            ),
                    )
            )
            .child(
                div()
                    .px(px(10.))
                    .pt(px(12.))
                    .child(
                        div()
                            .id("compose-button")
                            .h(px(36.))
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .rounded_lg()
                            .bg(theme.accent)
                            .text_color(gpui::white())
                            .text_size(px(12.8))
                            .font_weight(FontWeight::SEMIBOLD)
                            .cursor_pointer()
                            .hover(move |style| style.bg(theme.accent_hover))
                            .on_click(move |_, window, cx| {
                                compose_entity.update(cx, |this, cx| {
                                    this.open_compose(ComposeMode::New, window, cx);
                                });
                            })
                            .child(icon("compose", 15., gpui::white()))
                            .child("写邮件")
                            .child(
                                div()
                                    .ml(px(2.))
                                    .text_size(px(9.5))
                                    .text_color(gpui::rgba(0xFFFFFFB0))
                                    .child("Ctrl N"),
                            ),
                    ),
            )
            .child(
                div()
                    .id("sidebar-scroll")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .px(px(10.))
                    .child(section_label("邮箱", &theme))
                    .children(items)
                    .child(div().mt(px(8.)).child(horizontal_divider(&theme)))
                    .child(section_label("账户", &theme))
                    .children(account_rows)
                    .child(
                        div()
                            .id("add-account")
                            .h(px(30.))
                            .px(px(9.))
                            .flex()
                            .items_center()
                            .gap_2()
                            .rounded_md()
                            .cursor_pointer()
                            .text_color(theme.text_muted)
                            .text_size(px(12.2))
                            .hover(|style| style.bg(theme.card))
                            .on_click(move |_, _, cx| {
                                add_account_entity.update(cx, |this, cx| this.add_account(cx));
                            })
                            .child(icon("plus", 14., theme.text_muted))
                            .child(format!("添加账户 · 共 {account_count} 个")),
                    ),
            )
            .child(horizontal_divider(&theme))
            .child(
                div()
                    .flex_none()
                    .px(px(14.))
                    .py(px(10.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(if syncing {
                        icon("refresh", 13., theme.accent)
                    } else {
                        icon("cloud", 13., theme.success)
                    })
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(10.5))
                            .text_color(theme.text_muted)
                            .child(if syncing {
                                "正在同步…".to_string()
                            } else {
                                format!("已就绪 · {total_unread} 封未读")
                            }),
                    )
                    .child(
                        div()
                            .id("sync-now")
                            .size(px(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|style| style.bg(theme.card))
                            .on_click(move |_, _, cx| {
                                sync_entity.update(cx, |this, cx| this.sync(cx));
                            })
                            .child(icon("refresh", 13., theme.text_muted)),
                    ),
            )
            .into_any_element()
    }

    // -----------------------------------------------------------------
    // Message list
    // -----------------------------------------------------------------

    fn render_message_row(
        &self,
        index: usize,
        message: &Message,
        entity: Entity<Root>,
        theme: Theme,
    ) -> AnyElement {
        let selected = self.selected == Some(index);
        let click_entity = entity.clone();
        let is_starred = message.is_starred;

        div()
            .id(SharedString::from(format!("message-{}", message.id)))
            .relative()
            .w_full()
            .h(px(90.))
            .px(px(16.))
            .py(px(12.))
            .flex()
            .gap(px(12.))
            .cursor_pointer()
            .border_b_1()
            .border_color(theme.border_soft)
            .when(selected, |this| this.bg(theme.selected_row))
            .when(!selected, |this| {
                this.hover(move |style| style.bg(theme.card))
            })
            .on_click(move |_, window, cx| {
                click_entity.update(cx, |this, cx| {
                    this.select_message(index, window, cx);
                });
            })
            .when(selected, |this| {
                this.child(
                    div()
                        .absolute()
                        .left(px(0.))
                        .top(px(0.))
                        .bottom(px(0.))
                        .w(px(3.))
                        .bg(theme.accent),
                )
            })
            .child(avatar(
                &message.from_email,
                &message.sender_label(),
                38.,
                15.,
                &theme,
            ))
            .child(
                div()
                    .w(px(272.))
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .text_color(if message.is_read {
                                        theme.text_soft
                                    } else {
                                        theme.text
                                    })
                                    .font_weight(if message.is_read {
                                        FontWeight::NORMAL
                                    } else {
                                        FontWeight::SEMIBOLD
                                    })
                                    .truncate()
                                    .child(crate::models::ellipsize(&message.sender_label(), 12.5, 158.)),
                            )
                            .when(!message.is_read, |this| {
                                this.child(
                                    div()
                                        .flex_none()
                                        .size(px(6.))
                                        .rounded_full()
                                        .bg(theme.accent),
                                )
                            })
                            .child(div().flex_1())
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(px(10.5))
                                    .text_color(theme.text_muted)
                                    .child(message.time_label()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .text_size(px(12.5))
                                    .truncate()
                                    .text_color(if message.is_read {
                                        theme.text_soft
                                    } else {
                                        theme.text
                                    })
                                    .font_weight(if message.is_read {
                                        FontWeight::NORMAL
                                    } else {
                                        FontWeight::MEDIUM
                                    })
                                    .child(crate::models::ellipsize(message.subject_or_placeholder(), 12.5, 226.)),
                            )
                            .when(message.has_attachments(), |this| {
                                this.child(icon("paperclip", 12., theme.text_muted))
                            })
                            .when(is_starred, |this| {
                                this.child(icon("star", 12., theme.star))
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(theme.text_muted)
                            .truncate()
                            .child(crate::models::ellipsize(&message.preview, 11.5, 250.)),
                    ),
            )
            .into_any_element()
    }

    fn render_thread_row(
        &self,
        index: usize,
        thread: &Thread,
        entity: Entity<Root>,
        theme: Theme,
    ) -> AnyElement {
        let selected = self.selected_thread == Some(index);
        let latest = thread.latest();
        let unread = thread.unread();
        let count = thread.count();
        let click_entity = entity.clone();

        div()
            .id(SharedString::from(format!("thread-{}", thread.key)))
            .relative()
            .w_full()
            .h(px(90.))
            .px(px(16.))
            .py(px(12.))
            .flex()
            .gap(px(12.))
            .cursor_pointer()
            .border_b_1()
            .border_color(theme.border_soft)
            .when(selected, |this| this.bg(theme.selected_row))
            .when(!selected, |this| {
                this.hover(move |style| style.bg(theme.card))
            })
            .on_click(move |_, window, cx| {
                click_entity.update(cx, |this, cx| {
                    this.select_thread(index, window, cx);
                });
            })
            .when(selected, |this| {
                this.child(
                    div()
                        .absolute()
                        .left(px(0.))
                        .top(px(0.))
                        .bottom(px(0.))
                        .w(px(3.))
                        .bg(theme.accent),
                )
            })
            .child(avatar(
                &latest.from_email,
                &latest.sender_label(),
                38.,
                15.,
                &theme,
            ))
            .child(
                div()
                    .w(px(272.))
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .text_color(if unread > 0 {
                                        theme.text
                                    } else {
                                        theme.text_soft
                                    })
                                    .font_weight(if unread > 0 {
                                        FontWeight::SEMIBOLD
                                    } else {
                                        FontWeight::NORMAL
                                    })
                                    .truncate()
                                    .child(crate::models::ellipsize(
                                        &latest.sender_label(),
                                        12.5,
                                        112.,
                                    )),
                            )
                            .when(unread > 0, |this| {
                                this.child(
                                    div()
                                        .flex_none()
                                        .size(px(6.))
                                        .rounded_full()
                                        .bg(theme.accent),
                                )
                            })
                            .when(count > 1, |this| {
                                this.child(pill(
                                    format!("{count} 封"),
                                    if unread > 0 {
                                        theme.accent_soft
                                    } else {
                                        theme.card
                                    },
                                    if unread > 0 {
                                        theme.accent
                                    } else {
                                        theme.text_muted
                                    },
                                ))
                            })
                            .child(div().flex_1())
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(px(10.5))
                                    .text_color(theme.text_muted)
                                    .child(latest.time_label()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .text_size(px(12.5))
                                    .truncate()
                                    .text_color(if unread > 0 {
                                        theme.text
                                    } else {
                                        theme.text_soft
                                    })
                                    .font_weight(if unread > 0 {
                                        FontWeight::MEDIUM
                                    } else {
                                        FontWeight::NORMAL
                                    })
                                    .child(crate::models::ellipsize(
                                        thread.subject(),
                                        12.5,
                                        214.,
                                    )),
                            )
                            .when(thread.has_attachments(), |this| {
                                this.child(icon("paperclip", 12., theme.text_muted))
                            })
                            .when(thread.starred(), |this| {
                                this.child(icon("star", 12., theme.star))
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(theme.text_muted)
                            .truncate()
                            .child(crate::models::ellipsize(&latest.preview, 11.5, 250.)),
                    ),
            )
            .into_any_element()
    }

    fn render_list(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let entity = cx.entity();
        let grouped = self.grouped;
        let rows: Vec<AnyElement> = if grouped {
            self.threads
                .iter()
                .enumerate()
                .map(|(index, thread)| {
                    self.render_thread_row(index, thread, entity.clone(), theme)
                })
                .collect()
        } else {
            self.messages
                .iter()
                .enumerate()
                .map(|(index, message)| {
                    self.render_message_row(index, message, entity.clone(), theme)
                })
                .collect()
        };
        let label = if self.search_all && !self.query.is_empty() {
            "搜索结果".to_string()
        } else {
            self.selected_folder_label()
        };
        let query = self.query.clone();
        let search_all = self.search_all;
        let empty = rows.is_empty();
        let count = self.messages.len();
        let thread_count = self.threads.len();
        let unread = self.messages.iter().filter(|message| !message.is_read).count();
        let compose_entity = entity.clone();
        let grouping_color = if grouped {
            theme.accent
        } else {
            theme.text_muted
        };
        let grouping_bg = if grouped {
            theme.accent_soft
        } else {
            theme.surface
        };

        div()
            .flex_none()
            .w(px(368.))
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.surface)
            .border_r_1()
            .border_color(theme.border_soft)
            .child(
                div()
                    .flex_none()
                    .h(px(52.))
                    .px(px(18.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border_soft)
                    .child(
                        div()
                            .text_size(px(14.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child(label),
                    )
                    .child(
                        div()
                            .text_size(px(10.5))
                            .text_color(theme.text_muted)
                            .child(if grouped && !query.is_empty() {
                                format!("搜索 · {thread_count} 位联系人")
                            } else if grouped {
                                format!("{thread_count} 位联系人")
                            } else if query.is_empty() {
                                format!("{count} 封")
                            } else {
                                format!("搜索 · {count}")
                            }),
                    )
                    .child(div().flex_1())
                    .when(!query.is_empty(), |this| {
                        this.child(
                            div()
                                .id("search-scope")
                                .h(px(24.))
                                .px(px(7.))
                                .flex()
                                .items_center()
                                .rounded_md()
                                .bg(if search_all {
                                    theme.accent_soft
                                } else {
                                    theme.card
                                })
                                .text_size(px(9.5))
                                .text_color(if search_all {
                                    theme.accent
                                } else {
                                    theme.text_muted
                                })
                                .cursor_pointer()
                                .hover(|style| style.bg(theme.card_hover))
                                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                    this.toggle_search_scope(cx);
                                }))
                                .child(if search_all {
                                    "全部邮件"
                                } else {
                                    "当前文件夹"
                                }),
                        )
                    })
                    .child(
                        div()
                            .id("toggle-grouping")
                            .h(px(26.))
                            .px(px(7.))
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .rounded_md()
                            .bg(grouping_bg)
                            .cursor_pointer()
                            .hover(move |style| style.bg(theme.card))
                            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                this.toggle_grouping(cx);
                            }))
                            .child(icon("layers", 13., grouping_color))
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(grouping_color)
                                    .child("聚合"),
                            ),
                    )
                    .when(unread > 0 && query.is_empty(), |this| {
                        this.child(
                            icon_button(
                                SharedString::from("mark-all-read"),
                                "check",
                                14.,
                                theme.text_muted,
                                theme.card,
                                26.,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                this.mark_all_read(cx);
                            })),
                        )
                    })
            )
            .child(
                div()
                    .id("message-list")
                    .flex_1()
                    .min_h(px(0.))
                    .flex()
                    .flex_col()
                    .when(empty, |this| {
                        this.items_center().justify_center()
                    })
                    .overflow_y_scroll()
                    .track_scroll(&self.list_scroll)
                    .when(empty, |this| {
                        this.child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(px(12.))
                                .child(icon("mail", 30., theme.border))
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(theme.text_muted)
                                        .child(if grouped && !query.is_empty() {
                                            "没有匹配的联系人"
                                        } else if query.is_empty() {
                                            "这个文件夹里没有邮件"
                                        } else {
                                            "没有匹配的邮件"
                                        }),
                                )
                                .when(query.is_empty(), |this| {
                                    this.child(
                                        div()
                                            .id("empty-compose")
                                            .px(px(12.))
                                            .h(px(28.))
                                            .flex()
                                            .items_center()
                                            .rounded_md()
                                            .bg(theme.accent_soft)
                                            .text_color(theme.accent)
                                            .text_size(px(11.5))
                                            .cursor_pointer()
                                            .on_click(move |_, window, cx| {
                                                compose_entity.update(cx, |this, cx| {
                                                    this.open_compose(
                                                        ComposeMode::New,
                                                        window,
                                                        cx,
                                                    );
                                                });
                                            })
                                            .child("写一封"),
                                    )
                                }),
                        )
                    })
                    .children(rows)
                    .when(!self.has_more && query.is_empty() && self.folder(&self.selected_folder).is_some_and(|folder| folder.server_name.is_some() && !folder.uid_validity.eq(&0)), |this| {
                        this.child(
                            div()
                                .id("load-older-from-server")
                                .h(px(40.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .px(px(12.))
                                .text_size(px(10.5))
                                .text_color(if self.older_loading { theme.text_muted } else { theme.accent })
                                .cursor_pointer()
                                .hover(|style| style.bg(theme.card))
                                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                    this.load_older_from_server(cx);
                                }))
                                .child(if self.older_loading {
                                    "正在从服务器加载…"
                                } else {
                                    "从服务器加载更早邮件"
                                }),
                        )
                    })
                    .when(self.has_more && query.is_empty(), |this| {
                        this.child(
                            div()
                                .id("load-more-messages")
                                .h(px(40.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .px(px(12.))
                                .text_size(px(10.5))
                                .text_color(theme.accent)
                                .cursor_pointer()
                                .hover(|style| style.bg(theme.card))
                                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                    this.load_more_messages(cx);
                                }))
                                .child("加载更早的邮件"),
                        )
                    }),
            )
            .into_any_element()
    }

    // -----------------------------------------------------------------
    // Reader
    // -----------------------------------------------------------------

    fn render_reader(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        if self.grouped {
            if let Some(thread) = self.current_thread().cloned() {
                if thread.count() > 1 {
                    return self.render_thread(&thread, cx);
                }
            }
        }
        let Some(message) = self.current_message().cloned() else {
            return self.render_empty_reader(theme);
        };
        let entity = cx.entity();
        let draft_entity = entity.clone();
        let draft_message_id = message.id;
        let is_draft = message.is_draft;
        let star_color = if message.is_starred {
            theme.star
        } else {
            theme.text_soft
        };
        let unread_color = if message.is_read {
            theme.text_soft
        } else {
            theme.accent
        };
        let removed = matches!(self.selected_folder.as_str(), "ARCHIVE" | "TRASH");

        let attachments: Vec<AnyElement> = message
            .attachments
            .iter()
            .enumerate()
            .map(|(index, attachment)| {
                let save_entity = entity.clone();
                div()
                    .id(SharedString::from(format!("attachment-{}", index)))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px(px(10.))
                    .py(px(7.))
                    .rounded_md()
                    .bg(theme.card)
                    .border_1()
                    .border_color(theme.border_soft)
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.card_hover))
                    .on_click(move |_, _, cx| {
                        save_entity.update(cx, |this, cx| {
                            this.save_attachment(index, cx);
                        });
                    })
                    .child(icon("file", 14., theme.accent))
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(theme.text_soft)
                            .child(attachment.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(theme.text_muted)
                            .child(attachment.size_label()),
                    )
                    .child(icon("download", 12., theme.text_muted))
                    .child({
                        let open_entity = entity.clone();
                        div()
                            .id(SharedString::from(format!("attachment-open-{index}")))
                            .size(px(18.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_sm()
                            .cursor_pointer()
                            .hover(|style| style.bg(theme.card_hover))
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                open_entity.update(cx, |this, cx| {
                                    this.open_attachment_of(message.id, index, cx);
                                });
                            })
                            .child(icon("chevron_right", 11., theme.text_soft))
                    })
                    .into_any_element()
            })
            .collect();
        let has_attachments = !attachments.is_empty();

        let paragraphs: Vec<AnyElement> = message
            .body
            .split('\n')
            .map(|line| {
                div()
                    .w_full()
                    .min_h(px(6.))
                    .text_size(px(13.8))
                    .line_height(px(24.))
                    .text_color(theme.text_soft)
                    .child(line.to_string())
                    .into_any_element()
            })
            .collect();

        let reply_entity = entity.clone();
        let forward_entity = entity.clone();

        div()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.reader)
            .child(
                div()
                    .flex_none()
                    .h(px(52.))
                    .px(px(16.))
                    .flex()
                    .items_center()
                    .gap_1()
                    .border_b_1()
                    .border_color(theme.border_soft)
                    .child(if removed {
                        icon_button(
                            SharedString::from("reader-restore"),
                            "inbox",
                            15.,
                            theme.text_soft,
                            theme.card,
                            30.,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                            this.restore_current(cx);
                        }))
                        .into_any_element()
                    } else {
                        icon_button(
                            SharedString::from("reader-archive"),
                            "archive",
                            15.,
                            theme.text_soft,
                            theme.card,
                            30.,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                            this.archive_current(cx);
                        }))
                        .into_any_element()
                    })
                    .child(
                        icon_button(
                            SharedString::from("reader-delete"),
                            "trash",
                            15.,
                            theme.text_soft,
                            theme.card,
                            30.,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                            this.delete_current(cx);
                        })),
                    )
                    .child(
                        icon_button(
                            SharedString::from("reader-unread"),
                            "mail",
                            15.,
                            unread_color,
                            theme.card,
                            30.,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                            this.toggle_unread(cx);
                        })),
                    )
                    .child(
                        icon_button(
                            SharedString::from("reader-star"),
                            "star",
                            15.,
                            star_color,
                            theme.card,
                            30.,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                            this.toggle_star(cx);
                        })),
                    )
                    .child(
                        icon_button(
                            SharedString::from("reader-translate"),
                            "languages",
                            15.,
                            if self.translation.visible {
                                theme.accent
                            } else {
                                theme.text_soft
                            },
                            theme.card,
                            30.,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                            this.toggle_translation(cx);
                        })),
                    )
                    .child(
                        icon_button(
                            SharedString::from("reader-copy-body"),
                            "file",
                            15.,
                            theme.text_soft,
                            theme.card,
                            30.,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                            this.copy_current_body(cx);
                        })),
                    )
                    .child(vertical_divider(&theme, 18.))
                    .child(
                        icon_button(
                            SharedString::from("reader-reply"),
                            "reply",
                            15.,
                            theme.text_soft,
                            theme.card,
                            30.,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.open_compose(ComposeMode::Reply, window, cx);
                        })),
                    )
                    .child(
                        icon_button(
                            SharedString::from("reader-forward"),
                            "forward",
                            15.,
                            theme.text_soft,
                            theme.card,
                            30.,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.open_compose(ComposeMode::Forward, window, cx);
                        })),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(10.5))
                            .text_color(theme.text_muted)
                            .child(message.full_date_label()),
                    ),
            )
            .child(
                div()
                    .id("reader-pane")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .track_scroll(&self.reader_scroll)
                    .child(
                        div()
                            .w_full()
                            .max_w(px(800.))
                            .mx_auto()
                            .px(px(38.))
                            .pt(px(26.))
                            .pb(px(30.))
                            .flex()
                            .flex_col()
                            .when(is_draft, |this| {
                                this.child(
                                    div()
                                        .mb(px(14.))
                                        .p(px(12.))
                                        .rounded_lg()
                                        .bg(theme.accent_soft)
                                        .flex()
                                        .items_center()
                                        .gap(px(10.))
                                        .child(icon("compose", 15., theme.accent))
                                        .child(
                                            div()
                                                .flex_1()
                                                .text_size(px(11.))
                                                .text_color(theme.text_soft)
                                                .child("这是一封草稿，尚未发送"),
                                        )
                                        .child(
                                            div()
                                                .id("edit-draft")
                                                .h(px(28.))
                                                .px(px(12.))
                                                .flex()
                                                .items_center()
                                                .rounded_md()
                                                .bg(theme.accent)
                                                .text_size(px(11.))
                                                .text_color(gpui::white())
                                                .cursor_pointer()
                                                .on_click(move |_, window, cx| {
                                                    draft_entity.update(cx, |this, cx| {
                                                        this.open_draft(
                                                            draft_message_id,
                                                            window,
                                                            cx,
                                                        );
                                                    });
                                                })
                                                .child("继续编辑"),
                                        ),
                                )
                            })
                            .child(
                                div()
                                    .text_size(px(21.))
                                    .line_height(px(30.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text)
                                    .child(message.subject_or_placeholder().to_string()),
                            )
                            .child(
                                div()
                                    .mt(px(18.))
                                    .flex()
                                    .items_center()
                                    .gap(px(12.))
                                    .child(avatar(
                                        &message.from_email,
                                        &message.sender_label(),
                                        36.,
                                        14.,
                                        &theme,
                                    ))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.))
                                            .flex()
                                            .flex_col()
                                            .gap(px(3.))
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .text_size(px(13.))
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_color(theme.text)
                                                            .child(message.sender_label()),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(11.))
                                                            .text_color(theme.text_muted)
                                                            .child(format!(
                                                                "<{}>",
                                                                message.from_email
                                                            )),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.))
                                                    .text_color(theme.text_muted)
                                                    .truncate()
                                                    .child(format!(
                                                        "发给我 · {}",
                                                        if message.to.is_empty() {
                                                            "（无收件人）".to_string()
                                                        } else {
                                                            message.to.join("，")
                                                        }
                                                    )),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex_none()
                                            .text_size(px(10.5))
                                            .text_color(theme.text_muted)
                                            .child(message.time_label()),
                                    ),
                            )
                            .child(div().mt(px(18.)).child(horizontal_divider(&theme)))
                            .when(has_attachments, |this| {
                                this.child(
                                    div()
                                        .mt(px(16.))
                                        .flex()
                                        .flex_wrap()
                                        .gap_2()
                                        .children(attachments),
                                )
                            })
                            .child(
                                div()
                                    .mt(px(18.))
                                    .flex()
                                    .flex_col()
                                    .children(paragraphs),
                            )
                            .when_some(
                                self.render_translation_panel(message.id, theme, cx),
                                |this, panel| this.child(panel),
                            )
                            .when(!extract_links(&message.body).is_empty(), |this| {
                                let links = extract_links(&message.body);
                                this.child(
                                    div()
                                        .mt(px(18.))
                                        .flex()
                                        .flex_col()
                                        .gap(px(5.))
                                        .child(
                                            div()
                                                .text_size(px(10.))
                                                .text_color(theme.text_muted)
                                                .child("邮件中的链接"),
                                        )
                                        .children(links.into_iter().enumerate().map(
                                            |(index, url)| {
                                                let url_click = url.clone();
                                                div()
                                                    .id(SharedString::from(format!(
                                                        "reader-link-{index}"
                                                    )))
                                                    .h(px(24.))
                                                    .px(px(8.))
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(6.))
                                                    .rounded_sm()
                                                    .bg(theme.accent_soft)
                                                    .text_size(px(10.5))
                                                    .text_color(theme.accent)
                                                    .cursor_pointer()
                                                    .hover(|style| style.opacity(0.85))
                                                    .on_click(move |_, _, cx| {
                                                        cx.open_url(&url_click);
                                                    })
                                                    .child(icon("arrow_left", 11., theme.accent))
                                                    .child(div().truncate().child(url))
                                            },
                                        )),
                                )
                            })
                            .child(
                                div()
                                    .mt(px(26.))
                                    .pt(px(16.))
                                    .border_t_1()
                                    .border_color(theme.border_soft)
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("quick-reply")
                                            .h(px(32.))
                                            .px(px(12.))
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .rounded_md()
                                            .bg(theme.accent_soft)
                                            .text_color(theme.accent)
                                            .text_size(px(11.5))
                                            .font_weight(FontWeight::MEDIUM)
                                            .cursor_pointer()
                                            .hover(|style| style.opacity(0.85))
                                            .on_click(move |_, window, cx| {
                                                reply_entity.update(cx, |this, cx| {
                                                    this.open_compose(
                                                        ComposeMode::Reply,
                                                        window,
                                                        cx,
                                                    );
                                                });
                                            })
                                            .child(icon("reply", 13., theme.accent))
                                            .child("回复"),
                                    )
                                    .child(
                                        div()
                                            .id("quick-forward")
                                            .h(px(32.))
                                            .px(px(12.))
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .rounded_md()
                                            .border_1()
                                            .border_color(theme.border)
                                            .text_color(theme.text_soft)
                                            .text_size(px(11.5))
                                            .cursor_pointer()
                                            .hover(|style| style.bg(theme.card))
                                            .on_click(move |_, window, cx| {
                                                forward_entity.update(cx, |this, cx| {
                                                    this.open_compose(
                                                        ComposeMode::Forward,
                                                        window,
                                                        cx,
                                                    );
                                                });
                                            })
                                            .child(icon("forward", 13., theme.text_soft))
                                            .child("转发"),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_translation_panel(
        &self,
        message_id: i64,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.translation.visible {
            return None;
        }
        let entry = self.translation.entries.get(&message_id)?;

        // No provider or language chrome: the translated text is shown directly
        // as a quiet continuation of the original body.
        let mut panel = div()
            .mt(px(14.))
            .pl(px(12.))
            .py(px(2.))
            .border_l_2()
            .border_color(theme.accent)
            .flex()
            .flex_col();

        match entry {
            crate::app::TranslationEntry::Loading => {
                panel = panel.child(
                    div()
                        .text_size(px(12.))
                        .text_color(theme.text_muted)
                        .child("正在翻译…"),
                );
            }
            crate::app::TranslationEntry::Ready(text) => {
                for line in text.split('\n') {
                    panel = panel.child(
                        div()
                            .mt(px(3.))
                            .text_size(px(13.2))
                            .line_height(px(22.))
                            .text_color(theme.text)
                            .child(line.to_string()),
                    );
                }
            }
            crate::app::TranslationEntry::Failed(err) => {
                panel = panel
                    .border_color(theme.danger)
                    .child(
                        div()
                            .text_size(px(11.5))
                            .line_height(px(18.))
                            .text_color(theme.danger)
                            .child(format!("翻译失败：{err}")),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("translate-retry-{message_id}")))
                            .mt(px(8.))
                            .h(px(26.))
                            .px(px(10.))
                            .flex()
                            .items_center()
                            .rounded_md()
                            .bg(theme.accent_soft)
                            .text_color(theme.accent)
                            .text_size(px(10.5))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                                this.retry_translation(message_id, cx);
                            }))
                            .child("重试"),
                    );
            }
        }

        Some(panel.into_any_element())
    }

    fn render_thread(&mut self, thread: &Thread, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let entity = cx.entity();
        let count = thread.count();
        let removed = matches!(self.selected_folder.as_str(), "ARCHIVE" | "TRASH");

        let mut blocks: Vec<AnyElement> = Vec::new();
        for (index, message) in thread.messages.iter().enumerate() {
            let save_entity = entity.clone();
            let message_id = message.id;
            let attachments: Vec<AnyElement> = message
                .attachments
                .iter()
                .enumerate()
                .map(|(attachment_index, attachment)| {
                    let save_entity = save_entity.clone();
                    div()
                        .id(SharedString::from(format!(
                            "thread-{}-attachment-{}",
                            message_id, attachment_index
                        )))
                        .flex()
                        .items_center()
                        .gap_2()
                        .px(px(9.))
                        .py(px(5.))
                        .rounded_md()
                        .bg(theme.reader)
                        .border_1()
                        .border_color(theme.border_soft)
                        .cursor_pointer()
                        .hover(|style| style.bg(theme.card_hover))
                        .on_click(move |_, _, cx| {
                            save_entity.update(cx, |this, cx| {
                                this.save_attachment_of(message_id, attachment_index, cx);
                            });
                        })
                        .child(icon("file", 12., theme.accent))
                        .child(
                            div()
                                .text_size(px(10.5))
                                .text_color(theme.text_soft)
                                .child(attachment.name.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(9.5))
                                .text_color(theme.text_muted)
                                .child(attachment.size_label()),
                        )
                        .child({
                            let open_entity = entity.clone();
                            div()
                                .id(SharedString::from(format!(
                                    "thread-open-{}-{}",
                                    message_id, attachment_index
                                )))
                                .size(px(18.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_sm()
                                .cursor_pointer()
                                .hover(|style| style.bg(theme.card_hover))
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    open_entity.update(cx, |this, cx| {
                                        this.open_attachment_of(
                                            message_id,
                                            attachment_index,
                                            cx,
                                        );
                                    });
                                })
                                .child(icon("chevron_right", 10., theme.text_soft))
                        })
                        .into_any_element()
                })
                .collect();
            let has_attachments = !attachments.is_empty();

            let paragraphs: Vec<AnyElement> = message
                .body
                .split('\n')
                .map(|line| {
                    div()
                        .w_full()
                        .min_h(px(5.))
                        .text_size(px(13.5))
                        .line_height(px(23.))
                        .text_color(theme.text_soft)
                        .child(line.to_string())
                        .into_any_element()
                })
                .collect();

            blocks.push(
                div()
                    .w_full()
                    .p(px(16.))
                    .rounded_lg()
                    .bg(theme.card)
                    .border_1()
                    .border_color(if index + 1 == count {
                        theme.border
                    } else {
                        theme.border_soft
                    })
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(avatar(
                                &message.from_email,
                                &message.sender_label(),
                                26.,
                                11.,
                                &theme,
                            ))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(1.))
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.text)
                                            .child(message.sender_label()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(theme.text_muted)
                                            .child(format!("<{}>", message.from_email)),
                                    ),
                            )
                            .child(div().flex_1())
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(theme.text_muted)
                                    .child(message.full_date_label()),
                            ),
                    )
                    .when(has_attachments, |this| {
                        this.child(
                            div()
                                .mt(px(10.))
                                .flex()
                                .flex_wrap()
                                .gap_2()
                                .children(attachments),
                        )
                    })
                    .child(
                        div()
                            .mt(px(10.))
                            .flex()
                            .flex_col()
                            .children(paragraphs),
                    )
                    .when_some(
                        self.render_translation_panel(message.id, theme, cx),
                        |this, panel| this.child(panel),
                    )
                    .into_any_element(),
            );
        }

        let reply_entity = entity.clone();
        let forward_entity = entity.clone();

        div()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.reader)
            .child(
                div()
                    .flex_none()
                    .h(px(52.))
                    .px(px(16.))
                    .flex()
                    .items_center()
                    .gap_1()
                    .border_b_1()
                    .border_color(theme.border_soft)
                    .child(if removed {
                        icon_button(
                            SharedString::from("thread-restore"),
                            "inbox",
                            15.,
                            theme.text_soft,
                            theme.card,
                            30.,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                            this.restore_current(cx);
                        }))
                        .into_any_element()
                    } else {
                        icon_button(
                            SharedString::from("thread-archive"),
                            "archive",
                            15.,
                            theme.text_soft,
                            theme.card,
                            30.,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                            this.archive_current(cx);
                        }))
                        .into_any_element()
                    })
                    .child(
                        icon_button(
                            SharedString::from("thread-delete"),
                            "trash",
                            15.,
                            theme.text_soft,
                            theme.card,
                            30.,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                            this.delete_current(cx);
                        })),
                    )
                    .child(
                        icon_button(
                            SharedString::from("thread-unread"),
                            "mail",
                            15.,
                            theme.text_soft,
                            theme.card,
                            30.,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                            this.toggle_unread(cx);
                        })),
                    )
                    .child(
                        icon_button(
                            SharedString::from("thread-star"),
                            "star",
                            15.,
                            if thread.starred() {
                                theme.star
                            } else {
                                theme.text_soft
                            },
                            theme.card,
                            30.,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                            this.toggle_star(cx);
                        })),
                    )
                    .child(
                        icon_button(
                            SharedString::from("thread-translate"),
                            "languages",
                            15.,
                            if self.translation.visible {
                                theme.accent
                            } else {
                                theme.text_soft
                            },
                            theme.card,
                            30.,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                            this.toggle_translation(cx);
                        })),
                    )
                    .child(vertical_divider(&theme, 18.))
                    .child(
                        icon_button(
                            SharedString::from("thread-reply"),
                            "reply",
                            15.,
                            theme.text_soft,
                            theme.card,
                            30.,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.open_compose(ComposeMode::Reply, window, cx);
                        })),
                    )
                    .child(
                        icon_button(
                            SharedString::from("thread-forward"),
                            "forward",
                            15.,
                            theme.text_soft,
                            theme.card,
                            30.,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.open_compose(ComposeMode::Forward, window, cx);
                        })),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(10.5))
                            .text_color(theme.text_muted)
                            .child(format!("{count} 封邮件")),
                    ),
            )
            .child(
                div()
                    .id("thread-reader")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .track_scroll(&self.reader_scroll)
                    .child(
                        div()
                            .w_full()
                            .max_w(px(800.))
                            .mx_auto()
                            .px(px(38.))
                            .pt(px(26.))
                            .pb(px(30.))
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(21.))
                                    .line_height(px(30.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text)
                                    .child(thread.subject().to_string()),
                            )
                            .child(
                                div()
                                    .mt(px(10.))
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(icon("layers", 13., theme.accent))
                                    .child(
                                        div()
                                            .text_size(px(11.5))
                                            .text_color(theme.text_soft)
                                            .child(format!(
                                                "与 {} 的 {} 封邮件",
                                                thread.sender_name, count
                                            )),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.5))
                                            .text_color(theme.text_muted)
                                            .child(format!("<{}>", thread.sender_email)),
                                    ),
                            )
                            .child(div().mt(px(16.)).child(horizontal_divider(&theme)))
                            .child(
                                div()
                                    .mt(px(14.))
                                    .flex()
                                    .flex_col()
                                    .gap(px(10.))
                                    .children(blocks),
                            )
                            .child(
                                div()
                                    .mt(px(22.))
                                    .pt(px(14.))
                                    .border_t_1()
                                    .border_color(theme.border_soft)
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("thread-quick-reply")
                                            .h(px(32.))
                                            .px(px(12.))
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .rounded_md()
                                            .bg(theme.accent_soft)
                                            .text_color(theme.accent)
                                            .text_size(px(11.5))
                                            .font_weight(FontWeight::MEDIUM)
                                            .cursor_pointer()
                                            .hover(|style| style.opacity(0.85))
                                            .on_click(move |_, window, cx| {
                                                reply_entity.update(cx, |this, cx| {
                                                    this.open_compose(
                                                        ComposeMode::Reply,
                                                        window,
                                                        cx,
                                                    );
                                                });
                                            })
                                            .child(icon("reply", 13., theme.accent))
                                            .child("回复最新一封"),
                                    )
                                    .child(
                                        div()
                                            .id("thread-quick-forward")
                                            .h(px(32.))
                                            .px(px(12.))
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .rounded_md()
                                            .border_1()
                                            .border_color(theme.border)
                                            .text_color(theme.text_soft)
                                            .text_size(px(11.5))
                                            .cursor_pointer()
                                            .hover(|style| style.bg(theme.card))
                                            .on_click(move |_, window, cx| {
                                                forward_entity.update(cx, |this, cx| {
                                                    this.open_compose(
                                                        ComposeMode::Forward,
                                                        window,
                                                        cx,
                                                    );
                                                });
                                            })
                                            .child(icon("forward", 13., theme.text_soft))
                                            .child("转发"),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_empty_reader(&self, theme: Theme) -> AnyElement {
        div()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.))
            .bg(theme.reader)
            .child(div().opacity(0.45).child(logo(54.)))
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text_soft)
                    .child("选择一封邮件开始阅读"),
            )
            .child(
                div()
                    .text_size(px(11.5))
                    .text_color(theme.text_muted)
                    .child("Ctrl N 写邮件 · Ctrl J / K 切换邮件"),
            )
            .into_any_element()
    }

    // -----------------------------------------------------------------
    // Compose modal
    // -----------------------------------------------------------------

    fn render_compose(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let Some(compose) = self.compose.as_ref() else {
            return div().into_any_element();
        };
        let title = compose.mode.title();
        let sending = compose.sending;
        let to = compose.to.clone();
        let cc = compose.cc.clone();
        let bcc = compose.bcc.clone();
        let cc_focus = cc.clone();
        let bcc_focus = bcc.clone();
        let subject = compose.subject.clone();
        let body = compose.body.clone();
        let to_focus = to.clone();
        let subject_focus = subject.clone();
        let body_focus = body.clone();
        let show_cc = compose.show_cc;
        let show_bcc = compose.show_bcc;
        let attachments = compose.attachments.clone();
        let suggestions = self.contact_suggestions.clone();
        let entity = cx.entity();
        let suggestion_entity = entity.clone();
        let body_scroll = self.compose_scroll.clone();

        let close_entity = entity.clone();
        let draft_entity = entity.clone();
        let cc_entity = entity.clone();
        let bcc_entity = entity.clone();
        let send_entity = entity.clone();
        let add_attachment_entity = entity.clone();

        div()
            .absolute()
            .top(px(0.))
            .left(px(0.))
            .right(px(0.))
            .bottom(px(0.))
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.overlay)
            .child(
                div()
                    .w(px(720.))
                    .h(px(602.))
                    .max_h(relative(0.92))
                    .rounded_xl()
                    .bg(theme.reader)
                    .border_1()
                    .border_color(theme.border)
                                        .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex_none()
                            .h(px(52.))
                            .px(px(20.))
                            .flex()
                            .items_center()
                            .gap_2()
                            .border_b_1()
                            .border_color(theme.border_soft)
                            .child(icon("compose", 16., theme.accent))
                            .child(
                                div()
                                    .text_size(px(13.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(title),
                            )
                            .child(div().flex_1())
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(theme.text_muted)
                                    .child("Ctrl ↵ 发送"),
                            )
                            .child(
                                div()
                                    .id("compose-close")
                                    .size(px(28.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .hover(|style| style.bg(theme.card))
                                    .on_click(move |_, window, cx| {
                                        close_entity.update(cx, |this, cx| {
                                            this.close_compose(window, cx);
                                        });
                                    })
                                    .child(icon("close", 14., theme.text_muted)),
                            ),
                    )
                    .child(
                        div()
                            .flex_none()
                            .h(px(46.))
                            .px(px(20.))
                            .flex()
                            .items_center()
                            .gap_3()
                            .border_b_1()
                            .border_color(theme.border_soft)
                            .on_mouse_down(MouseButton::Left, focus_input(to_focus))
                            .child(field_label("收件人", &theme))
                            .child(div().flex_1().child(to))
                            .child(
                                div()
                                    .id("toggle-cc")
                                    .px(px(6.))
                                    .py(px(2.))
                                    .rounded_sm()
                                    .text_size(px(10.5))
                                    .text_color(theme.text_muted)
                                    .cursor_pointer()
                                    .hover(|style| style.bg(theme.card))
                                    .on_click(move |_, _, cx| {
                                        cc_entity.update(cx, |this, cx| {
                                            if let Some(compose) = this.compose.as_mut() {
                                                compose.show_cc = !compose.show_cc;
                                            }
                                            cx.notify();
                                        });
                                    })
                                    .child("抄送"),
                            )
                            .child(
                                div()
                                    .id("toggle-bcc")
                                    .px(px(6.))
                                    .py(px(2.))
                                    .rounded_sm()
                                    .text_size(px(10.5))
                                    .text_color(theme.text_muted)
                                    .cursor_pointer()
                                    .hover(|style| style.bg(theme.card))
                                    .on_click(move |_, _, cx| {
                                        bcc_entity.update(cx, |this, cx| {
                                            if let Some(compose) = this.compose.as_mut() {
                                                compose.show_bcc = !compose.show_bcc;
                                            }
                                            cx.notify();
                                        });
                                    })
                                    .child("密送"),
                            ),
                    )
                    .when(!suggestions.is_empty(), |this| {
                        this.child(
                            div()
                                .flex_none()
                                .px(px(20.))
                                .py(px(6.))
                                .flex()
                                .flex_wrap()
                                .gap(px(6.))
                                .border_b_1()
                                .border_color(theme.border_soft)
                                .children(suggestions.into_iter().enumerate().map(
                                    |(index, (name, email))| {
                                        let suggestion_entity = suggestion_entity.clone();
                                        div()
                                            .id(SharedString::from(format!(
                                                "contact-suggestion-{index}"
                                            )))
                                            .h(px(26.))
                                            .px(px(8.))
                                            .flex()
                                            .items_center()
                                            .gap(px(5.))
                                            .rounded_md()
                                            .bg(theme.card)
                                            .border_1()
                                            .border_color(theme.border_soft)
                                            .text_size(px(10.5))
                                            .text_color(theme.text_soft)
                                            .cursor_pointer()
                                            .hover(|style| style.bg(theme.card_hover))
                                            .on_click(move |_, window, cx| {
                                                suggestion_entity.update(cx, |this, cx| {
                                                    this.apply_contact_suggestion(
                                                        index, window, cx,
                                                    );
                                                });
                                            })
                                            .child(if name.trim().is_empty() {
                                                email.clone()
                                            } else {
                                                name.clone()
                                            })
                                            .child(
                                                div()
                                                    .text_size(px(9.5))
                                                    .text_color(theme.text_muted)
                                                    .child(format!("<{email}>")),
                                            )
                                    },
                                )),
                        )
                    })
                    .when(show_cc, |this| {
                        this.child(
                            div()
                                .flex_none()
                                .h(px(46.))
                                .px(px(20.))
                                .flex()
                                .items_center()
                                .gap_3()
                                .border_b_1()
                                .border_color(theme.border_soft)
                                .on_mouse_down(MouseButton::Left, focus_input(cc_focus))
                                .child(field_label("抄送", &theme))
                                .child(div().flex_1().child(cc)),
                        )
                    })
                    .when(show_bcc, |this| {
                        this.child(
                            div()
                                .flex_none()
                                .h(px(46.))
                                .px(px(20.))
                                .flex()
                                .items_center()
                                .gap_3()
                                .border_b_1()
                                .border_color(theme.border_soft)
                                .on_mouse_down(MouseButton::Left, focus_input(bcc_focus))
                                .child(field_label("密送", &theme))
                                .child(div().flex_1().child(bcc)),
                        )
                    })
                    .child(
                        div()
                            .flex_none()
                            .h(px(46.))
                            .px(px(20.))
                            .flex()
                            .items_center()
                            .gap_3()
                            .border_b_1()
                            .border_color(theme.border_soft)
                            .on_mouse_down(MouseButton::Left, focus_input(subject_focus))
                            .child(field_label("主题", &theme))
                            .child(div().flex_1().child(subject)),
                    )
                    .child(
                        div()
                            .id("compose-body")
                            .flex_1()
                            .min_h(px(0.))
                            .overflow_y_scroll()
                            .track_scroll(&body_scroll)
                            .on_mouse_down(MouseButton::Left, {
                                let body_focus = body_focus.clone();
                                move |_, window, cx| {
                                    window.focus(&body_focus.read(cx).handle());
                                }
                            })
                            .px(px(28.))
                            .py(px(16.))
                            .child(body),
                    )
                    .when(!attachments.is_empty(), |this| {
                        this.child(
                            div()
                                .flex_none()
                                .px(px(20.))
                                .py(px(8.))
                                .border_t_1()
                                .border_color(theme.border_soft)
                                .flex()
                                .flex_wrap()
                                .gap(px(6.))
                                .children(attachments.iter().enumerate().map(
                                    |(index, attachment)| {
                                        let remove_entity = entity.clone();
                                        div()
                                            .h(px(28.))
                                            .px(px(8.))
                                            .flex()
                                            .items_center()
                                            .gap(px(6.))
                                            .rounded_md()
                                            .bg(theme.card)
                                            .border_1()
                                            .border_color(theme.border_soft)
                                            .child(icon("paperclip", 12., theme.text_muted))
                                            .child(
                                                div()
                                                    .max_w(px(180.))
                                                    .truncate()
                                                    .text_size(px(10.5))
                                                    .text_color(theme.text_soft)
                                                    .child(attachment.name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(9.5))
                                                    .text_color(theme.text_muted)
                                                    .child(attachment.size_label()),
                                            )
                                            .child(
                                                div()
                                                    .id(SharedString::from(format!(
                                                        "remove-attachment-{index}"
                                                    )))
                                                    .size(px(18.))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .rounded_full()
                                                    .cursor_pointer()
                                                    .hover(|style| style.bg(theme.card_hover))
                                                    .on_click(move |_, _, cx| {
                                                        remove_entity.update(cx, |this, cx| {
                                                            this.remove_compose_attachment(index, cx);
                                                        });
                                                    })
                                                    .child(icon("close", 10., theme.text_muted)),
                                            )
                                    },
                                )),
                        )
                    })
                    .child(
                        div()
                            .flex_none()
                            .h(px(58.))
                            .px(px(20.))
                            .flex()
                            .items_center()
                            .gap_2()
                            .border_t_1()
                            .border_color(theme.border_soft)
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(theme.text_muted)
                                    .child("纯文本邮件 · Ctrl+Enter 快捷发送"),
                            )
                            .child(div().flex_1())
                            .child(
                                div()
                                    .id("add-attachment")
                                    .h(px(32.))
                                    .px(px(10.))
                                    .flex()
                                    .items_center()
                                    .gap(px(5.))
                                    .rounded_md()
                                    .text_size(px(11.5))
                                    .text_color(theme.text_soft)
                                    .cursor_pointer()
                                    .hover(|style| style.bg(theme.card))
                                    .on_click(move |_, _, cx| {
                                        add_attachment_entity.update(cx, |this, cx| {
                                            this.add_compose_attachments(cx);
                                        });
                                    })
                                    .child(icon("paperclip", 13., theme.text_soft))
                                    .child("附件"),
                            )
                            .child(
                                div()
                                    .id("save-draft")
                                    .h(px(32.))
                                    .px(px(12.))
                                    .flex()
                                    .items_center()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(theme.border)
                                    .text_size(px(11.5))
                                    .text_color(theme.text_soft)
                                    .cursor_pointer()
                                    .hover(|style| style.bg(theme.card))
                                    .on_click(move |_, window, cx| {
                                        draft_entity.update(cx, |this, cx| {
                                            this.save_draft(window, cx);
                                        });
                                    })
                                    .child("保存草稿"),
                            )
                            .child(
                                div()
                                    .id("send-mail")
                                    .h(px(32.))
                                    .px(px(16.))
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .rounded_md()
                                    .bg(if sending {
                                        theme.border
                                    } else {
                                        theme.accent
                                    })
                                    .text_color(gpui::white())
                                    .text_size(px(11.8))
                                    .font_weight(FontWeight::MEDIUM)
                                    .when(!sending, |this| {
                                        this.cursor_pointer()
                                            .hover(move |style| style.bg(theme.accent_hover))
                                    })
                                    .on_click(move |_, _, cx| {
                                        send_entity.update(cx, |this, cx| {
                                            this.send_current(cx);
                                        });
                                    })
                                    .child(icon("send", 13., gpui::white()))
                                    .child(if sending { "发送中…" } else { "发送" }),
                            ),
                    ),
            )
            .into_any_element()
    }

    // -----------------------------------------------------------------
    // Settings modal
    // -----------------------------------------------------------------

    fn render_settings(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let Some(settings) = self.settings.as_ref() else {
            return div().into_any_element();
        };
        let display_name = settings.display_name.clone();
        let email = settings.email.clone();
        let password = settings.password.clone();
        let error = settings.error.clone();
        let entity = cx.entity();
        let close_entity = entity.clone();
        let close_entity_2 = entity.clone();
        let save_entity = entity.clone();
        let remove_entity = entity.clone();
        let auth_help_entity = entity.clone();
        let paste_entity = entity.clone();

        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(section_label("账户信息", &theme))
            .child(settings_row("显示名称", &theme, display_name))
            .child(settings_row("邮箱地址", &theme, email))
            .child(settings_row("授权码", &theme, password))
            .child(
                div()
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(10.))
                            .line_height(px(15.))
                            .text_color(theme.text_muted)
                            .child("QQ / 163 / Gmail 等邮箱请使用「授权码」或「应用专用密码」，而不是登录密码。"),
                    )
                    .child(
                        div()
                            .id("auth-help-open")
                            .flex_none()
                            .h(px(22.))
                            .px(px(8.))
                            .flex()
                            .items_center()
                            .rounded_md()
                            .bg(theme.accent_soft)
                            .text_size(px(10.))
                            .text_color(theme.accent)
                            .cursor_pointer()
                            .hover(|style| style.bg(theme.card_hover))
                            .on_click(move |_, _window, cx| {
                                auth_help_entity.update(cx, |this, cx| {
                                    this.open_auth_help(cx);
                                });
                            })
                            .child("获取授权码"),
                    )
                    .child(
                        div()
                            .id("auth-code-paste")
                            .flex_none()
                            .h(px(22.))
                            .px(px(8.))
                            .flex()
                            .items_center()
                            .rounded_md()
                            .text_size(px(10.))
                            .text_color(theme.text_soft)
                            .cursor_pointer()
                            .hover(|style| style.bg(theme.card))
                            .on_click(move |_, window, cx| {
                                paste_entity.update(cx, |this, cx| {
                                    this.paste_auth_code(window, cx);
                                });
                            })
                            .child("粘贴"),
                    ),
            );

        if let Some(error) = &error {
            body = body.child(
                div()
                    .mx(px(12.))
                    .px(px(10.))
                    .py(px(6.))
                    .rounded_md()
                    .bg(gpui::rgba(0xD94A4A22))
                    .text_size(px(10.5))
                    .text_color(theme.danger)
                    .child(error.clone()),
            );
        }

        div()
            .absolute()
            .top(px(0.))
            .left(px(0.))
            .right(px(0.))
            .bottom(px(0.))
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.overlay)
            .child(
                div()
                    .w(px(640.))
                    .h(px(440.))
                    .max_h(relative(0.92))
                    .rounded_xl()
                    .bg(theme.reader)
                    .border_1()
                    .border_color(theme.border)
                                        .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex_none()
                            .h(px(52.))
                            .px(px(20.))
                            .flex()
                            .items_center()
                            .gap_2()
                            .border_b_1()
                            .border_color(theme.border_soft)
                            .child(icon("settings", 16., theme.accent))
                            .child(
                                div()
                                    .text_size(px(13.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("账户设置"),
                            )
                            .child(div().flex_1())
                            .child(
                                div()
                                    .id("settings-close")
                                    .size(px(28.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .hover(|style| style.bg(theme.card))
                                    .on_click(move |_, window, cx| {
                                        close_entity.update(cx, |this, cx| {
                                            this.settings = None;
                                            window.focus(&this.focus_handle);
                                            cx.notify();
                                        });
                                    })
                                    .child(icon("close", 14., theme.text_muted)),
                            ),
                    )
                    .child(
                        div()
                            .id("settings-body")
                            .flex_1()
                            .min_h(px(0.))
                            .overflow_y_scroll()
                            .py(px(6.))
                            .child(body),
                    )
                    .child(
                        div()
                            .flex_none()
                            .h(px(60.))
                            .px(px(18.))
                            .flex()
                            .items_center()
                            .gap_2()
                            .border_t_1()
                            .border_color(theme.border_soft)
                            .when(true, |this| {
                                this.child(
                                    div()
                                        .id("remove-account")
                                        .h(px(32.))
                                        .px(px(10.))
                                        .flex()
                                        .items_center()
                                        .rounded_md()
                                        .text_size(px(11.5))
                                        .text_color(theme.danger)
                                        .cursor_pointer()
                                        .hover(|style| style.bg(theme.card))
                                        .on_click(move |_, _window, cx| {
                                            remove_entity.update(cx, |this, cx| {
                                                this.request_remove_account(cx);
                                            });
                                        })
                                        .child("移除账户"),
                                )
                            })
                            .child(div().flex_1())
                            .child(
                                div()
                                    .id("settings-cancel")
                                    .h(px(32.))
                                    .px(px(12.))
                                    .flex()
                                    .items_center()
                                    .rounded_md()
                                    .text_size(px(11.5))
                                    .text_color(theme.text_soft)
                                    .cursor_pointer()
                                    .hover(|style| style.bg(theme.card))
                                    .on_click(move |_, window, cx| {
                                        close_entity_2.update(cx, |this, cx| {
                                            this.settings = None;
                                            window.focus(&this.focus_handle);
                                            cx.notify();
                                        });
                                    })
                                    .child("取消"),
                            )
                            .child(
                                div()
                                    .id("settings-save")
                                    .h(px(32.))
                                    .px(px(16.))
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .rounded_md()
                                    .bg(theme.accent)
                                    .text_color(gpui::white())
                                    .text_size(px(11.8))
                                    .font_weight(FontWeight::MEDIUM)
                                    .cursor_pointer()
                                    .hover(move |style| style.bg(theme.accent_hover))
                                    .on_click(move |_, window, cx| {
                                        save_entity.update(cx, |this, cx| {
                                            this.save_settings(window, cx);
                                        });
                                    })
                                    .child(icon("check", 13., gpui::white()))
                                    .child("保存并连接"),
                            ),
                    ),
            )
            .into_any_element()
    }

    // -----------------------------------------------------------------
    // Destructive-action confirmation
    // -----------------------------------------------------------------

    fn render_confirm(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let Some(action) = self.confirm.as_ref() else {
            return div().into_any_element();
        };
        let (title, message, icon_name, confirm_label) = match action {
            ConfirmAction::PermanentDelete { ids } => (
                "永久删除这些邮件？".to_string(),
                if ids.len() > 1 {
                    format!("将从本机永久删除 {} 封邮件，无法恢复。", ids.len())
                } else {
                    "将从本机永久删除这封邮件，无法恢复。".to_string()
                },
                "trash",
                "永久删除",
            ),
            ConfirmAction::RemoveAccount { email, .. } => (
                "移除这个账户？".to_string(),
                format!(
                    "账户 {email} 的本地缓存和设置会被删除；服务器上的邮件不受影响。"
                ),
                "user",
                "移除账户",
            ),
        };
        let entity = cx.entity();
        let cancel_entity = entity.clone();
        let confirm_entity = entity.clone();

        div()
            .absolute()
            .top(px(0.))
            .left(px(0.))
            .right(px(0.))
            .bottom(px(0.))
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.overlay)
            .child(
                div()
                    .w(px(440.))
                    .p(px(22.))
                    .rounded_xl()
                    .bg(theme.reader)
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .child(icon(icon_name, 18., theme.danger))
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text)
                                    .child(title),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .line_height(px(19.))
                            .text_color(theme.text_soft)
                            .child(message),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap(px(8.))
                            .child(
                                div()
                                    .id("confirm-cancel")
                                    .h(px(34.))
                                    .px(px(14.))
                                    .flex()
                                    .items_center()
                                    .rounded_md()
                                    .text_size(px(11.5))
                                    .text_color(theme.text_soft)
                                    .cursor_pointer()
                                    .hover(|style| style.bg(theme.card))
                                    .on_click(move |_, window, cx| {
                                        cancel_entity.update(cx, |this, cx| {
                                            this.cancel_confirm(window, cx);
                                        });
                                    })
                                    .child("取消"),
                            )
                            .child(
                                div()
                                    .id("confirm-danger")
                                    .h(px(34.))
                                    .px(px(16.))
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .rounded_md()
                                    .bg(theme.danger)
                                    .text_color(gpui::white())
                                    .text_size(px(11.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .cursor_pointer()
                                    .on_click(move |_, window, cx| {
                                        confirm_entity.update(cx, |this, cx| {
                                            this.confirm_destructive_action(window, cx);
                                        });
                                    })
                                    .child(icon("trash", 13., gpui::white()))
                                    .child(confirm_label),
                            ),
                    ),
            )
            .into_any_element()
    }

    // -----------------------------------------------------------------
    // Authorization-code help
    // -----------------------------------------------------------------

    fn render_auth_help(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let Some(provider) = self.auth_help else {
            return div().into_any_element();
        };
        let entity = cx.entity();
        let close_entity = entity.clone();
        let back_entity = entity.clone();
        let open_entity = entity.clone();
        let copy_entity = entity.clone();
        let paste_entity = entity.clone();

        let mut chips = Vec::new();
        for item in AuthHelpProvider::ALL {
            let active = item == provider;
            let chip_entity = entity.clone();
            chips.push(
                div()
                    .id(SharedString::from(format!(
                        "auth-provider-{}",
                        item as usize
                    )))
                    .h(px(26.))
                    .px(px(11.))
                    .flex()
                    .items_center()
                    .rounded_full()
                    .border_1()
                    .border_color(if active { theme.accent } else { theme.border })
                    .bg(if active { theme.accent_soft } else { theme.card })
                    .text_size(px(10.5))
                    .text_color(if active { theme.accent } else { theme.text_soft })
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.card_hover))
                    .on_click(move |_, _, cx| {
                        chip_entity.update(cx, |this, cx| {
                            this.select_auth_provider(item, cx);
                        });
                    })
                    .child(item.label()),
            );
        }

        let mut step_rows = Vec::new();
        for (index, step) in provider.steps().iter().enumerate() {
            step_rows.push(
                div()
                    .flex()
                    .items_center()
                    .gap(px(9.))
                    .child(
                        div()
                            .flex_none()
                            .size(px(18.))
                            .rounded_full()
                            .bg(theme.accent_soft)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(9.5))
                            .text_color(theme.accent)
                            .child(format!("{}", index + 1)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(11.))
                            .line_height(px(18.))
                            .text_color(theme.text_soft)
                            .child(*step),
                    ),
            );
        }

        let url = provider.url();
        let domain = url
            .and_then(|url| url.split('/').nth(2))
            .unwrap_or("官方帮助中心")
            .to_string();

        let content = div()
            .flex()
            .flex_col()
            .pb(px(16.))
            .child(
                div()
                    .mx(px(18.))
                    .mt(px(12.))
                    .p(px(12.))
                    .rounded_lg()
                    .bg(theme.accent_soft)
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(icon("bolt", 15., theme.accent))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(10.5))
                            .line_height(px(17.))
                            .text_color(theme.text_soft)
                            .child("授权码由邮箱服务商在官方网页生成，它不是登录密码。软件只保存你粘贴的授权码，不会代为申请或记录网页登录信息。"),
                    ),
            )
            .child(
                div()
                    .mx(px(18.))
                    .mt(px(12.))
                    .flex()
                    .flex_wrap()
                    .gap(px(7.))
                    .children(chips),
            )
            .child(
                div()
                    .mx(px(18.))
                    .mt(px(12.))
                    .p(px(14.))
                    .rounded_lg()
                    .bg(theme.card)
                    .border_1()
                    .border_color(theme.border_soft)
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text)
                                    .child(provider.title()),
                            )
                            .child(div().flex_1())
                            .child(
                                div()
                                    .h(px(18.))
                                    .px(px(7.))
                                    .flex()
                                    .items_center()
                                    .rounded_full()
                                    .bg(theme.accent_soft)
                                    .text_size(px(9.))
                                    .text_color(theme.accent)
                                    .child("官方页面"),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(9.5))
                            .text_color(theme.text_muted)
                            .child(domain),
                    )
                    .child(div().flex().flex_col().gap(px(8.)).children(step_rows))
                    .child(
                        div()
                            .flex()
                            .gap(px(6.))
                            .child(icon("lock", 11., theme.text_muted))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(9.5))
                                    .line_height(px(15.))
                                    .text_color(theme.text_muted)
                                    .child(provider.note()),
                            ),
                    ),
            )
            .child(
                div()
                    .mx(px(18.))
                    .mt(px(12.))
                    .text_size(px(9.5))
                    .line_height(px(15.))
                    .text_color(theme.text_muted)
                    .child("流程：打开官方页面生成授权码 → 复制 → 回到这里点「粘贴授权码」→ 保存并连接。"),
            );

        let mut footer = div()
            .flex_none()
            .h(px(58.))
            .px(px(18.))
            .flex()
            .items_center()
            .gap(px(8.))
            .border_t_1()
            .border_color(theme.border_soft)
            .child(
                div()
                    .id("auth-help-back")
                    .h(px(32.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .rounded_md()
                    .text_size(px(11.5))
                    .text_color(theme.text_soft)
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.card))
                    .on_click(move |_, _, cx| {
                        back_entity.update(cx, |this, cx| {
                            this.close_auth_help(cx);
                        });
                    })
                    .child("返回账户设置"),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("auth-help-paste")
                    .h(px(32.))
                    .px(px(11.))
                    .flex()
                    .items_center()
                    .rounded_md()
                    .bg(theme.accent_soft)
                    .text_size(px(11.5))
                    .text_color(theme.accent)
                    .cursor_pointer()
                    .on_click(move |_, window, cx| {
                        paste_entity.update(cx, |this, cx| {
                            this.paste_auth_code(window, cx);
                        });
                    })
                    .child("粘贴授权码"),
            );

        if url.is_some() {
            footer = footer
                .child(
                    div()
                        .id("auth-help-copy")
                        .h(px(32.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .rounded_md()
                        .text_size(px(11.5))
                        .text_color(theme.text_soft)
                        .cursor_pointer()
                        .hover(|style| style.bg(theme.card))
                        .on_click(move |_, _, cx| {
                            copy_entity.update(cx, |this, cx| {
                                this.copy_auth_help_link(cx);
                            });
                        })
                        .child("复制链接"),
                )
                .child(
                    div()
                        .id("auth-help-open")
                        .h(px(32.))
                        .px(px(14.))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .rounded_md()
                        .bg(theme.accent)
                        .text_color(gpui::white())
                        .text_size(px(11.5))
                        .font_weight(FontWeight::MEDIUM)
                        .cursor_pointer()
                        .hover(move |style| style.bg(theme.accent_hover))
                        .on_click(move |_, _, cx| {
                            open_entity.update(cx, |this, cx| {
                                this.open_auth_help_page(cx);
                            });
                        })
                        .child("打开官方申请页")
                        .child(icon("chevron_right", 12., gpui::white())),
                );
        }

        div()
            .absolute()
            .top(px(0.))
            .left(px(0.))
            .right(px(0.))
            .bottom(px(0.))
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.overlay)
            .child(
                div()
                    .w(px(620.))
                    .h(px(550.))
                    .max_h(relative(0.92))
                    .rounded_xl()
                    .bg(theme.reader)
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex_none()
                            .h(px(54.))
                            .px(px(20.))
                            .flex()
                            .items_center()
                            .gap_2()
                            .border_b_1()
                            .border_color(theme.border_soft)
                            .child(icon("lock", 16., theme.accent))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(1.))
                                    .child(
                                        div()
                                            .text_size(px(13.5))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.text)
                                            .child("获取邮箱授权码"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(9.5))
                                            .text_color(theme.text_muted)
                                            .child("在官方网页申请后，复制并粘贴回 mail"),
                                    ),
                            )
                            .child(div().flex_1())
                            .child(
                                div()
                                    .id("auth-help-close")
                                    .size(px(28.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .hover(|style| style.bg(theme.card))
                                    .on_click(move |_, _, cx| {
                                        close_entity.update(cx, |this, cx| {
                                            this.close_auth_help(cx);
                                        });
                                    })
                                    .child(icon("close", 14., theme.text_muted)),
                            ),
                    )
                    .child(
                        div()
                            .id("auth-help-body")
                            .flex_1()
                            .min_h(px(0.))
                            .overflow_y_scroll()
                            .child(content),
                    )
                    .child(footer),
            )
            .into_any_element()
    }

    // -----------------------------------------------------------------
    // First-run welcome / login
    // -----------------------------------------------------------------

    fn render_welcome(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let Some(settings) = self.settings.as_ref() else {
            return div().into_any_element();
        };
        let display_name = settings.display_name.clone();
        let email = settings.email.clone();
        let password = settings.password.clone();
        let error = settings.error.clone();
        let entity = cx.entity();
        let save_entity = entity.clone();
        let auth_entity = entity.clone();
        let paste_entity = entity.clone();

        let mut form = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(settings_row("显示名称", &theme, display_name))
            .child(settings_row("邮箱地址", &theme, email))
            .child(settings_row("授权码", &theme, password))
            .child(
                div()
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(10.))
                            .line_height(px(15.))
                            .text_color(theme.text_muted)
                            .child("QQ / 163 / Gmail 等邮箱请使用授权码，而不是登录密码。"),
                    )
                    .child(
                        div()
                            .id("welcome-auth-help")
                            .h(px(22.))
                            .px(px(8.))
                            .flex()
                            .items_center()
                            .rounded_md()
                            .bg(theme.accent_soft)
                            .text_size(px(10.))
                            .text_color(theme.accent)
                            .cursor_pointer()
                            .hover(|style| style.bg(theme.card_hover))
                            .on_click(move |_, _window, cx| {
                                auth_entity.update(cx, |this, cx| {
                                    this.open_auth_help(cx);
                                });
                            })
                            .child("获取授权码"),
                    )
                    .child(
                        div()
                            .id("welcome-paste")
                            .h(px(22.))
                            .px(px(8.))
                            .flex()
                            .items_center()
                            .rounded_md()
                            .text_size(px(10.))
                            .text_color(theme.text_soft)
                            .cursor_pointer()
                            .hover(|style| style.bg(theme.card))
                            .on_click(move |_, window, cx| {
                                paste_entity.update(cx, |this, cx| {
                                    this.paste_auth_code(window, cx);
                                });
                            })
                            .child("粘贴"),
                    ),
            );

        if let Some(error) = &error {
            form = form.child(
                div()
                    .mx(px(12.))
                    .px(px(10.))
                    .py(px(6.))
                    .rounded_md()
                    .bg(gpui::rgba(0xD94A4A22))
                    .text_size(px(10.5))
                    .text_color(theme.danger)
                    .child(error.clone()),
            );
        }

        let header = div()
            .flex_none()
            .h(px(52.))
            .w_full()
            .flex()
            .items_center()
            .bg(theme.bg)
            .border_b_1()
            .border_color(theme.border_soft)
            .child(
                div()
                    .flex_none()
                    .w(px(220.))
                    .h_full()
                    .flex()
                    .items_center()
                    .gap_2()
                    .pl(px(18.))
                    .window_control_area(WindowControlArea::Drag)
                    .child(logo(26.))
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child("mail"),
                    ),
            )
            .child(div().flex_1())
            .child(
                icon_button(
                    SharedString::from("welcome-theme"),
                    if theme.dark { "sun" } else { "moon" },
                    16.,
                    theme.text_soft,
                    theme.card,
                    30.,
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                    this.toggle_theme(cx);
                })),
            )
            .child(window_button(
                "welcome-min",
                "minimize",
                12.,
                theme.card,
                WindowControlArea::Min,
                &theme,
            ))
            .child(window_button(
                "welcome-max",
                "maximize",
                11.,
                theme.card,
                WindowControlArea::Max,
                &theme,
            ))
            .child(
                div()
                    .id("welcome-close")
                    .w(px(46.))
                    .h(px(52.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|style| style.bg(gpui::rgb(0xD94A4A)))
                    .window_control_area(WindowControlArea::Close)
                    .child(icon("close", 12., theme.text_muted)),
            );

        let card = div()
            .w(px(640.))
            .max_h(relative(0.88))
            .rounded_xl()
            .bg(theme.reader)
            .border_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_none()
                    .px(px(28.))
                    .pt(px(24.))
                    .pb(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(14.))
                    .child(logo(52.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(5.))
                            .child(
                                div()
                                    .text_size(px(22.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text)
                                    .child("欢迎使用 mail"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .line_height(px(17.))
                                    .text_color(theme.text_muted)
                                    .child("连接你的邮箱，开始使用本地优先、按联系人聚合的邮件客户端。"),
                            ),
                    ),
            )
            .child(
                div()
                    .id("welcome-form")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .py(px(4.))
                    .child(form),
            )
            .child(
                div()
                    .flex_none()
                    .h(px(58.))
                    .px(px(20.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_t_1()
                    .border_color(theme.border_soft)
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(9.5))
                            .line_height(px(14.))
                            .text_color(theme.text_muted)
                            .child("授权码只保存在本机；账户、邮件和附件不会被上传到任何服务器，除了你自己的邮件服务器。"),
                    )
                    .child(
                        div()
                            .id("welcome-login")
                            .h(px(34.))
                            .px(px(16.))
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .rounded_md()
                            .bg(theme.accent)
                            .text_size(px(11.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(gpui::white())
                            .cursor_pointer()
                            .hover(move |style| style.bg(theme.accent_hover))
                            .on_click(move |_, window, cx| {
                                save_entity.update(cx, |this, cx| {
                                    this.save_settings(window, cx);
                                });
                            })
                            .child("登录并开始使用")
                            .child(icon("chevron_right", 12., gpui::white())),
                    ),
            );

        let mut root = div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.bg)
            .text_color(theme.text)
            .font_family("Microsoft YaHei UI")
            .text_size(px(13.))
            .on_action(cx.listener(Self::action_toggle_theme))
            .on_action(cx.listener(Self::action_dismiss))
            .child(header)
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .px(px(28.))
                    .child(card),
            );
        if self.auth_help.is_some() {
            root = root.child(self.render_auth_help(cx));
        }
        if let Some(toast) = self.render_toast(cx) {
            root = root.child(toast);
        }
        root.into_any_element()
    }

    // -----------------------------------------------------------------
    // Toast
    // -----------------------------------------------------------------

    fn render_toast(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let toast = self.toast.as_ref()?;
        let theme = self.theme;
        let (color, icon_name) = toast_colors(toast.kind, &theme);
        Some(
            div()
                .absolute()
                .bottom(px(26.))
                .left(px(0.))
                .right(px(0.))
                .flex()
                .justify_center()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px(px(14.))
                        .h(px(38.))
                        .rounded_full()
                        .bg(theme.card)
                        .border_1()
                        .border_color(theme.border)
                        .shadow_md()
                        .child(icon(icon_name, 14., color))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(theme.text)
                                .child(toast.message.clone()),
                        )
                        .when_some(toast.action.clone(), |this, action| {
                            let label = match action {
                                crate::app::ToastAction::UndoMove { .. } => "撤销",
                            };
                            this.child(
                                div()
                                    .id("toast-undo")
                                    .h(px(22.))
                                    .px(px(8.))
                                    .ml(px(2.))
                                    .flex()
                                    .items_center()
                                    .rounded_md()
                                    .bg(theme.accent_soft)
                                    .text_color(theme.accent)
                                    .text_size(px(10.5))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                        this.undo_move(cx);
                                    }))
                                    .child(label),
                            )
                        }),
                )
                .into_any_element(),
        )
    }

    // -----------------------------------------------------------------
    // Actions
    // -----------------------------------------------------------------

    fn action_new_mail(&mut self, _: &NewMail, window: &mut Window, cx: &mut Context<Self>) {
        if self.compose.is_none() && self.settings.is_none() {
            self.open_compose(ComposeMode::New, window, cx);
        }
    }

    fn action_reply(&mut self, _: &ReplyMail, window: &mut Window, cx: &mut Context<Self>) {
        if self.compose.is_none() && self.settings.is_none() {
            self.open_compose(ComposeMode::Reply, window, cx);
        }
    }

    fn action_reply_all(&mut self, _: &ReplyAllMail, window: &mut Window, cx: &mut Context<Self>) {
        if self.compose.is_none() && self.settings.is_none() {
            self.open_compose(ComposeMode::ReplyAll, window, cx);
        }
    }

    fn action_forward(&mut self, _: &ForwardMail, window: &mut Window, cx: &mut Context<Self>) {
        if self.compose.is_none() && self.settings.is_none() {
            self.open_compose(ComposeMode::Forward, window, cx);
        }
    }

    fn action_archive(&mut self, _: &ArchiveMail, _window: &mut Window, cx: &mut Context<Self>) {
        if self.modal_open() {
            return;
        }
        self.archive_current(cx);
    }

    fn action_delete(&mut self, _: &DeleteMail, _window: &mut Window, cx: &mut Context<Self>) {
        if self.modal_open() {
            return;
        }
        self.delete_current(cx);
    }

    fn action_star(&mut self, _: &ToggleStar, _window: &mut Window, cx: &mut Context<Self>) {
        if self.modal_open() {
            return;
        }
        self.toggle_star(cx);
    }

    fn action_unread(&mut self, _: &ToggleUnread, _window: &mut Window, cx: &mut Context<Self>) {
        if self.modal_open() {
            return;
        }
        self.toggle_unread(cx);
    }

    fn action_next(&mut self, _: &NextMail, window: &mut Window, cx: &mut Context<Self>) {
        if self.modal_open() {
            return;
        }
        self.step_selection(1, window, cx);
    }

    fn action_prev(&mut self, _: &PrevMail, window: &mut Window, cx: &mut Context<Self>) {
        if self.modal_open() {
            return;
        }
        self.step_selection(-1, window, cx);
    }

    fn action_focus_search(
        &mut self,
        _: &FocusSearch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.modal_open() {
            return;
        }
        let handle = self.search.read(cx).handle();
        window.focus(&handle);
    }

    fn action_sync(&mut self, _: &SyncMail, _window: &mut Window, cx: &mut Context<Self>) {
        if self.modal_open() {
            return;
        }
        self.sync(cx);
    }

    fn action_translate(&mut self, _: &TranslateMail, _window: &mut Window, cx: &mut Context<Self>) {
        if self.modal_open() {
            return;
        }
        self.toggle_translation(cx);
    }

    fn action_toggle_grouping(
        &mut self,
        _: &ToggleGrouping,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.modal_open() {
            return;
        }
        self.toggle_grouping(cx);
    }

    fn action_open_settings(
        &mut self,
        _: &OpenSettings,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings.is_none() {
            self.open_settings(cx);
        }
    }

    fn action_send(&mut self, _: &SendMail, _window: &mut Window, cx: &mut Context<Self>) {
        if self.compose.is_some() {
            self.send_current(cx);
        }
    }

    fn action_toggle_theme(
        &mut self,
        _: &ToggleThemeAction,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_theme(cx);
    }

    fn action_dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        if self.auth_help.is_some() {
            self.close_auth_help(cx);
            return;
        }
        if self.welcome {
            return;
        }
        if self.confirm.is_some() {
            self.cancel_confirm(window, cx);
            return;
        }
        if self.compose.is_some() {
            let dirty = self.compose.as_ref().is_some_and(|compose| {
                !compose.subject.read(cx).value().trim().is_empty()
                    || !compose.body.read(cx).content().trim().is_empty()
            });
            if dirty {
                self.save_draft(window, cx);
            } else {
                self.close_compose(window, cx);
            }
        } else if self.settings.is_some() {
            self.settings = None;
            window.focus(&self.focus_handle);
            cx.notify();
        } else if !self.query.is_empty() {
            self.search
                .update(cx, |input, cx| input.clear(cx));
            window.focus(&self.focus_handle);
        }
    }
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.welcome {
            return self.render_welcome(window, cx);
        }
        let theme = self.theme;
        let titlebar = self.render_titlebar(window, cx);
        let sidebar = self.render_sidebar(cx);
        let list = self.render_list(cx);
        let reader = self.render_reader(cx);

        let mut overlays: Vec<AnyElement> = Vec::new();
        if self.compose.is_some() {
            overlays.push(self.render_compose(cx));
        }
        if self.settings.is_some() {
            overlays.push(self.render_settings(cx));
            if self.auth_help.is_some() {
                overlays.push(self.render_auth_help(cx));
            }
        }
        if self.confirm.is_some() {
            overlays.push(self.render_confirm(cx));
        }
        let toast = self.render_toast(cx);

        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.bg)
            .text_color(theme.text)
            .font_family("Microsoft YaHei UI")
            .text_size(px(13.))
            .on_action(cx.listener(Self::action_new_mail))
            .on_action(cx.listener(Self::action_reply))
            .on_action(cx.listener(Self::action_reply_all))
            .on_action(cx.listener(Self::action_forward))
            .on_action(cx.listener(Self::action_archive))
            .on_action(cx.listener(Self::action_delete))
            .on_action(cx.listener(Self::action_star))
            .on_action(cx.listener(Self::action_unread))
            .on_action(cx.listener(Self::action_next))
            .on_action(cx.listener(Self::action_prev))
            .on_action(cx.listener(Self::action_focus_search))
            .on_action(cx.listener(Self::action_sync))
            .on_action(cx.listener(Self::action_toggle_grouping))
            .on_action(cx.listener(Self::action_translate))
            .on_action(cx.listener(Self::action_send))
            .on_action(cx.listener(Self::action_toggle_theme))
            .on_action(cx.listener(Self::action_open_settings))
            .on_action(cx.listener(Self::action_dismiss))
            .child(titlebar)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .key_context("Mailbox")
                    .track_focus(&self.focus_handle)
                    .child(sidebar)
                    .child(list)
                    .child(reader),
            )
            .children(overlays)
            .children(toast)
            .into_any_element()
    }
}

// ---------------------------------------------------------------------
// Small layout helpers
// ---------------------------------------------------------------------

fn window_button(
    id: &'static str,
    icon_name: &'static str,
    icon_size: f32,
    hover_bg: gpui::Rgba,
    area: WindowControlArea,
    theme: &Theme,
) -> gpui::Stateful<Div> {
    div()
        .id(id)
        .w(px(42.))
        .h(px(52.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(move |style| style.bg(hover_bg))
        .window_control_area(area)
        .child(icon(icon_name, icon_size, theme.text_muted))
}

fn field_label(label: &'static str, theme: &Theme) -> Div {
    div()
        .flex_none()
        .w(px(46.))
        .text_size(px(11.5))
        .text_color(theme.text_muted)
        .child(label)
}

fn settings_row(label: &'static str, theme: &Theme, field: Entity<TextInput>) -> Div {
    let focus = focus_input(field.clone());
    div()
        .px(px(12.))
        .h(px(42.))
        .flex()
        .items_center()
        .gap_3()
        .child(
            div()
                .flex_none()
                .w(px(72.))
                .text_size(px(11.5))
                .text_color(theme.text_muted)
                .child(label),
        )
        .child(
            div()
                .flex_1()
                .h(px(32.))
                .px(px(8.))
                .flex()
                .items_center()
                .rounded_md()
                .bg(theme.card)
                .border_1()
                .border_color(theme.border_soft)
                .on_mouse_down(MouseButton::Left, focus)
                .child(field),
        )
}

fn focus_input(
    entity: Entity<TextInput>,
) -> impl Fn(&gpui::MouseDownEvent, &mut Window, &mut gpui::App) {
    move |_, window, cx| {
        window.focus(&entity.read(cx).handle());
    }
}

fn extract_links(body: &str) -> Vec<String> {
    let mut links = Vec::new();
    for word in body.split_whitespace() {
        let trimmed = word.trim_matches(|c: char| {
            matches!(
                c,
                '(' | ')'
                    | '['
                    | ']'
                    | '<'
                    | '>'
                    | '"'
                    | '\''
                    | ','
                    | '.'
                    | ';'
                    | ':'
                    | '，'
                    | '。'
                    | '；'
                    | '：'
                    | '！'
                    | '？'
            )
        });
        if (trimmed.starts_with("http://") || trimmed.starts_with("https://"))
            && !links.iter().any(|existing| existing == trimmed)
        {
            links.push(trimmed.to_string());
            if links.len() >= 8 {
                break;
            }
        }
    }
    links
}
