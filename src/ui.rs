use crate::*;
use egui::{FontFamily, Stroke, StrokeKind, Vec2};
const LINE: Color32 = Color32::from_rgb(42, 45, 54);
const WHITE: Color32 = Color32::from_rgb(239, 241, 246);
const SUBTLE: Color32 = Color32::from_rgb(111, 117, 133);
const ACCENT: Color32 = Color32::from_rgb(0, 144, 255);
fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}
fn type_at(p: &egui::Painter, pos: Pos2, label: &str, size: f32, color: Color32, strong: bool) {
    p.text(
        pos,
        Align2::LEFT_TOP,
        label,
        if strong {
            bold(size)
        } else {
            FontId::proportional(size)
        },
        color,
    );
}
fn rule(p: &egui::Painter, a: Pos2, b: Pos2) {
    p.line_segment([a, b], Stroke::new(1.0, LINE));
}
fn action(ui: &mut egui::Ui, rect: Rect, label: &str, primary: bool, enabled: bool) -> bool {
    let response = ui.interact(
        rect,
        egui::Id::new((label, rect.min.x.to_bits(), rect.min.y.to_bits())),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let fill = if primary {
        if !enabled {
            Color32::from_rgb(34, 68, 99)
        } else if response.hovered() {
            Color32::from_rgb(30, 157, 255)
        } else {
            BLUE
        }
    } else if response.hovered() {
        Color32::from_rgb(51, 55, 65)
    } else {
        Color32::from_rgb(37, 40, 49)
    };
    ui.painter().rect_filled(rect, 4, fill);
    if !primary {
        ui.painter().rect_stroke(
            rect,
            4,
            Stroke::new(1.0, Color32::from_rgb(57, 61, 72)),
            StrokeKind::Inside,
        );
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        bold(14.0),
        if enabled { WHITE } else { MUTED },
    );
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
        && enabled
}
fn image(p: &egui::Painter, texture: &TextureHandle, rect: Rect) {
    let size = texture.size_vec2();
    let scale = (rect.width() / size.x).min(rect.height() / size.y);
    let rect = Rect::from_center_size(rect.center(), size * scale);
    p.image(
        texture.id(),
        rect,
        Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
        WHITE,
    );
}
fn preferences(p: &egui::Painter, center: Pos2, color: Color32) {
    let stroke = Stroke::new(1.6, color);
    for (y, knob) in [(-7.0, -4.0), (0.0, 5.0), (7.0, -1.0)] {
        p.line_segment(
            [
                center + egui::vec2(-10.0, y),
                center + egui::vec2(knob - 3.0, y),
            ],
            stroke,
        );
        p.line_segment(
            [
                center + egui::vec2(knob + 3.0, y),
                center + egui::vec2(10.0, y),
            ],
            stroke,
        );
        p.circle_stroke(center + egui::vec2(knob, y), 3.0, stroke);
    }
}
fn gear(p: &egui::Painter, center: Pos2, color: Color32) {
    p.circle_stroke(center, 7.0, Stroke::new(1.6, color));
    p.circle_stroke(center, 2.5, Stroke::new(1.6, color));
    for i in 0..8 {
        let angle = i as f32 * std::f32::consts::TAU / 8.0;
        let v = Vec2::angled(angle);
        p.line_segment(
            [center + v * 7.0, center + v * 10.0],
            Stroke::new(2.2, color),
        );
    }
}
fn arrow(p: &egui::Painter, pos: Pos2, color: Color32) {
    p.line_segment([pos, pos + egui::vec2(8.0, 0.0)], Stroke::new(1.5, color));
    p.line_segment(
        [pos + egui::vec2(4.0, -4.0), pos + egui::vec2(8.0, 0.0)],
        Stroke::new(1.5, color),
    );
    p.line_segment(
        [pos + egui::vec2(4.0, 4.0), pos + egui::vec2(8.0, 0.0)],
        Stroke::new(1.5, color),
    );
}
fn side_link(ui: &mut egui::Ui, rect: Rect, label: &str) -> bool {
    let r = ui.interact(rect, egui::Id::new(label), Sense::click());
    if r.hovered() {
        ui.painter().rect_filled(rect, 4, PANEL);
    }
    type_at(
        ui.painter(),
        rect.min + egui::vec2(12.0, 12.0),
        label,
        13.0,
        if r.hovered() { WHITE } else { MUTED },
        false,
    );
    arrow(
        ui.painter(),
        egui::pos2(rect.right() - 24.0, rect.center().y),
        SUBTLE,
    );
    r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}
pub(super) fn setup(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "regular".into(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/Inter-Regular.otf")).into(),
    );
    fonts.font_data.insert(
        "bold".into(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/Inter-SemiBold.otf")).into(),
    );
    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, "regular".into());
    let mut fallback = fonts.families[&FontFamily::Proportional].clone();
    fallback.insert(0, "bold".into());
    fonts
        .families
        .insert(FontFamily::Name("bold".into()), fallback);
    ctx.set_fonts(fonts);
    ctx.set_theme(egui::Theme::Dark);
    let mut style = (*ctx.global_style()).clone();
    style.visuals = egui::Visuals::dark();
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, bold(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Heading, bold(22.0));
    style.spacing.menu_margin = egui::Margin::same(8);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
    style.visuals.hyperlink_color = ACCENT;
    style.visuals.panel_fill = BG;
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.corner_radius = 4.into();
        widget.fg_stroke = Stroke::new(1.0, WHITE);
    }
    style.visuals.widgets.inactive.bg_fill = PANEL;
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(47, 60, 77);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(31, 73, 113);
    style.visuals.widgets.open.bg_fill = Color32::from_rgb(31, 73, 113);
    style.visuals.widgets.open.weak_bg_fill = Color32::from_rgb(31, 73, 113);
    style.spacing.interact_size.y = 32.0;
    style.spacing.combo_width = 190.0;
    style.visuals.menu_corner_radius = 6.into();
    style.visuals.window_fill = BG;
    style.visuals.extreme_bg_color = Color32::from_rgb(14, 17, 23);
    style.visuals.override_text_color = Some(WHITE);
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
    style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(47, 60, 77);
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(75, 111, 150));
    style.visuals.widgets.active.weak_bg_fill = Color32::from_rgb(31, 73, 113);
    style.visuals.selection.bg_fill = Color32::from_rgb(25, 83, 136);
    style.visuals.window_corner_radius = 8.into();
    style.visuals.window_stroke = Stroke::new(1.0, LINE);
    style.visuals.widgets.inactive.weak_bg_fill = PANEL;
    ctx.set_global_style(style);
}
impl Launcher {
    pub(super) fn close_confirmation(&mut self, ctx: &egui::Context) {
        let response = egui::Modal::new(egui::Id::new("close-launcher-choice"))
            .frame(settings_frame())
            .show(ctx, |ui| {
                ui.set_width(420.0_f32.min(ctx.content_rect().width() - 48.0));
                ui.label(
                    RichText::new("Close Battle.net?")
                        .font(bold(22.0))
                        .color(WHITE),
                );
                ui.add_space(12.0);
                ui.label("Keep the launcher running in the background, or quit the application.");
                ui.add_space(16.0);
                ui.checkbox(&mut self.remember_close_choice, "Remember my choice");
                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.close_prompt = false;
                    }
                    if ui
                        .button(if self.tray.available() {
                            "Minimize to tray"
                        } else {
                            "Minimize window"
                        })
                        .clicked()
                    {
                        self.close_to_tray = true;
                        self.close_choice_remembered = self.remember_close_choice;
                        self.minimize_launcher(ctx);
                    }
                    if ui.button(RichText::new("Quit").strong()).clicked() {
                        self.close_to_tray = false;
                        self.close_choice_remembered = self.remember_close_choice;
                        self.quit_launcher(ctx);
                    }
                });
            });
        if response.should_close() {
            self.close_prompt = false;
        }
    }
    fn game_sidebar(&mut self, ui: &mut egui::Ui, rect: Rect) {
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.set_width(rect.width());
            ui.label(RichText::new("FRIENDS").font(bold(12.0)).color(MUTED));
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                let (dot, _) = ui.allocate_exact_size(egui::vec2(10.0, 16.0), Sense::hover());
                ui.painter().circle_filled(
                    dot.center(),
                    3.0,
                    if self.chat.connected {
                        Color32::from_rgb(112, 198, 76)
                    } else {
                        MUTED
                    },
                );
                ui.label(
                    RichText::new(if self.chat.connected {
                        "Connected"
                    } else if self.chat.authorizing {
                        "Signing in…"
                    } else if self.chat.connection.is_some() {
                        "Connecting…"
                    } else {
                        "Disconnected"
                    })
                    .size(12.0)
                    .color(MUTED),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.chat.connection.is_some() {
                        let options = ui
                            .add_sized([32.0, 28.0], egui::Button::new("•••"))
                            .on_hover_text("Chat options");
                        egui::Popup::menu(&options).show(|ui| {
                            if ui.button("Open chats").clicked() {
                                self.chat.open = true;
                                ui.close();
                            }
                            if ui.button("Disconnect").clicked() {
                                self.chat_paused = true;
                                self.chat.disconnect();
                                ui.close();
                            }
                        });
                    } else if !self.chat.connected
                        && self.signed_in_this_session
                        && self.pending.is_none()
                        && ui.small_button("Connect").clicked()
                    {
                        if let Some(account) = self.account.clone() {
                            self.chat_paused = false;
                            self.chat.status = "Connecting…".into();
                            self.chat.connection = Some(chat::Connection::start(
                                self.game.region.clone(),
                                account,
                                ui.ctx().clone(),
                            ));
                        }
                    }
                });
            });
            ui.add_space(12.0);
            let search_id = egui::Id::new("friend-search");
            let mut search = ui
                .ctx()
                .data_mut(|d| d.get_temp::<String>(search_id).unwrap_or_default());
            let search_align = if search.is_empty() {
                egui::Align::Center
            } else {
                egui::Align::Min
            };
            ui.add_sized(
                [rect.width(), 32.0],
                egui::TextEdit::singleline(&mut search)
                    .hint_text("Search friends")
                    .horizontal_align(search_align)
                    .vertical_align(egui::Align::Center)
                    .char_limit(100),
            );
            ui.ctx()
                .data_mut(|d| d.insert_temp(search_id, search.clone()));
            ui.add_space(14.0);
            let selected = self
                .chat
                .selected
                .and_then(|id| self.chat.contacts.iter().find(|f| f.id == id))
                .cloned();
            if !self.chat.contacts.is_empty() {
                ui.label(
                    RichText::new(format!("Friends · {}", self.chat.contacts.len()))
                        .size(11.0)
                        .color(MUTED),
                );
                ui.add_space(6.0);
            }
            let list_height = if selected.is_some() {
                150.0
            } else {
                (rect.bottom() - ui.cursor().top() - 100.0).max(90.0)
            };
            egui::ScrollArea::vertical()
                .id_salt("sidebar-contacts")
                .max_height(list_height)
                .show(ui, |ui| {
                    for contact in self
                        .chat
                        .contacts
                        .iter()
                        .filter(|f| f.name.to_lowercase().contains(&search.to_lowercase()))
                    {
                        let (row, response) =
                            ui.allocate_exact_size(egui::vec2(rect.width(), 48.0), Sense::click());
                        let active = self.chat.selected == Some(contact.id);
                        if active || response.hovered() {
                            ui.painter().rect_filled(
                                row,
                                4,
                                if active {
                                    Color32::from_rgb(35, 49, 66)
                                } else {
                                    PANEL
                                },
                            );
                        }
                        let center = row.left_center() + egui::vec2(22.0, 0.0);
                        ui.painter()
                            .circle_filled(center, 16.0, Color32::from_rgb(41, 55, 75));
                        let avatar_id = self
                            .chat
                            .avatars
                            .get(&(contact.id.high, contact.id.low))
                            .copied()
                            .unwrap_or(1);
                        if let Some(texture) = self.avatars.texture(ui.ctx(), avatar_id) {
                            image(
                                ui.painter(),
                                &texture,
                                Rect::from_center_size(center, egui::vec2(32.0, 32.0)),
                            );
                        } else {
                            // Neutral silhouette while the real portrait loads.
                            ui.painter()
                                .circle_filled(center - egui::vec2(0.0, 4.0), 5.0, SUBTLE);
                            ui.painter().rect_filled(
                                Rect::from_center_size(
                                    center + egui::vec2(0.0, 7.0),
                                    egui::vec2(15.0, 8.0),
                                ),
                                4,
                                SUBTLE,
                            );
                        }
                        let name = ui.painter().layout_no_wrap(
                            contact.name.clone(),
                            bold(12.0),
                            Color32::from_rgb(173, 211, 244),
                        );
                        let text_rect = Rect::from_min_max(
                            row.min + egui::vec2(47.0, 15.0),
                            row.max - egui::vec2(6.0, 0.0),
                        );
                        ui.painter()
                            .with_clip_rect(text_rect.intersect(ui.clip_rect()))
                            .galley(text_rect.min, name, WHITE);
                        if response
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            self.chat.selected = Some(contact.id);
                            self.chat.unread = 0;
                        }
                    }
                });
            if self.chat.contacts.is_empty() {
                ui.add_space(18.0);
                ui.label(
                    RichText::new(if self.chat.connected && self.chat.friends_loaded {
                        "No friends to display."
                    } else if self.chat.connected {
                        "Loading your friends…"
                    } else if self.chat.authorizing {
                        "Signing in to Battle.net…"
                    } else if self.chat.connection.is_some() {
                        "Connecting to Battle.net…"
                    } else if self.signed_in_this_session {
                        "Your friends will appear here when chat connects."
                    } else {
                        "Sign in to see your friends."
                    })
                    .size(12.0)
                    .color(MUTED),
                );
            }
            if self.chat.error {
                ui.add_space(10.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(&self.chat.status)
                            .size(12.0)
                            .color(Color32::from_rgb(240, 145, 132)),
                    )
                    .wrap(),
                );
            }
            if let Some(friend) = selected {
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&friend.name).strong().color(WHITE));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button("×")
                            .on_hover_text("Close conversation")
                            .clicked()
                        {
                            self.chat.selected = None;
                        }
                    });
                });
                let height = (rect.bottom() - ui.cursor().top() - 115.0).max(70.0);
                egui::ScrollArea::vertical()
                    .id_salt(("sidebar-messages", friend.id.high, friend.id.low))
                    .stick_to_bottom(true)
                    .max_height(height)
                    .min_scrolled_height(height)
                    .show(ui, |ui| {
                        ui.set_width(rect.width() - 10.0);
                        for message in self.chat.messages.iter().filter(|m| m.peer == friend.id) {
                            egui::Frame::new()
                                .fill(if message.outgoing {
                                    Color32::from_rgb(28, 62, 94)
                                } else {
                                    PANEL
                                })
                                .corner_radius(6)
                                .inner_margin(9)
                                .show(ui, |ui| {
                                    ui.set_max_width(rect.width() - 32.0);
                                    ui.label(
                                        RichText::new(if message.outgoing {
                                            "You"
                                        } else {
                                            &message.name
                                        })
                                        .size(10.0)
                                        .color(MUTED),
                                    );
                                    ui.add(
                                        egui::Label::new(RichText::new(&message.text).size(13.0))
                                            .wrap(),
                                    );
                                });
                            ui.add_space(5.0);
                        }
                    });
                ui.add_space(6.0);
                ui.add_enabled_ui(self.chat.connected, |ui| {
                    ui.horizontal(|ui| {
                        let message_align = if self.chat.draft.is_empty() {
                            egui::Align::Center
                        } else {
                            egui::Align::Min
                        };
                        let edit = ui.add_sized(
                            [rect.width() - 65.0, 34.0],
                            egui::TextEdit::singleline(&mut self.chat.draft)
                                .hint_text("Message…")
                                .horizontal_align(message_align)
                                .vertical_align(egui::Align::Center)
                                .char_limit(4096),
                        );
                        let send = ui
                            .add_sized([55.0, 34.0], egui::Button::new("Send"))
                            .clicked()
                            || (edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                        if send
                            && !self.chat.draft.trim().is_empty()
                            && self.chat.draft.len() <= 4096
                        {
                            if let Some(connection) = &self.chat.connection {
                                if connection
                                    .commands
                                    .send(chat::Command::Send(
                                        friend.clone(),
                                        self.chat.draft.clone(),
                                    ))
                                    .is_ok()
                                {
                                    self.chat.draft.clear();
                                    edit.request_focus();
                                }
                            }
                        }
                    });
                });
            }
            if !self.session_note.is_empty() {
                ui.add_space(10.0);
                ui.label(RichText::new(&self.session_note).size(11.0).color(MUTED));
            }
            if !self.signed_in_this_session
                && self.account.is_some()
                && ui
                    .add_enabled(self.pending.is_none(), egui::Button::new("Sign in again"))
                    .clicked()
            {
                self.login(ui.ctx());
            }
            let footer = Rect::from_min_size(
                egui::pos2(rect.left(), rect.bottom() - 34.0),
                egui::vec2(rect.width(), 34.0),
            );
            if action(ui, footer, "Chats and friends", false, true) {
                self.chat.open = true;
            }
        });
    }

    fn featured_shop(&mut self, ui: &mut egui::Ui) {
        let slides = [
            (
                "S2",
                "StarCraft II",
                "Three races. A galaxy of strategy.",
                "Campaigns, co-op missions and competitive multiplayer.",
            ),
            (
                "S1",
                "StarCraft: Remastered",
                "Return to the Koprulu Sector",
                "Discover the original StarCraft and Brood War.",
            ),
            (
                "Hero",
                "Heroes of the Storm",
                "Your heroes. Your team.",
                "Bring Blizzard's heroes together in the Nexus.",
            ),
        ];
        let id = egui::Id::new("store-slideshow");
        let (mut index, mut since, mut paused) = ui.ctx().data_mut(|d| {
            d.get_temp::<(usize, Instant, bool)>(id)
                .unwrap_or((0, Instant::now(), false))
        });
        if !paused && since.elapsed() >= Duration::from_secs(8) {
            index = (index + 1) % slides.len();
            since = Instant::now();
        }
        if !paused {
            ui.ctx().request_repaint_after(Duration::from_secs(1));
        }
        let (code, name, title, description) = slides[index];
        let width = ui.available_width();
        let height = (width * 0.33).clamp(230.0, 310.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), Sense::hover());
        let art = match code {
            "S1" => &self.art.starcraft,
            "Hero" => &self.art.heroes,
            _ => &self.art.hero,
        };
        let picture = Rect::from_min_max(
            rect.min,
            egui::pos2(rect.left() + width * 0.56, rect.bottom()),
        );
        cover(ui.painter(), art, picture);
        let details = Rect::from_min_max(picture.right_top(), rect.max);
        ui.painter().rect_filled(details, 3, PANEL);
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(details.shrink(22.0)),
            |ui| {
                ui.label(RichText::new(name.to_uppercase()).size(11.0).color(ACCENT));
                ui.add_space(12.0);
                ui.label(RichText::new(title).font(bold(24.0)).color(WHITE));
                ui.add_space(8.0);
                ui.label(RichText::new(description).size(14.0).color(MUTED));
            },
        );
        if action(
            ui,
            Rect::from_min_size(
                details.left_bottom() + egui::vec2(22.0, -58.0),
                egui::vec2((details.width() - 44.0).max(100.0), 36.0),
            ),
            "Explore in store ↗",
            true,
            true,
        ) {
            self.open_link(shop_link(code));
        }
        ui.advance_cursor_after_rect(rect);
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui
                .add_sized([32.0, 32.0], egui::Button::new("‹"))
                .on_hover_text("Previous slide")
                .clicked()
            {
                index = (index + slides.len() - 1) % slides.len();
                since = Instant::now();
            }
            for i in 0..slides.len() {
                let (dot, response) =
                    ui.allocate_exact_size(egui::vec2(22.0, 24.0), Sense::click());
                ui.painter().circle_filled(
                    dot.center(),
                    if index == i { 4.0 } else { 3.0 },
                    if index == i { ACCENT } else { MUTED },
                );
                if response.clicked() {
                    index = i;
                    since = Instant::now();
                }
            }
            if ui
                .add_sized([32.0, 32.0], egui::Button::new("›"))
                .on_hover_text("Next slide")
                .clicked()
            {
                index = (index + 1) % slides.len();
                since = Instant::now();
            }
            ui.toggle_value(&mut paused, "Pause");
        });
        ui.ctx()
            .data_mut(|d| d.insert_temp(id, (index, since, paused)));
        ui.add_space(22.0);
    }
    fn shop(&mut self, ui: &mut egui::Ui) {
        self.featured_shop(ui);
        ui.label(RichText::new("Browse games").font(bold(24.0)).color(WHITE));
        ui.add_space(16.0);
        let id = egui::Id::new("shop-category");
        let mut category = ui.ctx().data_mut(|d| d.get_temp::<usize>(id).unwrap_or(0));
        ui.horizontal_wrapped(|ui| {
            for (i, name) in ["All games", "StarCraft", "Warcraft", "Diablo", "Other"]
                .iter()
                .enumerate()
            {
                ui.selectable_value(&mut category, i, *name);
            }
        });
        ui.ctx().data_mut(|d| d.insert_temp(id, category));
        ui.add_space(24.0);
        let columns = (ui.available_width() / 240.0).floor().clamp(2.0, 4.0);
        let width = (ui.available_width() - 16.0 * (columns - 1.0)) / columns;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(16.0, 16.0);
            for product in catalog::PRODUCTS
                .iter()
                .filter(|p| !matches!(p.code, "WoWC" | "WoWE" | "WoWA"))
            {
                let group = match product.code {
                    "S1" | "S2" => 1,
                    "W1R" | "W2R" | "W3" | "WoW" => 2,
                    "D3" | "Fen" | "OSI" => 3,
                    _ => 4,
                };
                if category != 0 && category != group {
                    continue;
                }
                let (card, response) =
                    ui.allocate_exact_size(egui::vec2(width, 236.0), Sense::click());
                let p = ui.painter();
                p.rect_filled(card, 6, PANEL);
                let picture = Rect::from_min_size(card.min, egui::vec2(width, 148.0));
                p.rect_filled(picture, 4, Color32::from_rgb(14, 17, 23));
                image(p, self.game_art_for(product.code), picture);
                let title = p.layout(product.name.into(), bold(15.0), WHITE, width - 28.0);
                p.galley(card.min + egui::vec2(14.0, 160.0), title, WHITE);
                let requirement = p.layout(
                    catalog::access(product.code).label().into(),
                    FontId::proportional(11.0),
                    MUTED,
                    width - 28.0,
                );
                p.galley(card.min + egui::vec2(14.0, 205.0), requirement, MUTED);
                if response.hovered() {
                    p.rect_stroke(card, 6, Stroke::new(1.0, ACCENT), StrokeKind::Inside);
                }
                if response
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    self.open_link(shop_link(product.code));
                }
            }
        });
        ui.add_space(24.0);
        if ui.button("Visit the full Battle.net store ↗").clicked() {
            self.open_link("https://shop.battle.net/");
        }
    }
    pub(super) fn chat_view(&mut self, ctx: &egui::Context) {
        let mut open = self.chat.open;
        egui::Window::new("Chats and friends").id(egui::Id::new("battle-net-chat")).title_bar(false).frame(settings_frame().inner_margin(20)).default_pos(ctx.content_rect().center()-egui::vec2(430.0,285.0)).default_size([860.0,570.0]).min_size([640.0,420.0]).show(ctx,|ui|{
            settings_header(ui,"Chats and friends","Battle.net",&mut open);
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui|{
                let status=if self.chat.status.is_empty(){"Chat is disconnected"}else{&self.chat.status};ui.label(RichText::new(status).color(if self.chat.error{Color32::from_rgb(240,145,132)}else if self.chat.connected{Color32::from_rgb(107,194,153)}else{MUTED}));
                if self.chat.connection.is_some(){if ui.button("Disconnect").clicked(){self.chat_paused = true; self.chat.disconnect();}}
                else if ui.add_enabled(self.signed_in_this_session&&self.pending.is_none(),egui::Button::new("Connect").fill(BLUE)).clicked(){if let Some(account)=self.account.clone(){self.chat_paused=false;self.chat.status="Connecting…".into();self.chat.connection=Some(chat::Connection::start(self.game.region.clone(),account,ctx.clone()));}}
            });ui.add_space(8.0);ui.separator();ui.add_space(8.0);
            if self.account.is_none(){settings_note(ui,"Sign in to your Battle.net account to connect to chat.");}
            else if !self.chat.connected && self.chat.contacts.is_empty(){settings_note(ui,"Connect to load your contacts and receive messages. Blizzard may open a sign-in window to authorize chat. Chat history is kept in memory for this launcher session.");ui.add_space(16.0);}
            let height=(ui.available_height()-8.0).max(260.0);
            let conversation_width=(ui.available_width()-242.0).max(250.0);
            ui.horizontal_top(|ui|{
                ui.vertical(|ui|{ui.set_width(210.0);ui.label(RichText::new("CONTACTS").size(11.0).color(MUTED));ui.add_space(10.0);
                    egui::ScrollArea::vertical().id_salt("chat-contacts").max_height(height-38.0).show(ui,|ui|{
                        for friend in &self.chat.contacts {
                            ui.horizontal(|ui| {
                                let avatar = self.chat.avatars.get(&(friend.id.high, friend.id.low)).copied().unwrap_or(1);
                                let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0,28.0), Sense::hover());
                                if let Some(texture) = self.avatars.texture(ctx, avatar) { image(ui.painter(), &texture, rect); }
                                let selected = self.chat.selected == Some(friend.id);
                                if ui.add_sized([164.0,36.0],egui::Button::new(&friend.name).selected(selected)).clicked(){self.chat.selected=Some(friend.id);self.chat.unread=0;}
                            });
                        }
                        if self.chat.connected&&self.chat.contacts.is_empty(){settings_note(ui,"No contacts were returned by Battle.net.");}
                    });
                });ui.separator();
                ui.vertical(|ui|{
                    let width=conversation_width;ui.set_width(width);ui.set_max_width(width);
                    let selected=self.chat.selected.and_then(|id|self.chat.contacts.iter().find(|f|f.id==id)).cloned();
                    if let Some(friend)=selected{
                        ui.label(RichText::new(&friend.name).font(bold(20.0)));ui.add_space(8.0);
                        egui::ScrollArea::vertical().id_salt(("chat-messages",friend.id.high,friend.id.low)).stick_to_bottom(true).max_height(height-115.0).min_scrolled_height((height-115.0).max(100.0)).show(ui,|ui|{
                            for message in self.chat.messages.iter().filter(|m|m.peer==friend.id){egui::Frame::new().fill(if message.outgoing{Color32::from_rgb(26,57,86)}else{PANEL}).inner_margin(10).corner_radius(5).show(ui,|ui|{ui.set_max_width(width-30.0);ui.label(RichText::new(if message.outgoing{"You"}else{&message.name}).size(11.0).color(MUTED));ui.add(egui::Label::new(&message.text).wrap());});ui.add_space(8.0);}
                        });ui.add_space(12.0);
                        let enabled=self.chat.connected;
                        ui.add_enabled_ui(enabled,|ui|{ui.horizontal(|ui|{
                            let message_align = if self.chat.draft.is_empty() { egui::Align::Center } else { egui::Align::Min };
                            let edit=ui.add_sized([(width-90.0).max(140.0),36.0],egui::TextEdit::singleline(&mut self.chat.draft).hint_text("Write a message…").horizontal_align(message_align).vertical_align(egui::Align::Center).char_limit(4096));
                            let send=ui.add_enabled(!self.chat.draft.trim().is_empty(),egui::Button::new("Send").fill(BLUE)).clicked() || edit.lost_focus()&&ui.input(|i|i.key_pressed(egui::Key::Enter));
                            if send&&!self.chat.draft.trim().is_empty(){if self.chat.draft.len()>4096{self.chat.status="Message is too long (maximum 4096 UTF-8 bytes).".into();}else if let Some(connection)=&self.chat.connection{let body=self.chat.draft.clone();if connection.commands.send(chat::Command::Send(friend.clone(),body)).is_ok(){self.chat.draft.clear();self.chat.status="Sending message…".into();edit.request_focus();}else{self.chat.status="Chat connection closed.".into();}}}
                        });});
                    }else{ui.add_space(48.0);ui.label(RichText::new("Choose a contact").font(bold(22.0)));settings_note(ui,"Private messages appear here after connecting to Battle.net.");}
                });
            });
        });
        self.chat.open = open;
        if open {
            self.chat.unread = 0;
        }
    }

    fn open_link(&mut self, url: &str) {
        if self.browser_job.is_some() {
            return;
        }
        let url = url.to_owned();
        let (tx, rx) = std::sync::mpsc::channel();
        self.browser_job = Some(rx);
        self.status = "Opening your browser…".into();
        std::thread::spawn(move || {
            let _ = tx.send(runner::open_url(&url));
        });
    }
    pub(super) fn app_settings_view(&mut self, ctx: &egui::Context) {
        let id = egui::Id::new("launcher-preferences-page");
        let mut page = ctx.data_mut(|d| d.get_temp::<usize>(id).unwrap_or(0));
        let response=egui::Modal::new(egui::Id::new("launcher-preferences"))
            .frame(settings_frame()).show(ctx,|ui|{
                ui.set_width(720.0_f32.min(ctx.content_rect().width()-64.0));
                settings_header(ui,"Settings","Battle.net Linux",&mut self.app_settings);
                ui.add_space(20.0);
                let content_width=(ui.available_width()-208.0).max(200.0);
                ui.horizontal_top(|ui|{
                    settings_navigation(ui,&["General","Appearance","Account"],&mut page);
                    ui.add_space(24.0);
                    ui.vertical(|ui|{
                        ui.set_min_height(240.0);ui.set_width(content_width);ui.set_max_width(content_width);
                        match page {
                            0=>{
                                settings_section(ui,"Window behavior","Choose what happens when you close the launcher.");
                                ui.radio_value(&mut self.close_choice_remembered,false,"Ask every time");
                                ui.radio_value(&mut self.close_choice_remembered,true,"Use my chosen action");
                                if self.close_choice_remembered {
                                    ui.indent("remembered-close-action",|ui|{
                                        ui.radio_value(&mut self.close_to_tray,true,"Minimize launcher");
                                        ui.radio_value(&mut self.close_to_tray,false,"Quit launcher");
                                    });
                                }
                                ui.add_space(12.0);
                                ui.checkbox(&mut self.auto_chat,"Connect to chat after sign-in");
                                settings_note(ui,if self.tray.available(){"Use the tray menu to reopen or quit the launcher. Games keep running when the launcher closes."}else{"Your desktop has no system tray. Minimize keeps the launcher in the taskbar."});
                            }
                            1=>{
                                settings_section(ui,"Interface size","Adjust text and controls for your display.");
                                let mut scale=ctx.zoom_factor();
                                if ui.add(egui::Slider::new(&mut scale,0.75..=1.75).suffix("×").step_by(0.05)).changed(){ctx.set_zoom_factor(scale);}
                                ui.add_space(12.0);
                                if ui.button("Restore default size").clicked(){ctx.set_zoom_factor(1.0);}
                            }
                            _=>{
                                settings_section(ui,"Battle.net account","Your saved sign-in is shared across your game library.");
                                if let Some(account)=&self.account {ui.label(RichText::new(account.battle_tag.as_deref().unwrap_or("Signed in")).size(20.0).strong());}
                                ui.add_space(12.0);
                                settings_note(ui,"Game authorization is managed separately for each title. Passwords are entered only on Blizzard's sign-in page.");
                                ui.add_space(12.0);
                                if let Some(refreshed)=self.chat.last_renewed {ui.label(RichText::new(format!("Session refreshed {} minutes ago",refreshed.elapsed().as_secs()/60)).size(12.0).color(MUTED));}
                                settings_note(ui,if self.session_verified {"Session verified with Battle.net."}else if self.session_note.is_empty(){"Reconnect to verify your Battle.net session."}else{&self.session_note});
                                if ui.add_enabled(self.pending.is_none()&&!self.login_via_chat,egui::Button::new("Renew sign-in")).clicked(){self.login(ctx);}
                                ui.add_space(16.0);
                                if ui.add_enabled(self.pending.is_none()&&!self.login_via_chat,egui::Button::new("Sign out")).clicked(){let game=self.game.clone();self.job(move||Completed::Logout(runner::sign_out_account(&game)));self.app_settings=false;}
                            }
                        }
                    });
                });
                settings_footer(ui,&mut self.app_settings);
            });
        ctx.data_mut(|d| d.insert_temp(id, page));
        if response.should_close() {
            self.app_settings = false;
        }
    }
    pub(super) fn settings_view(&mut self, ctx: &egui::Context) {
        let product = *catalog::product(&self.game.product_code).unwrap();
        let id = egui::Id::new("game-preferences-page");
        let mut page = ctx.data_mut(|d| d.get_temp::<usize>(id).unwrap_or(0));
        if self.discovered.is_none() && self.pending.is_none() {
            self.discover();
        }
        let locked = self.pending.is_some() || self.selected_running();
        let response=egui::Modal::new(egui::Id::new("game-preferences"))
            .frame(settings_frame()).show(ctx,|ui|{
                ui.set_width(780.0_f32.min(ctx.content_rect().width()-64.0));
                settings_header(ui,"Game settings",product.name,&mut self.settings);
                ui.add_space(20.0);
                let content_width=(ui.available_width()-208.0).max(200.0);
                ui.horizontal_top(|ui|{
                    settings_navigation(ui,&["Installation","Runtime","Updates","Authorization"],&mut page);
                    ui.add_space(24.0);
                    ui.vertical(|ui|{
                        ui.set_width(content_width);ui.set_max_width(content_width);ui.set_min_height(350.0);
                        egui::ScrollArea::vertical().id_salt("game-settings-content").max_height(380.0).show(ui,|ui|{
                        ui.add_enabled_ui(!locked,|ui|match page {
                            0=>{
                                settings_section(ui,"Installation","Choose the game folder, region, and language.");
                                settings_note(ui,catalog::access(&self.game.product_code).label());
                                if catalog::access(&self.game.product_code).requires_access_options(){settings_note(ui,"Ownership is not verified. You need an existing license, eligible trial, or active game time to play.");}
                                let mut directory=catalog::target_root(&self.game).map(|p|p.to_string_lossy().into_owned()).unwrap_or_default();
                                let(changed,browse)=path_field(ui,"Game folder",&mut directory,"/home/…/Games/…");
                                if browse{self.choose_path(discovery::Pick::GameFolder);}
                                if changed {
                                    self.game.install_dir=directory.clone();self.game.executable=std::path::Path::new(&directory).join(product.executable).to_string_lossy().into();
                                    self.build_label=runner::installed_build(&self.game).map(|s|s.trim_start_matches("Base").into());self.next_version_check=Instant::now();
                                }
                                ui.horizontal_wrapped(|ui|{if ui.button("Locate using EXE…").clicked(){self.choose_path(discovery::Pick::GameExe);}if ui.button("Find installations").clicked(){self.discover();}});
                                let installs=self.discovered.as_ref().map(|d|d.installations.clone()).unwrap_or_default();
                                if !installs.is_empty(){egui::ComboBox::from_id_salt("detected-games").width(ui.available_width()).selected_text(format!("Detected installations ({})",installs.len())).show_ui(ui,|ui|{for install in installs{if ui.selectable_label(false,install.root.display().to_string()).clicked(){self.settings_message=Some(discovery::apply(&mut self.game,discovery::Pick::GameFolder,&install.root).unwrap_or_else(|e|e));self.next_version_check=Instant::now();}}});}
                                ui.add_space(12.0);
                                ui.horizontal(|ui|{
                                    ui.vertical(|ui|{ui.label("Region");egui::ComboBox::from_id_salt("game-region").width(180.0).selected_text(region_name(&self.game.region)).show_ui(ui,|ui|{for region in ["EU","US","KR","CN"]{ui.selectable_value(&mut self.game.region,region.into(),region_name(region));}});});
                                    ui.add_space(16.0);
                                    ui.vertical(|ui|{ui.label("Language");egui::ComboBox::from_id_salt("game-locale").width(150.0).selected_text(locale_name(&self.game.locale)).show_ui(ui,|ui|{for (code,label) in LOCALES {ui.selectable_value(&mut self.game.locale,(*code).into(),*label);}});});
                                });
                                ui.add_space(20.0);
                                if ui.add_enabled(runner::install_root(&self.game).is_ok(),egui::Button::new("Open game folder")).clicked(){if let Ok(root)=runner::install_root(&self.game){if let Err(error)=runner::host_command("xdg-open").arg(root).spawn(){self.status=error.to_string();}}}
                                ui.add_space(12.0);settings_note(ui,"Changing region does not change your account or download another game edition.");
                            }
                            1=>{
                                settings_section(ui,"Game runtime","Use Wine, or an existing Proton installation with umu.");
                                let mut proton=runner::uses_proton(&self.game);
                                ui.horizontal(|ui|{if ui.selectable_value(&mut proton,false,"Wine").changed(){self.game.runtime_kind="wine".into();}if ui.selectable_value(&mut proton,true,"Proton / umu").changed(){self.game.runtime_kind="proton".into();}if ui.small_button("Refresh").clicked(){self.discover();}});ui.add_space(12.0);
                                let runners=self.discovered.as_ref().map(|d|d.runtimes.clone()).unwrap_or_default();
                                egui::ComboBox::from_id_salt("detected-runtime").width(ui.available_width()).selected_text("Choose a detected runner…").show_ui(ui,|ui|{for runner in runners{if runner.proton!=proton{continue;}if ui.selectable_label(false,&runner.name).on_hover_text(runner.path.display().to_string()).clicked(){self.settings_message=Some(discovery::apply(&mut self.game,if proton{discovery::Pick::Proton}else{discovery::Pick::Wine},&runner.path).unwrap_or_else(|e|e));if self.game.umu.is_empty(){if let Some(umu)=self.discovered.as_ref().and_then(|d|d.umu.as_ref()){self.game.umu=umu.to_string_lossy().into();}}}}});ui.add_space(12.0);
                                if !proton {let(_,browse)=path_field(ui,"Wine executable",&mut self.game.runtime,"wine");if browse{self.choose_path(discovery::Pick::Wine);}}
                                let(_,browse)=path_field(ui,"Wine prefix",&mut self.game.prefix,"/home/…/Games/battlenet");if browse{self.choose_path(discovery::Pick::Prefix);}
                                let prefixes=self.discovered.as_ref().map(|d|d.prefixes.clone()).unwrap_or_default();if !prefixes.is_empty(){egui::ComboBox::from_id_salt("detected-prefix").width(ui.available_width()).selected_text("Choose a detected prefix…").show_ui(ui,|ui|{for prefix in prefixes{if ui.selectable_label(false,prefix.display().to_string()).clicked(){self.game.prefix=prefix.to_string_lossy().into();}}});}
                                if proton {let(_,browse)=path_field(ui,"Proton folder",&mut self.game.proton,"/path/to/proton");if browse{self.choose_path(discovery::Pick::Proton);}let(_,browse)=path_field(ui,"umu-run executable",&mut self.game.umu,"/path/to/umu-run");if browse{self.choose_path(discovery::Pick::Umu);}}

                            }
                            2=>{
                                settings_section(ui,"Game updates","Download updates directly from Blizzard.");
                                ui.checkbox(&mut self.game.auto_update,"Install updates automatically while idle");
                                ui.add_space(16.0);
                                if let Some(status)=&self.version_status {
                                    ui.label(format!("Installed build {}",status.installed.trim_start_matches("Base")));
                                    ui.label(format!("Latest version {}",status.latest.name));
                                } else if let Some(build)=&self.build_label {ui.label(format!("Installed build {build}"));}
                                if let Some(error)=&self.version_error {ui.label(RichText::new(error).color(Color32::from_rgb(240,145,132)));}
                                ui.add_space(16.0);
                                if ui.button("Check for updates").clicked(){self.check_updates();}
                                ui.add_space(12.0);if ui.add_enabled(runner::install_root(&self.game).is_ok(),egui::Button::new("Scan & Repair")).clicked(){self.start_patch(true);}
                                settings_note(ui,"Scan & Repair verifies game files against Blizzard’s latest build and downloads replacements. The game must be closed.");
                                ui.add_space(16.0);settings_note(ui,"Updates wait until the game is closed. You can cancel a download before the new version is activated.");
                            }
                            _=>{
                                settings_section(ui,"Game authorization","Your account stays signed in between launcher sessions.");
                                settings_note(ui,"Refresh authorization if this game rejects a saved sign-in. Blizzard may ask you to confirm your account.");ui.add_space(16.0);
                                if ui.add_enabled(self.account.is_some(),egui::Button::new("Refresh authorization")).clicked(){if let(Some(account),Some(helper))=(self.account.clone(),runner::login_helper()){let game=self.game.clone();self.job(move||Completed::Authorization(runner::refresh_game_authorization(&game,&account,&helper)));}}
                                ui.add_space(24.0);
                                egui::CollapsingHeader::new("Advanced diagnostics").show(ui,|ui|{
                                    settings_note(ui,"Checks local ticket storage. This does not prove the game server will accept the ticket.");
                                    if ui.button("Check local ticket").clicked(){let game=self.game.clone();self.job(move||Completed::Probe(runner::probe(&game)));}
                                    if ui.button("Open launch details").clicked(){self.open_details();self.settings=false;}
                                });
                            }
                        });
                        if locked {ui.add_space(12.0);settings_note(ui,"Changes are paused while this game is running or an operation is in progress.");}
                        if self.patch_can_cancel {if let Some(cancel)=&self.patch_cancel {if ui.button("Cancel update").clicked(){cancel.store(true,std::sync::atomic::Ordering::Relaxed);}}}
                        });
                    });
                });
                if self.patch_progress.is_some(){settings_note(ui,&self.status);if let Some(fraction)=self.patch_fraction{ui.add(egui::ProgressBar::new(fraction).show_percentage());}}
                if let Some(message)=&self.settings_message{ui.add_space(8.0);settings_note(ui,message);}
                settings_footer(ui,&mut self.settings);
            });
        ctx.data_mut(|d| d.insert_temp(id, page));
        if response.should_close() {
            self.settings = false;
        }
    }

    fn game_icon(&self, code: &str) -> &TextureHandle {
        &self.art.game_icons[match code {
            "S1" => 1,
            "Hero" => 2,
            "Pro" => 3,
            "W1R" => 4,
            "W2R" => 5,
            "W3" => 6,
            "WoW" | "WoWC" | "WoWE" | "WoWA" => 7,
            "WTCG" => 8,
            "OSI" => 9,
            "D3" => 10,
            "Fen" => 11,
            _ => 0,
        }]
    }
    fn game_art_for(&self, code: &str) -> &TextureHandle {
        let index = catalog::PRODUCTS
            .iter()
            .position(|product| product.code == code)
            .unwrap_or(0);
        &self.art.game_covers[index]
    }
    fn game_art(&self) -> &TextureHandle {
        self.game_art_for(&self.game.product_code)
    }
    fn game_menu(&mut self, ui: &mut egui::Ui, code: &str) {
        ui.set_min_width(220.0);
        let current = self
            .profiles
            .iter()
            .find(|g| g.product_code == code)
            .cloned()
            .unwrap_or_else(|| self.game.clone());
        let installed = runner::install_root(&current).is_ok();
        if ui
            .add_sized(
                [220.0, 32.0],
                egui::Button::new(if installed {
                    "View game"
                } else {
                    catalog::access(code).installation_label()
                })
                .frame(false),
            )
            .clicked()
        {
            self.select_game(code);
            self.tab = 0;
            if !installed {
                self.settings = true;
            }
            ui.close();
        }
        if ui
            .add_sized(
                [220.0, 32.0],
                egui::Button::new("Game settings").frame(false),
            )
            .clicked()
        {
            self.select_game(code);
            self.settings = true;
            self.app_settings = false;
            self.diagnostics = false;
            ui.close();
        }
        ui.separator();
        if ui
            .add_enabled(
                installed,
                egui::Button::new("Check for updates")
                    .frame(false)
                    .min_size(egui::vec2(220.0, 32.0)),
            )
            .clicked()
        {
            self.select_game(code);
            self.check_updates();
            ui.close();
        }
        if ui
            .add_enabled(
                installed,
                egui::Button::new("Open installation folder")
                    .frame(false)
                    .min_size(egui::vec2(220.0, 32.0)),
            )
            .clicked()
        {
            if let Ok(root) = catalog::target_root(&current) {
                let _ = runner::host_command("xdg-open").arg(root).spawn();
            }
            ui.close();
        }
        if ui
            .add_sized(
                [220.0, 32.0],
                egui::Button::new("Launch details").frame(false),
            )
            .clicked()
        {
            self.select_game(code);
            self.open_details();
            ui.close();
        }
    }

    pub(super) fn login_view(&mut self, ui: &mut egui::Ui) {
        let b = ui.max_rect();
        let p = ui.painter().clone();
        let backdrop = Rect::from_min_size(b.min, egui::vec2(b.width(), 210.0));
        let mut fade = egui::Mesh::default();
        for (pos, color) in [
            (backdrop.left_top(), Color32::from_rgb(18, 35, 53)),
            (backdrop.right_top(), Color32::from_rgb(17, 29, 45)),
            (backdrop.right_bottom(), BG),
            (backdrop.left_bottom(), BG),
        ] {
            fade.colored_vertex(pos, color);
        }
        fade.add_triangle(0, 1, 2);
        fade.add_triangle(0, 2, 3);
        p.add(fade);
        brand(
            &p,
            egui::pos2(b.center().x - 135.0, b.top() + 56.0),
            true,
            &self.art,
        );
        let form = Rect::from_min_size(
            egui::pos2(b.center().x - 170.0, b.top() + 192.0),
            egui::vec2(340.0, 400.0),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(form), |ui| {
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Log in").font(bold(28.0)).color(WHITE));
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Sign in to your Battle.net account")
                        .size(14.0)
                        .color(MUTED),
                );
                ui.add_space(32.0);
                ui.add_space(14.0);
                let (r, _) = ui.allocate_exact_size(egui::vec2(340.0, 48.0), Sense::hover());
                if action(
                    ui,
                    r,
                    if self.pending.is_some() || self.login_via_chat {
                        "Signing in…"
                    } else {
                        "Log in with Battle.net"
                    },
                    true,
                    self.pending.is_none() && !self.login_via_chat,
                ) {
                    self.login(ui.ctx());
                }
                ui.add_space(14.0);
                ui.label(
                    RichText::new("Complete sign-in securely in the Battle.net window.")
                        .size(12.0)
                        .color(MUTED),
                );
                if self.pending.is_some() || self.login_via_chat {
                    ui.add_space(16.0);
                    ui.spinner();
                }
                if !self.status.is_empty() {
                    ui.add_space(16.0);
                    ui.label(RichText::new(&self.status).size(12.0).color(MUTED));
                }
                ui.add_space(16.0);
                if ui
                    .link(RichText::new("Sign-in details").size(12.0))
                    .clicked()
                {
                    self.open_details();
                }
                ui.add_space(20.0);
                if ui.link(RichText::new("Browse games").size(13.0)).clicked() {
                    self.enter_library(ui.ctx());
                }
            });
        });
    }
    pub(super) fn library_view(&mut self, ui: &mut egui::Ui) {
        let b = ui.max_rect();
        let p = ui.painter().clone();
        let split = b.right() - if self.sidebar_open { 260.0 } else { 64.0 };
        p.rect_filled(
            Rect::from_min_max(egui::pos2(split, b.top()), b.max),
            0,
            Color32::from_rgb(22, 24, 31),
        );
        rule(
            &p,
            egui::pos2(split, b.top() + 72.0),
            egui::pos2(split, b.bottom()),
        );
        brand(&p, b.min + egui::vec2(28.0, 18.0), false, &self.art);
        rule(
            &p,
            b.min + egui::vec2(83.0, 22.0),
            b.min + egui::vec2(83.0, 48.0),
        );
        for (i, title) in ["HOME", "GAMES", "SHOP"].iter().enumerate() {
            let rect = Rect::from_min_size(
                b.min + egui::vec2(104.0 + i as f32 * 92.0, 11.0),
                egui::vec2(80.0, 49.0),
            );
            let r = ui.interact(rect, egui::Id::new(*title), Sense::click());
            p.text(
                rect.center(),
                Align2::CENTER_CENTER,
                *title,
                bold(14.0),
                if self.tab == i || r.hovered() {
                    WHITE
                } else {
                    MUTED
                },
            );
            if self.tab == i {
                p.line_segment(
                    [
                        egui::pos2(rect.left() + 14.0, rect.bottom() + 1.0),
                        egui::pos2(rect.right() - 14.0, rect.bottom() + 1.0),
                    ],
                    Stroke::new(2.0, ACCENT),
                );
            }
            if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                self.tab = i;
            }
        }
        let config = Rect::from_min_size(
            egui::pos2(split - 58.0, b.top() + 17.0),
            egui::vec2(38.0, 38.0),
        );
        let response = ui.interact(config, egui::Id::new("global-settings"), Sense::click());
        preferences(
            &p,
            config.center(),
            if response.hovered() { WHITE } else { MUTED },
        );
        if response.on_hover_text("Launcher settings").clicked() {
            self.settings = false;
            self.diagnostics = false;
            self.app_settings = true;
        }
        let avatar = Rect::from_min_size(
            egui::pos2(
                split + if self.sidebar_open { 22.0 } else { 14.0 },
                b.top() + 17.0,
            ),
            egui::vec2(36.0, 36.0),
        );
        p.circle_filled(avatar.center(), 18.0, Color32::from_rgb(35, 45, 60));
        let avatar_id = self
            .account
            .as_ref()
            .and_then(|account| account.account_id.parse::<u64>().ok())
            .and_then(|id| {
                self.chat
                    .avatars
                    .iter()
                    .find(|((_, low), _)| *low == id)
                    .map(|(_, avatar)| *avatar)
            })
            .unwrap_or(1);
        if let Some(texture) = self.avatars.texture(ui.ctx(), avatar_id) {
            image(&p, &texture, avatar);
        } else {
            p.circle_filled(avatar.center() - egui::vec2(0.0, 4.0), 6.0, SUBTLE);
            p.rect_filled(
                Rect::from_center_size(
                    avatar.center() + egui::vec2(0.0, 8.0),
                    egui::vec2(18.0, 9.0),
                ),
                5,
                SUBTLE,
            );
        }
        if self.sidebar_open {
            type_at(
                &p,
                avatar.right_top() + egui::vec2(12.0, 1.0),
                self.account
                    .as_ref()
                    .and_then(|account| account.battle_tag.as_deref())
                    .unwrap_or("Battle.net account"),
                13.0,
                WHITE,
                true,
            );
            type_at(
                &p,
                avatar.right_top() + egui::vec2(12.0, 21.0),
                if self.chat.connected {
                    "Online"
                } else if self.chat.authorizing {
                    "Signing in…"
                } else if self.chat.connection.is_some() {
                    "Connecting…"
                } else if self.signed_in_this_session {
                    "Offline"
                } else {
                    "Sign-in required"
                },
                11.0,
                MUTED,
                false,
            );
        }
        let account_rect = Rect::from_min_max(
            avatar.left_top() - egui::vec2(6.0, 6.0),
            egui::pos2(b.right() - 12.0, avatar.bottom() + 6.0),
        );
        let account_button = ui
            .interact(
                account_rect,
                egui::Id::new("account-menu-button"),
                Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Account menu");
        if self.sidebar_open {
            let center = egui::pos2(b.right() - 23.0, avatar.center().y);
            p.line_segment(
                [
                    center + egui::vec2(-3.0, -2.0),
                    center + egui::vec2(0.0, 2.0),
                ],
                Stroke::new(1.3, MUTED),
            );
            p.line_segment(
                [
                    center + egui::vec2(0.0, 2.0),
                    center + egui::vec2(3.0, -2.0),
                ],
                Stroke::new(1.3, MUTED),
            );
        }
        egui::Popup::menu(&account_button).show(|ui| {
            ui.set_min_width(190.0);
            if ui.button("Account settings").clicked() {
                ui.ctx().data_mut(|data| {
                    data.insert_temp(egui::Id::new("launcher-preferences-page"), 2_usize)
                });
                self.app_settings = true;
                self.settings = false;
                ui.close();
            }
            if self.account.is_some() {
                if ui
                    .add_enabled(
                        self.pending.is_none()
                            && !self.login_via_chat
                            && !self.chat.authorizing
                            && (self.chat.connection.is_none() || self.chat.connected),
                        egui::Button::new("Refresh session"),
                    )
                    .on_hover_text(
                        "Reconnect using saved credentials. This does not check game ownership.",
                    )
                    .clicked()
                {
                    if self.game.region == "CN" {
                        self.login(ui.ctx());
                    } else if let Some(account) = self.account.clone() {
                        self.chat.connection.take();
                        self.chat.connected = false;
                        self.chat.friends_loaded = false;
                        self.chat.verified_account = None;
                        self.chat.last_renewed = None;
                        self.chat_paused = false;
                        self.next_chat_connect = Instant::now() + Duration::from_secs(30);
                        self.chat.status = "Refreshing session…".into();
                        self.chat.connection = Some(chat::Connection::start(
                            self.game.region.clone(),
                            account,
                            ui.ctx().clone(),
                        ));
                        self.session_note.clear();
                    }
                    ui.close();
                }
                if ui
                    .add_enabled(
                        self.pending.is_none() && !self.login_via_chat,
                        egui::Button::new("Sign out"),
                    )
                    .clicked()
                {
                    let game = self.game.clone();
                    self.job(move || Completed::Logout(runner::sign_out_account(&game)));
                    ui.close();
                }
            } else if ui
                .add_enabled(
                    self.pending.is_none() && !self.login_via_chat,
                    egui::Button::new("Sign in"),
                )
                .clicked()
            {
                self.login(ui.ctx());
                ui.close();
            }
            ui.separator();
            if ui.button("Quit launcher").clicked() {
                if let Some(cancel) = &self.patch_cancel {
                    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                self.force_quit = true;
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                ui.close();
            }
        });
        let rail = Rect::from_min_max(
            b.min + egui::vec2(24.0, 83.0),
            egui::pos2(split - 24.0, b.top() + 141.0),
        );
        p.rect_filled(rail, 4, PANEL);
        let all = Rect::from_min_size(rail.min + egui::vec2(8.0, 8.0), egui::vec2(115.0, 42.0));
        if ui
            .interact(all, egui::Id::new("all-games"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            self.tab = 1;
        }
        for y in 0..2 {
            for x in 0..2 {
                p.rect_filled(
                    Rect::from_min_size(
                        all.min + egui::vec2(12.0 + x as f32 * 6.0, 15.0 + y as f32 * 6.0),
                        egui::vec2(4.0, 4.0),
                    ),
                    0,
                    MUTED,
                );
            }
        }
        type_at(
            &p,
            all.min + egui::vec2(34.0, 14.0),
            "All games",
            12.0,
            MUTED,
            true,
        );
        rule(
            &p,
            rail.min + egui::vec2(132.0, 15.0),
            rail.min + egui::vec2(132.0, 43.0),
        );
        let favorites = Rect::from_min_max(
            rail.min + egui::vec2(144.0, 7.0),
            rail.max - egui::vec2(8.0, 7.0),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(favorites), |ui| {
            egui::ScrollArea::horizontal()
                .id_salt("game-favorites")
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        for product in catalog::PRODUCTS
                            .iter()
                            .filter(|p| !matches!(p.code, "WoWC" | "WoWE" | "WoWA"))
                        {
                            let (rect, response) =
                                ui.allocate_exact_size(egui::vec2(48.0, 44.0), Sense::click());
                            let selected = self.game.product_code == product.code
                                || product.code == "WoW"
                                    && self.game.product_code.starts_with("WoW");
                            let p = ui.painter();
                            if selected || response.hovered() {
                                p.rect_filled(rect, 5, Color32::from_rgb(43, 49, 61));
                            }
                            image(
                                p,
                                self.game_icon(product.code),
                                Rect::from_center_size(rect.center(), egui::vec2(34.0, 34.0)),
                            );
                            if selected {
                                p.line_segment(
                                    [
                                        rect.left_bottom() + egui::vec2(8.0, 0.0),
                                        rect.right_bottom() - egui::vec2(8.0, 0.0),
                                    ],
                                    Stroke::new(2.0, ACCENT),
                                );
                            }
                            if response.clicked() {
                                self.select_game(product.code);
                                self.tab = 0;
                            }
                            response
                                .clone()
                                .on_hover_text(product.name)
                                .on_hover_cursor(egui::CursorIcon::PointingHand);
                            response.context_menu(|ui| self.game_menu(ui, product.code));
                        }
                    });
                });
        });
        let body = Rect::from_min_max(
            b.min + egui::vec2(24.0, 165.0),
            egui::pos2(split - 24.0, b.bottom() - 117.0),
        );
        // Use the available desktop width; content chooses its own image height.
        // The social sidebar remains anchored to the right edge at every scale.
        ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
            ui.set_width(body.width());
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if self.tab == 0 && self.game.product_code == "S2" {
                        self.home(ui);
                    } else if self.tab == 0 {
                        self.product_home(ui);
                    } else if self.tab == 2 {
                        self.shop(ui);
                    } else {
                        self.games(ui);
                    }
                });
        });
        let toggle = Rect::from_min_size(
            egui::pos2(
                split + if self.sidebar_open { 204.0 } else { 14.0 },
                b.top() + if self.sidebar_open { 110.0 } else { 76.0 },
            ),
            egui::vec2(36.0, 28.0),
        );
        if action(
            ui,
            toggle,
            if self.sidebar_open { "›" } else { "‹" },
            false,
            true,
        ) {
            self.sidebar_open = !self.sidebar_open;
        }
        if self.sidebar_open {
            self.game_sidebar(
                ui,
                Rect::from_min_max(
                    egui::pos2(split + 20.0, b.top() + 118.0),
                    b.max - egui::vec2(20.0, 20.0),
                ),
            );
        } else if self.chat.unread > 0 {
            type_at(
                &p,
                egui::pos2(split + 20.0, b.top() + 122.0),
                &self.chat.unread.to_string(),
                12.0,
                ACCENT,
                true,
            );
        }
        let footer = Rect::from_min_max(
            egui::pos2(b.left(), b.bottom() - 101.0),
            egui::pos2(split, b.bottom()),
        );
        p.rect_filled(footer, 0, Color32::from_rgb(25, 28, 35));
        rule(&p, footer.left_top(), footer.right_top());
        let play =
            Rect::from_min_size(footer.min + egui::vec2(24.0, 24.0), egui::vec2(202.0, 52.0));
        let access = catalog::access(&self.game.product_code);
        let title = if self.patch_cancel.is_some() && self.patch_can_cancel {
            "CANCEL UPDATE"
        } else if self.pending.is_some() {
            "PLEASE WAIT"
        } else if self.selected_running() || self.existing_game {
            "RUNNING"
        } else if self
            .version_status
            .as_ref()
            .is_some_and(|status| status.needs_update())
        {
            "UPDATE"
        } else if runner::install_root(&self.game).is_err() {
            access.primary_action()
        } else if self.signed_in_this_session {
            "PLAY"
        } else {
            "LOG IN"
        };
        if action(
            ui,
            play,
            title,
            true,
            (self.patch_cancel.is_some() && self.patch_can_cancel)
                || (self.pending.is_none() && !self.selected_running() && !self.existing_game),
        ) {
            if let Some(cancel) = &self.patch_cancel {
                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                self.status = "Cancelling update…".into();
            } else if runner::install_root(&self.game).is_err() {
                if access.requires_access_options() {
                    self.open_link(shop_link(&self.game.product_code));
                } else {
                    self.start_update();
                }
            } else if self
                .version_status
                .as_ref()
                .is_some_and(|status| status.needs_update())
            {
                self.start_update();
            } else if self.signed_in_this_session {
                self.play(ui.ctx());
            } else {
                self.login(ui.ctx());
            }
        }
        let options = Rect::from_min_size(
            play.right_top() + egui::vec2(6.0, 0.0),
            egui::vec2(48.0, 52.0),
        );
        p.rect_filled(options, 4, PANEL);
        gear(&p, options.center(), MUTED);
        let options_response = ui
            .interact(options, egui::Id::new("game-options"), Sense::click())
            .on_hover_text("Game settings");
        let product_code = self.game.product_code.clone();
        options_response.context_menu(|ui| self.game_menu(ui, &product_code));
        if options_response.clicked() {
            self.app_settings = false;
            self.diagnostics = false;
            self.settings = true;
        }
        let update_message = self
            .version_status
            .as_ref()
            .filter(|status| status.needs_update())
            .map(|status| format!("Update required: {}", status.latest.name));
        let status = if !self.status.is_empty() {
            &self.status
        } else if let Some(message) = &update_message {
            message.as_str()
        } else if self.status.is_empty() {
            if runner::install_root(&self.game).is_err() {
                access.label()
            } else if self.signed_in_this_session {
                "Ready to launch"
            } else {
                "Sign in to your Battle.net account"
            }
        } else {
            &self.status
        };
        let is_error = ["stopped", "failed", "invalid", "missing", "could not"]
            .iter()
            .any(|word| status.to_lowercase().contains(word));
        let info = Rect::from_min_max(
            footer.min + egui::vec2(310.0, 26.0),
            footer.max - egui::vec2(130.0, 14.0),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(info), |ui| {
            ui.label(RichText::new(&self.game.name).font(bold(14.0)).color(WHITE));
            ui.label(RichText::new(status).size(12.0).color(if is_error {
                Color32::from_rgb(240, 145, 132)
            } else {
                MUTED
            }));
        });
        let details = Rect::from_min_size(
            egui::pos2(split - 111.0, footer.center().y - 18.0),
            egui::vec2(88.0, 36.0),
        );
        if action(ui, details, "Details", false, true) {
            self.open_details();
        }
    }
    fn home(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width();
        let visible_height = ui.clip_rect().height();
        let hero_height = if width > 1500.0 {
            (width / 4.0).min(visible_height * 0.58).clamp(280.0, 480.0)
        } else {
            (visible_height * 0.46).clamp(200.0, 300.0)
        };
        let compact = hero_height < 250.0;
        let picture_height = if width > 1500.0 {
            (width / 9.0).clamp(150.0, 260.0)
        } else {
            (visible_height - hero_height - 175.0).clamp(48.0, 150.0)
        };
        let (hero, _) = ui.allocate_exact_size(egui::vec2(width, hero_height), Sense::hover());
        let p = ui.painter().clone();
        cover(&p, &self.art.hero, hero);
        shade(&p, hero);
        p.rect_stroke(
            hero,
            4,
            Stroke::new(1.0, Color32::from_white_alpha(12)),
            StrokeKind::Inside,
        );
        image(
            &p,
            &self.art.logo,
            Rect::from_min_size(
                hero.min + egui::vec2(26.0, 12.0),
                if compact {
                    egui::vec2(150.0, 62.8)
                } else {
                    egui::vec2(210.0, 87.9)
                },
            ),
        );
        type_at(
            &p,
            hero.min + egui::vec2(28.0, if compact { 84.0 } else { 110.0 }),
            "Your next battle awaits",
            if compact { 24.0 } else { 28.0 },
            WHITE,
            true,
        );
        type_at(
            &p,
            hero.min + egui::vec2(28.0, if compact { 124.0 } else { 152.0 }),
            "Command the Terran, Zerg, and Protoss.",
            14.0,
            Color32::from_rgb(200, 210, 222),
            false,
        );
        if !compact {
            type_at(
                &p,
                hero.min + egui::vec2(28.0, 175.0),
                "Explore campaigns, co-op, and competitive multiplayer.",
                14.0,
                Color32::from_rgb(200, 210, 222),
                false,
            );
        }
        if action(
            ui,
            Rect::from_min_size(
                hero.min + egui::vec2(28.0, hero_height - 48.0),
                egui::vec2(166.0, 34.0),
            ),
            "Explore the game",
            false,
            true,
        ) {
            self.open_link("https://starcraft2.blizzard.com/");
        }
        ui.advance_cursor_after_rect(hero);
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Discover StarCraft II")
                    .font(bold(20.0))
                    .color(WHITE),
            );
        });
        ui.add_space(10.0);
        let gap = 16.0;
        let cw = (width - gap * 2.0) / 3.0;
        let start = ui.cursor().min;
        for (i, (eyebrow, title, description, anchor)) in [
            (
                "THREE ICONIC RACES",
                "Choose your side",
                "Terran. Zerg. Protoss.",
                "races",
            ),
            (
                "CO-OP MISSIONS",
                "Stronger together",
                "Team up and take command.",
                "game-features",
            ),
            (
                "COMPETITIVE MULTIPLAYER",
                "Rise through the ranks",
                "Strategy meets competition.",
                "game-features",
            ),
        ]
        .iter()
        .enumerate()
        {
            let card = Rect::from_min_size(
                start + egui::vec2(i as f32 * (cw + gap), 0.0),
                egui::vec2(cw, picture_height + 78.0),
            );
            let picture = Rect::from_min_size(card.min, egui::vec2(cw, picture_height));
            cover(&p, &self.art.cards[i], picture);
            p.rect_filled(
                Rect::from_min_max(picture.left_bottom(), card.max),
                3,
                PANEL,
            );
            type_at(
                &p,
                picture.left_bottom() + egui::vec2(16.0, 13.0),
                eyebrow,
                9.0,
                SUBTLE,
                true,
            );
            type_at(
                &p,
                picture.left_bottom() + egui::vec2(16.0, 31.0),
                title,
                16.0,
                WHITE,
                true,
            );
            type_at(
                &p,
                picture.left_bottom() + egui::vec2(16.0, 56.0),
                description,
                11.0,
                MUTED,
                false,
            );
            let r = ui.interact(card, egui::Id::new(*title), Sense::click());
            if r.hovered() {
                p.rect_stroke(
                    card,
                    3,
                    Stroke::new(1.0, Color32::from_rgb(82, 96, 120)),
                    StrokeKind::Inside,
                );
            }
            if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                self.open_link(&format!("https://starcraft2.blizzard.com/en-us/#{anchor}"));
            }
        }
        ui.advance_cursor_after_rect(Rect::from_min_size(
            start,
            egui::vec2(width, picture_height + 78.0),
        ));
        ui.add_space(8.0);
    }
    fn product_home(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width();
        let height = (ui.clip_rect().height() * 0.53).clamp(240.0, 320.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, 6, Color32::from_rgb(14, 17, 23));
        image(p, self.game_art(), rect);
        shade(p, rect);
        type_at(
            p,
            rect.left_bottom() + egui::vec2(28.0, -75.0),
            &self.game.name,
            28.0,
            WHITE,
            true,
        );
        type_at(
            p,
            rect.left_bottom() + egui::vec2(28.0, -36.0),
            if runner::install_root(&self.game).is_ok() {
                "Installed · Ready when you are"
            } else {
                catalog::access(&self.game.product_code).label()
            },
            14.0,
            MUTED,
            false,
        );
        ui.add_space(24.0);
        ui.label(
            RichText::new("Discover the game")
                .font(bold(20.0))
                .color(WHITE),
        );
        ui.add_space(12.0);
        ui.label(RichText::new(match self.game.product_code.as_str(){"S1"=>"The original StarCraft and Brood War campaigns, with Remastered features available to licensed accounts.","Hero"=>"Team up with friends and bring Blizzard's heroes to the Nexus.",_=>"Install, update, and launch with your configured Wine or Proton runtime."}).size(15.0).color(MUTED));
        ui.add_space(16.0);
        let access = catalog::access(&self.game.product_code);
        if access.requires_access_options() {
            settings_note(ui, access.label());
            settings_note(
                ui,
                "Account ownership has not been verified. Installing files does not grant game access.",
            );
            if runner::install_root(&self.game).is_err()
                && ui
                    .add_enabled(
                        self.pending.is_none(),
                        egui::Button::new("Install with existing access…"),
                    )
                    .clicked()
            {
                self.start_update();
            }
            ui.add_space(12.0);
        }
        ui.horizontal(|ui| {
            if ui.button("Shop game ↗").clicked() {
                self.tab = 2;
            }
            if ui.button("Game settings").clicked() {
                self.settings = true;
            }
            if ui.button("Launch details").clicked() {
                self.open_details();
            }
        });
        ui.add_space(12.0);
        if ui.button("Visit official site ↗").clicked() {
            if let Ok(product) = catalog::product(&self.game.product_code) {
                self.open_link(product.website);
            }
        }
    }
    fn games(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Games").font(bold(28.0)).color(WHITE));
        ui.add_space(8.0);
        ui.label(RichText::new("Your Blizzard games").size(13.0).color(MUTED));
        ui.add_space(24.0);
        let width = ((ui.available_width() - 32.0) / 3.0).min(248.0).max(170.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(16.0, 16.0);
            for product in catalog::PRODUCTS {
                let game = if self.game.product_code == product.code {
                    self.game.clone()
                } else {
                    self.profiles
                        .iter()
                        .find(|g| g.product_code == product.code)
                        .cloned()
                        .unwrap()
                };
                let installed = runner::install_root(&game).is_ok();
                let (card, response) = ui.allocate_exact_size(
                    egui::vec2(width, width * 9.0 / 16.0 + 74.0),
                    Sense::click(),
                );
                let p = ui.painter();
                let picture = Rect::from_min_size(card.min, egui::vec2(width, width * 9.0 / 16.0));
                p.rect_filled(picture, 4, Color32::from_rgb(14, 17, 23));
                image(p, self.game_art_for(product.code), picture);
                p.rect_filled(
                    Rect::from_min_max(picture.left_bottom(), card.max),
                    4,
                    PANEL,
                );
                let title = p.layout(product.name.into(), bold(14.0), WHITE, width - 28.0);
                p.galley(picture.left_bottom() + egui::vec2(14.0, 10.0), title, WHITE);
                let label = if installed {
                    "Installed"
                } else {
                    catalog::access(product.code).label()
                };
                let requirement = p.layout(
                    label.into(),
                    FontId::proportional(11.0),
                    MUTED,
                    width - 28.0,
                );
                p.galley(
                    picture.left_bottom() + egui::vec2(14.0, 42.0),
                    requirement,
                    MUTED,
                );
                if response.hovered() {
                    p.rect_stroke(card, 4, Stroke::new(1.0, ACCENT), StrokeKind::Inside);
                }
                if response.clicked() {
                    self.select_game(product.code);
                    self.tab = 0;
                }
                response.context_menu(|ui| self.game_menu(ui, product.code));
            }
        });
    }
    #[allow(dead_code)]
    fn game_details_sidebar(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let chat_button = Rect::from_min_size(rect.min, egui::vec2(rect.width(), 38.0));
        let label = if self.chat.unread > 0 {
            format!("Chats · {} unread", self.chat.unread)
        } else {
            "Chats and friends".into()
        };
        if ui
            .put(chat_button, egui::Button::new(label).fill(PANEL))
            .clicked()
        {
            self.chat.open = true;
            self.chat.unread = 0;
        }
        let rect = Rect::from_min_max(rect.min + egui::vec2(0.0, 58.0), rect.max);
        let p = ui.painter().clone();
        let x = rect.left();
        let w = rect.width();
        type_at(
            &p,
            rect.min + egui::vec2(1.0, 6.0),
            "YOUR GAME",
            10.0,
            SUBTLE,
            true,
        );
        let picture = Rect::from_min_size(rect.min + egui::vec2(0.0, 34.0), egui::vec2(w, 116.0));
        if matches!(self.game.product_code.as_str(), "S2" | "S1" | "Hero") {
            cover(&p, self.game_art(), picture);
        } else {
            p.rect_filled(picture, 4, Color32::from_rgb(25, 39, 57));
            image(
                &p,
                self.game_icon(&self.game.product_code),
                Rect::from_center_size(picture.center(), egui::vec2(72.0, 72.0)),
            );
        }
        shade(&p, picture);
        type_at(
            &p,
            egui::pos2(x, picture.bottom() + 20.0),
            &self.game.name,
            16.0,
            WHITE,
            true,
        );
        type_at(
            &p,
            egui::pos2(x, picture.bottom() + 49.0),
            "Blizzard Entertainment",
            12.0,
            MUTED,
            false,
        );
        let rows = [
            ("Region", region_name(&self.game.region).to_owned()),
            (
                "Build",
                self.build_label
                    .clone()
                    .unwrap_or_else(|| "Not configured".into()),
            ),
            (
                "Runtime",
                if self.game.proton.is_empty() {
                    "Wine".into()
                } else if self.game.proton.contains("cachyos") {
                    "Proton-CachyOS".into()
                } else {
                    "Proton".into()
                },
            ),
        ];
        let mut y = picture.bottom() + 87.0;
        for (label, value) in rows {
            type_at(&p, egui::pos2(x, y), label, 12.0, SUBTLE, false);
            p.text(
                egui::pos2(rect.right(), y),
                Align2::RIGHT_TOP,
                value,
                FontId::proportional(12.0),
                MUTED,
            );
            y += 31.0;
        }
        rule(
            &p,
            egui::pos2(x, y + 5.0),
            egui::pos2(rect.right(), y + 5.0),
        );
        y += 31.0;
        type_at(&p, egui::pos2(x, y), "MANAGE", 10.0, SUBTLE, true);
        y += 20.0;
        if side_link(
            ui,
            Rect::from_min_size(egui::pos2(x - 10.0, y), egui::vec2(w + 20.0, 40.0)),
            "Game settings",
        ) {
            self.app_settings = false;
            self.diagnostics = false;
            self.settings = true;
        }
        y += 42.0;
        if side_link(
            ui,
            Rect::from_min_size(egui::pos2(x - 10.0, y), egui::vec2(w + 20.0, 40.0)),
            "Open game folder",
        ) {
            match runner::install_root(&self.game) {
                Ok(root) => {
                    if let Err(e) = runner::host_command("xdg-open").arg(root).spawn() {
                        self.status = format!("Could not open folder: {e}");
                    }
                }
                Err(e) => self.status = e,
            }
        }
        y += 42.0;
        if side_link(
            ui,
            Rect::from_min_size(egui::pos2(x - 10.0, y), egui::vec2(w + 20.0, 40.0)),
            "Launch details",
        ) {
            self.open_details();
        }
        let auth = Rect::from_min_size(egui::pos2(x, rect.bottom() - 38.0), egui::vec2(w, 38.0));
        if action(
            ui,
            auth,
            if self.signed_in_this_session {
                "Sign out"
            } else {
                "Sign in"
            },
            false,
            self.pending.is_none() && !self.selected_running(),
        ) {
            if self.signed_in_this_session {
                self.signed_in_this_session = false;
                self.account = None;
                let game = self.game.clone();
                self.job(move || Completed::Logout(runner::sign_out_account(&game)));
            } else {
                self.login(ui.ctx());
            }
        }
    }
    fn open_details(&mut self) {
        self.settings = false;
        self.app_settings = false;
        self.log = runner::read_log(&self.game).unwrap_or_else(|e| e.to_string());
        self.diagnostics = true;
    }
}
pub(super) fn region_name(region: &str) -> &str {
    match region {
        "EU" => "Europe",
        "US" => "Americas",
        "KR" => "Korea",
        "CN" => "China",
        _ => region,
    }
}

const LOCALES: &[(&str, &str)] = &[
    ("enUS", "English"),
    ("deDE", "Deutsch"),
    ("frFR", "Français"),
    ("esES", "Español"),
    ("esMX", "Español (Latinoamérica)"),
    ("itIT", "Italiano"),
    ("plPL", "Polski"),
    ("ptBR", "Português"),
    ("ruRU", "Русский"),
    ("koKR", "한국어"),
    ("zhTW", "繁體中文"),
    ("zhCN", "简体中文"),
    ("jaJP", "日本語"),
];
fn locale_name(code: &str) -> &str {
    LOCALES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, s)| *s)
        .unwrap_or(code)
}
fn settings_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(BG)
        .inner_margin(24)
        .corner_radius(8)
        .stroke(Stroke::new(1.0, LINE))
}
fn settings_header(ui: &mut egui::Ui, title: &str, subtitle: &str, open: &mut bool) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(title).font(bold(24.0)).color(WHITE));
            ui.label(RichText::new(subtitle).size(13.0).color(MUTED));
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_sized([32.0, 32.0], egui::Button::new("×").frame(false))
                .on_hover_text("Close")
                .clicked()
            {
                *open = false;
            }
        });
    });
    ui.add_space(16.0);
    ui.separator();
}
fn settings_navigation(ui: &mut egui::Ui, labels: &[&str], page: &mut usize) {
    ui.vertical(|ui| {
        ui.set_width(160.0);
        for (i, label) in labels.iter().enumerate() {
            let selected = *page == i;
            let button = egui::Button::new(RichText::new(*label).color(if selected {
                WHITE
            } else {
                MUTED
            }))
            .fill(if selected {
                Color32::from_rgb(33, 55, 79)
            } else {
                BG
            })
            .stroke(Stroke::NONE);
            if ui.add_sized([160.0, 40.0], button).clicked() {
                *page = i;
            }
            ui.add_space(4.0);
        }
    });
}
fn settings_section(ui: &mut egui::Ui, title: &str, description: &str) {
    ui.label(RichText::new(title).font(bold(20.0)).color(WHITE));
    ui.add_space(4.0);
    settings_note(ui, description);
    ui.add_space(20.0);
}
fn settings_note(ui: &mut egui::Ui, text: &str) {
    let width = ui.available_width().min(460.0);
    ui.scope(|ui| {
        ui.set_max_width(width);
        ui.add(egui::Label::new(RichText::new(text).size(13.0).color(MUTED)).wrap());
    });
}

#[allow(dead_code)]
fn settings_field(ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str) -> bool {
    ui.label(RichText::new(label).size(13.0).color(WHITE));
    let changed = ui
        .add_sized(
            [ui.available_width(), 34.0],
            egui::TextEdit::singleline(value)
                .hint_text(hint)
                .margin(egui::vec2(10.0, 8.0)),
        )
        .changed();
    ui.add_space(12.0);
    changed
}
fn settings_footer(ui: &mut egui::Ui, open: &mut bool) {
    ui.add_space(20.0);
    ui.separator();
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        settings_note(ui, "Changes are saved automatically.");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_sized(
                    [100.0, 36.0],
                    egui::Button::new(RichText::new("Done").strong().color(Color32::WHITE))
                        .fill(BLUE),
                )
                .clicked()
            {
                *open = false;
            }
        });
    });
}

pub(super) fn game_icons(ctx: &egui::Context) -> Vec<TextureHandle> {
    vec![
        texture(
            ctx,
            "icon-starcraft-ii",
            include_bytes!("../assets/game-icons/starcraft-ii.png"),
        ),
        texture(
            ctx,
            "icon-starcraft-remastered",
            include_bytes!("../assets/game-icons/starcraft-remastered.png"),
        ),
        texture(
            ctx,
            "icon-heroes-of-the-storm",
            include_bytes!("../assets/game-icons/heroes-of-the-storm.png"),
        ),
        texture(
            ctx,
            "icon-overwatch-2",
            include_bytes!("../assets/game-icons/overwatch-2.png"),
        ),
        texture(
            ctx,
            "icon-warcraft-remastered",
            include_bytes!("../assets/game-icons/warcraft-remastered.png"),
        ),
        texture(
            ctx,
            "icon-warcraft-ii-remastered",
            include_bytes!("../assets/game-icons/warcraft-ii-remastered.png"),
        ),
        texture(
            ctx,
            "icon-warcraft-iii",
            include_bytes!("../assets/game-icons/warcraft-iii.png"),
        ),
        texture(
            ctx,
            "icon-world-of-warcraft",
            include_bytes!("../assets/game-icons/world-of-warcraft.png"),
        ),
        texture(
            ctx,
            "icon-hearthstone",
            include_bytes!("../assets/game-icons/hearthstone.png"),
        ),
        texture(
            ctx,
            "icon-diablo-ii",
            include_bytes!("../assets/game-icons/diablo-ii.png"),
        ),
        texture(
            ctx,
            "icon-diablo-iii",
            include_bytes!("../assets/game-icons/diablo-iii.png"),
        ),
        texture(
            ctx,
            "icon-diablo-iv",
            include_bytes!("../assets/game-icons/diablo-iv.png"),
        ),
    ]
}

fn shop_link(code: &str) -> &'static str {
    match code {
        "S2" => "https://shop.battle.net/product/starcraft-ii",
        "S1" => "https://shop.battle.net/product/starcraft-remastered",
        "Hero" => "https://shop.battle.net/family/heroes-of-the-storm",
        "Pro" => "https://shop.battle.net/family/overwatch",
        "W3" => "https://shop.battle.net/family/warcraft-iii",
        "W1R" | "W2R" => "https://shop.battle.net/family/warcraft",
        "WoW" => "https://shop.battle.net/family/world-of-warcraft",
        "WTCG" => "https://shop.battle.net/family/hearthstone",
        "OSI" => "https://shop.battle.net/family/diablo-ii",
        "D3" => "https://shop.battle.net/family/diablo-iii",
        "Fen" => "https://shop.battle.net/family/diablo-iv",
        _ => "https://shop.battle.net/",
    }
}

fn path_field(ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str) -> (bool, bool) {
    ui.label(RichText::new(label).size(13.0));
    let mut changed = false;
    let mut browse = false;
    ui.horizontal(|ui| {
        let width = (ui.available_width() - 92.0).max(100.0);
        changed = ui
            .add_sized(
                [width, 34.0],
                egui::TextEdit::singleline(value)
                    .hint_text(hint)
                    .margin(egui::vec2(10.0, 8.0)),
            )
            .changed();
        browse = ui
            .add_sized([82.0, 34.0], egui::Button::new("Browse…"))
            .clicked();
    });
    ui.add_space(12.0);
    (changed, browse)
}
