mod auth;
mod catalog;
mod chat;
mod discovery;
mod patcher;
mod runner;
mod tray;
mod ui;
mod updates;

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, TextureHandle};
use std::{
    process::Child,
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};
const BG: Color32 = Color32::from_rgb(20, 22, 28);
const PANEL: Color32 = Color32::from_rgb(32, 35, 43);
const MUTED: Color32 = Color32::from_rgb(153, 159, 172);
const BLUE: Color32 = Color32::from_rgb(0, 120, 240);

enum Completed {
    Login(Result<auth::Account, String>),
    Probe(Result<(), String>),
    Authorization(Result<(), String>),
    Update(Result<(), String>),
    Logout(Result<(), String>),
    Launch(Result<(runner::Game, Child), String>),
    Picked(discovery::Pick, Result<Option<std::path::PathBuf>, String>),
    Discovered(discovery::Found),
}
struct Artwork {
    bnet: TextureHandle,
    bnet_wordmark: TextureHandle,
    hero: TextureHandle,
    starcraft: TextureHandle,
    heroes: TextureHandle,

    game_icons: Vec<TextureHandle>,
    logo: TextureHandle,
    cards: [TextureHandle; 3],
}
fn texture(ctx: &egui::Context, name: &str, bytes: &[u8]) -> TextureHandle {
    let mut img = image::load_from_memory(bytes).expect("bundled artwork");
    let limit = ctx.input(|i| i.max_texture_side).min(2048) as u32;
    if img.width() > limit || img.height() > limit {
        img = img.thumbnail(limit, limit);
    }
    let img = img.to_rgba8();
    ctx.load_texture(
        name,
        egui::ColorImage::from_rgba_unmultiplied(
            [img.width() as usize, img.height() as usize],
            &img,
        ),
        egui::TextureOptions::LINEAR,
    )
}
struct Launcher {
    game: runner::Game,
    discovered: Option<discovery::Found>,
    settings_message: Option<String>,
    art: Artwork,
    children: Vec<(runner::Game, Child)>,
    tray: tray::Controller,
    close_to_tray: bool,
    force_quit: bool,
    browser_job: Option<Receiver<Result<(), String>>>,
    chat: chat::State,
    profiles: Vec<runner::Game>,
    existing_game: bool,
    next_process_check: Instant,
    pending: Option<Receiver<Completed>>,
    status: String,
    version_check: Option<Receiver<(String, Result<updates::Status, String>)>>,
    version_status: Option<updates::Status>,
    version_error: Option<String>,
    version_key: String,
    update_requested: Option<String>,
    patch_progress: Option<Receiver<patcher::Progress>>,
    patch_cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    patch_can_cancel: bool,
    patch_fraction: Option<f32>,
    patch_retry_at: Instant,
    next_version_check: Instant,
    settings: bool,
    app_settings: bool,
    diagnostics: bool,
    log: String,
    signed_in_this_session: bool,
    account: Option<auth::Account>,
    library: bool,
    tab: usize,
    build_label: Option<String>,
}
impl Launcher {
    fn new(cc: &eframe::CreationContext<'_>, profile: Option<runner::Game>) -> Self {
        let ctx = &cc.egui_ctx;
        let mut style = egui::Style::default();
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = BG;
        style.visuals.window_fill = PANEL;
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(43, 46, 55);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(58, 63, 76);
        style.visuals.selection.bg_fill = BLUE;
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(14.0, 10.0);
        style.spacing.menu_margin = egui::Margin::same(8);
        style.spacing.interact_size.y = 32.0;
        style.spacing.combo_width = 190.0;
        style.visuals.window_corner_radius = egui::CornerRadius::same(8);
        style.visuals.menu_corner_radius = egui::CornerRadius::same(6);
        style.visuals.window_stroke = egui::Stroke::new(1.0, Color32::from_rgb(58, 63, 76));
        style.visuals.widgets.noninteractive.bg_stroke =
            egui::Stroke::new(1.0, Color32::from_rgb(58, 63, 76));
        style.visuals.popup_shadow = egui::epaint::Shadow {
            offset: [0, 4],
            blur: 16,
            spread: 0,
            color: Color32::from_black_alpha(90),
        };

        ctx.set_global_style(style);
        ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(egui::SystemTheme::Dark));
        ui::setup(ctx);
        let mut game: runner::Game = profile.unwrap_or_else(|| {
            cc.storage
                .and_then(|s| s.get_string("game"))
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        });
        // Recover a configured local profile when an older GUI save has empty paths.
        if let Ok(bytes) =
            std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/launcher.local.json"))
        {
            if let Ok(local) = serde_json::from_slice::<runner::Game>(&bytes) {
                if local.product_code == game.product_code {
                    if game.executable.is_empty()
                        && std::path::Path::new(&local.executable).is_file()
                    {
                        game.executable = local.executable.clone();
                        game.install_dir = local.install_dir.clone();
                    }
                    if game.prefix.is_empty()
                        && std::path::Path::new(&local.prefix)
                            .join("system.reg")
                            .is_file()
                    {
                        game.prefix = local.prefix.clone();
                        if game.proton.is_empty() {
                            game.proton = local.proton.clone();
                            game.umu = local.umu.clone();
                        }
                    }
                }
            }
        }
        let saved = cc
            .storage
            .and_then(|s| s.get_string("games"))
            .and_then(|s| serde_json::from_str::<Vec<runner::Game>>(&s).ok())
            .unwrap_or_default();
        let mut profiles = catalog::profiles(&game, saved);
        if let Ok(bytes) =
            std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/launcher.local.json"))
        {
            if let Ok(local) = serde_json::from_slice::<runner::Game>(&bytes) {
                for profile in &mut profiles {
                    if profile.product_code == local.product_code
                        && !std::path::Path::new(&profile.executable).is_file()
                        && std::path::Path::new(&local.executable).is_file()
                    {
                        profile.executable = local.executable.clone();
                        profile.install_dir = local.install_dir.clone();
                    }
                    if !std::path::Path::new(&profile.prefix)
                        .join("system.reg")
                        .is_file()
                        && std::path::Path::new(&local.prefix)
                            .join("system.reg")
                            .is_file()
                    {
                        profile.prefix = local.prefix.clone();
                        if profile.runtime_kind == "auto" && profile.proton.is_empty() {
                            profile.proton = local.proton.clone();
                            profile.umu = local.umu.clone();
                        }
                    }
                }
            }
        }

        if let Some(slot) = profiles
            .iter_mut()
            .find(|g| g.product_code == game.product_code)
        {
            if slot.executable.is_empty() {
                slot.executable = game.executable.clone();
                slot.install_dir = game.install_dir.clone();
            }
            if slot.prefix.is_empty() {
                slot.prefix = game.prefix.clone();
            }
            if slot.umu.is_empty() {
                slot.umu = game.umu.clone();
            }
            if slot.proton.is_empty() && slot.runtime_kind == "auto" {
                slot.proton = game.proton.clone();
            }
        }
        let selected = cc
            .storage
            .and_then(|s| s.get_string("selected-product"))
            .unwrap_or_else(|| game.product_code.clone());
        let game = profiles
            .iter()
            .find(|g| g.product_code == selected)
            .cloned()
            .unwrap_or(game);
        let build_label =
            runner::installed_build(&game).map(|b| b.trim_start_matches("Base").to_owned());
        let remembered = runner::remembered_account();
        let has_account = remembered.is_some();
        let mut app = Self {
            build_label,
            discovered: None,
            settings_message: None,
            game,
            art: Artwork {
                bnet: texture(
                    ctx,
                    "battlenet-icon",
                    include_bytes!("../assets/battlenet-icon.png"),
                ),
                bnet_wordmark: texture(
                    ctx,
                    "battlenet-wordmark",
                    include_bytes!("../assets/battlenet-horizontal.png"),
                ),
                hero: texture(ctx, "hero", include_bytes!("../assets/hero.png")),
                starcraft: texture(
                    ctx,
                    "starcraft-remastered",
                    include_bytes!("../assets/starcraft-remastered.image"),
                ),
                heroes: texture(ctx, "heroes", include_bytes!("../assets/heroes.image")),
                logo: texture(ctx, "logo", include_bytes!("../assets/logo.png")),
                game_icons: ui::game_icons(ctx),
                cards: [
                    texture(ctx, "races", include_bytes!("../assets/races.png")),
                    texture(ctx, "coop", include_bytes!("../assets/coop.png")),
                    texture(
                        ctx,
                        "multiplayer",
                        include_bytes!("../assets/multiplayer.png"),
                    ),
                ],
            },
            children: Vec::new(),
            tray: tray::Controller::start(ctx.clone()),
            close_to_tray: cc
                .storage
                .and_then(|s| s.get_string("close-to-tray"))
                .map(|s| s == "true")
                .unwrap_or(true),
            force_quit: false,
            browser_job: None,
            chat: chat::State::default(),
            profiles,
            existing_game: false,
            next_process_check: Instant::now(),
            pending: None,
            status: String::new(),
            version_check: None,
            version_status: None,
            version_error: None,
            version_key: String::new(),
            update_requested: None,
            patch_progress: None,
            patch_cancel: None,
            patch_can_cancel: false,
            patch_fraction: None,
            patch_retry_at: Instant::now(),
            next_version_check: Instant::now(),
            settings: false,
            app_settings: false,
            diagnostics: false,
            log: String::new(),
            signed_in_this_session: has_account,
            account: remembered,
            library: has_account || std::env::args().any(|arg| arg == "--library"),
            tab: 0,
        };
        if has_account {
            app.enter_library(ctx);
        }
        if cfg!(debug_assertions) {
            if std::env::args().any(|arg| arg == "--preview-shop") {
                app.tab = 2;
            }
            app.chat.open = std::env::args().any(|arg| arg == "--preview-chat");
            app.settings = std::env::args().any(|arg| arg == "--preview-game-settings");
            app.app_settings = std::env::args().any(|arg| arg == "--preview-launcher-settings");
        }
        if std::env::args().any(|arg| arg == "--login") {
            app.login(ctx);
        }
        app
    }
    fn selected_running(&self) -> bool {
        self.existing_game
            || self
                .children
                .iter()
                .any(|(game, _)| game.product_code == self.game.product_code)
    }
    fn select_game(&mut self, code: &str) {
        if self.pending.is_some() {
            return;
        }
        if let Some(current) = self
            .profiles
            .iter_mut()
            .find(|g| g.product_code == self.game.product_code)
        {
            *current = self.game.clone();
        }
        let Some(game) = self
            .profiles
            .iter()
            .find(|g| g.product_code == code)
            .cloned()
        else {
            return;
        };
        self.game = game;
        self.discovered = None;
        self.settings_message = None;
        if self.diagnostics {
            self.log = runner::read_log(&self.game).unwrap_or_else(|e| e.to_string());
        }
        self.status.clear();
        self.version_status = None;
        self.version_error = None;
        self.version_key.clear();
        self.next_version_check = Instant::now();
        self.next_process_check = Instant::now();
        self.update_requested = None;
        self.existing_game = runner::game_running(&self.game);
        self.build_label =
            runner::installed_build(&self.game).map(|s| s.trim_start_matches("Base").into());
    }
    fn job(&mut self, work: impl FnOnce() -> Completed + Send + 'static) {
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(work());
        });
    }
    fn enter_library(&mut self, ctx: &egui::Context) {
        self.library = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(1440.0, 940.0)));
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(
            960.0, 700.0,
        )));
    }
    fn enter_login(&mut self, ctx: &egui::Context) {
        self.library = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(
            420.0, 700.0,
        )));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(460.0, 760.0)));
    }
    fn choose_path(&mut self, kind: discovery::Pick) {
        if self.pending.is_some() || self.selected_running() {
            return;
        }
        let current = match kind {
            discovery::Pick::GameFolder => self.game.install_dir.clone(),
            discovery::Pick::GameExe => self.game.executable.clone(),
            discovery::Pick::Wine => self.game.runtime.clone(),
            discovery::Pick::Prefix => self.game.prefix.clone(),
            discovery::Pick::Proton => self.game.proton.clone(),
            discovery::Pick::Umu => self.game.umu.clone(),
        };
        self.job(move || Completed::Picked(kind, discovery::pick(kind, &current)));
    }
    fn discover(&mut self) {
        if self.pending.is_none() {
            let game = self.game.clone();
            self.settings_message =
                Some("Looking for installations and Wine/Proton runners…".into());
            self.job(move || Completed::Discovered(discovery::scan(&game)));
        }
    }
    fn start_update(&mut self) {
        self.start_patch(false);
    }
    fn start_patch(&mut self, repair: bool) {
        if self.pending.is_some() || self.selected_running() || self.existing_game {
            return;
        }
        if let Some(status) = &self.version_status {
            self.update_requested = Some(format!(
                "{}|{}|{}",
                self.game.executable, self.game.region, status.latest.build
            ));
        }
        let game = self.game.clone();
        self.status = "Planning game update…".into();
        let (tx, rx) = mpsc::channel();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.patch_progress = Some(rx);
        self.patch_cancel = Some(cancel.clone());
        self.patch_can_cancel = true;
        self.patch_fraction = None;
        self.job(move || {
            Completed::Update(
                if repair {
                    patcher::repair(&game, cancel, |progress| {
                        let _ = tx.send(progress);
                    })
                } else {
                    patcher::run(
                        &game,
                        cancel,
                        |progress| {
                            let _ = tx.send(progress);
                        },
                        false,
                    )
                }
                .map(|_| ()),
            )
        });
    }
    fn check_updates(&mut self) {
        if self.version_check.is_some() || runner::install_root(&self.game).is_err() {
            return;
        }
        let game = self.game.clone();
        let key = updates::installation_key(&game);
        let (tx, rx) = mpsc::channel();
        self.version_check = Some(rx);
        self.next_version_check = Instant::now() + Duration::from_secs(30 * 60);
        std::thread::spawn(move || {
            let _ = tx.send((key, updates::check(&game)));
        });
    }
    fn poll(&mut self, ctx: &egui::Context) {
        self.chat.poll();
        if self.account.is_none() && self.chat.connection.is_some() {
            self.chat.disconnect();
        }
        if let Some(rx) = &self.browser_job {
            if let Ok(result) = rx.try_recv() {
                self.browser_job = None;
                self.status = match result {
                    Ok(()) => "Opened in your browser.".into(),
                    Err(e) => e,
                };
            }
        }

        while let Ok(action) = self.tray.events.try_recv() {
            match action {
                tray::Action::Show | tray::Action::Unavailable => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                tray::Action::Game(code) => {
                    self.select_game(&code);
                    self.tab = 0;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                tray::Action::Quit => {
                    if let Some(cancel) = &self.patch_cancel {
                        cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    self.force_quit = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
        if ctx.input(|input| input.viewport().close_requested())
            && self.close_to_tray
            && self.tray.available()
            && !self.force_quit
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        if Instant::now() >= self.next_process_check {
            self.existing_game = runner::game_running(&self.game);
            self.next_process_check = Instant::now() + Duration::from_secs(3);
        }
        if let Some(rx) = &self.patch_progress {
            while let Ok(progress) = rx.try_recv() {
                self.patch_can_cancel = !progress.message.starts_with("Applying verified patch");
                self.patch_fraction =
                    (progress.total > 0).then(|| progress.completed as f32 / progress.total as f32);
                self.status = progress.message;
            }
        }
        let key = updates::installation_key(&self.game);
        if key != self.version_key {
            self.version_key = key.clone();
            self.version_status = None;
            self.version_error = None;
            self.next_version_check = Instant::now();
        }
        if let Some(rx) = &self.version_check {
            match rx.try_recv() {
                Ok((checked_key, result)) => {
                    self.version_check = None;
                    if checked_key == key {
                        match result {
                            Ok(status) => {
                                self.build_label =
                                    Some(status.installed.trim_start_matches("Base").into());
                                self.version_status = Some(status);
                                self.version_error = None;
                            }
                            Err(error) => {
                                self.version_error = Some(error);
                                self.next_version_check = Instant::now() + Duration::from_secs(60);
                            }
                        }
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.version_check = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if Instant::now() >= self.next_version_check {
            self.check_updates();
        }
        ctx.request_repaint_after(Duration::from_secs(1));
        if let Some(rx) = &self.pending {
            match rx.try_recv() {
                Ok(result) => {
                    self.pending = None;
                    match result {
                            Completed::Discovered(found) => {
                                self.settings_message = Some(format!(
                                    "Found {} game installation(s), {} runner(s), and {} Wine prefix(es).",
                                    found.installations.len(),
                                    found.runtimes.len(),
                                    found.prefixes.len()
                                ));
                                self.discovered = Some(found);
                            }
                            Completed::Picked(kind, result) => {
                                self.settings_message = Some(match result {
                                    Ok(Some(path)) => {
                                        match discovery::apply(&mut self.game, kind, &path) {
                                            Ok(message) => {
                                                if runner::uses_proton(&self.game)
                                                    && self.game.umu.is_empty()
                                                {
                                                    if let Some(umu) = self
                                                        .discovered
                                                        .as_ref()
                                                        .and_then(|d| d.umu.as_ref())
                                                    {
                                                        self.game.umu =
                                                            umu.to_string_lossy().into();
                                                    }
                                                }
                                                self.build_label =
                                                    runner::installed_build(&self.game).map(|s| {
                                                        s.trim_start_matches("Base").into()
                                                    });
                                                self.next_version_check = Instant::now();
                                                message
                                            }
                                            Err(error) => error,
                                        }
                                    }
                                    Ok(None) => "Selection cancelled; paths were kept.".into(),
                                    Err(error) => error,
                                });
                            }


                        Completed::Login(Ok(account)) => {
                            self.account = Some(account);
                            self.signed_in_this_session = true;
                            self.status = "Signed in to Battle.net.".into();
                            self.enter_library(ctx);
                        }
                        Completed::Update(Ok(())) => {
                            self.status = "Game update completed.".into();
                            self.patch_progress = None; self.patch_cancel = None; self.patch_fraction = None;
                            self.next_version_check = Instant::now();
                        }
                        Completed::Authorization(Ok(())) => {
                            self.status = "Game authorization refreshed. Press Play to start.".into();
                        }
                        Completed::Probe(Ok(())) => {
                            self.status = "Saved game authorization is readable.".into();
                        }
                        Completed::Logout(Ok(())) => {
                            self.account = None;
                            self.signed_in_this_session = false;
                            self.status = "Signed out of Battle.net.".into();
                        }
                        Completed::Launch(Ok((game,child))) => {
                            self.status = format!("{} started.",game.name);
                            self.children.push((game,child));
                        }
                        Completed::Login(Err(e))
                        | Completed::Logout(Err(e))
                        | Completed::Launch(Err(e))
                        | Completed::Authorization(Err(e)) => self.status = e,
                        Completed::Update(Err(error)) => {
                            self.status = error;
                            self.patch_progress = None; self.patch_cancel = None; self.patch_fraction = None;
                            self.patch_retry_at = Instant::now() + Duration::from_secs(5 * 60);
                            if !self.status.starts_with("Update cancelled") { self.update_requested = None; }
                        }
                        Completed::Probe(Err(_)) => {
                            self.status = "No saved game authorization. It will be requested when you press Play.".into()
                        }
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.pending = None;
                    self.status = "Background operation stopped unexpectedly.".into();
                    self.patch_progress = None;
                    self.patch_cancel = None;
                    self.patch_fraction = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let mut index = 0;
        while index < self.children.len() {
            let exit = self.children[index].1.try_wait();
            match exit {
                Ok(Some(exit)) => {
                    let (game, _) = self.children.remove(index);
                    runner::record_game_exit(&game, exit);
                    if game.product_code == self.game.product_code {
                        self.status = if exit.success() {
                            format!("{} closed.", game.name)
                        } else {
                            format!(
                                "{} stopped (exit code {}). Open Launch details for its runtime log.",
                                game.name,
                                exit.code()
                                    .map(|c| c.to_string())
                                    .unwrap_or_else(|| "unknown".into())
                            )
                        };
                    }
                }
                Err(error) => {
                    self.status = format!("Process status unavailable: {error}");
                    index += 1;
                }
                _ => index += 1,
            }
        }
        if self.game.auto_update
            && self.pending.is_none()
            && !self.selected_running()
            && !self.existing_game
            && Instant::now() >= self.patch_retry_at
        {
            if let Some(version) = &self.version_status {
                let key = format!(
                    "{}|{}|{}",
                    self.game.executable, self.game.region, version.latest.build
                );
                if version.needs_update() && self.update_requested.as_ref() != Some(&key) {
                    self.start_update();
                }
            }
        }
        if self.pending.is_some() || self.selected_running() {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }
    fn login(&mut self, _ctx: &egui::Context) {
        let Some(helper) = runner::login_helper() else {
            self.status =
                "The Battle.net sign-in component is missing. Reinstall the complete launcher."
                    .into();
            return;
        };
        let region = self.game.region.clone();
        self.status = "Opening Battle.net sign-in…".into();
        self.job(move || Completed::Login(runner::login_account(&region, &helper)));
    }
    fn play(&mut self, ctx: &egui::Context) {
        if self
            .version_status
            .as_ref()
            .is_some_and(|status| status.needs_update())
        {
            self.settings = true;
            self.app_settings = false;
            self.diagnostics = false;
            self.status = format!(
                "Update required: {}. Patch the installation before playing.",
                self.version_status.as_ref().unwrap().latest.name
            );
            return;
        }
        if !self.signed_in_this_session {
            self.enter_login(ctx);
            return;
        }
        if let Err(e) = runner::install_root(&self.game) {
            self.status = e;
            self.settings = true;
            return;
        }
        let Some(account) = self.account.clone() else {
            self.login(ctx);
            return;
        };
        let Some(helper) = runner::login_helper() else {
            self.status = "Battle.net sign-in component is missing.".into();
            return;
        };
        let game = self.game.clone();
        self.status = format!("Starting {}…", game.name);
        self.job(move || {
            Completed::Launch(
                runner::authorize_game(&game, &account, &helper)
                    .and_then(|_| runner::launch(&game))
                    .map(|child| (game, child)),
            )
        });
    }
}
impl eframe::App for Launcher {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("close-to-tray", self.close_to_tray.to_string());
        if let Some(current) = self
            .profiles
            .iter_mut()
            .find(|g| g.product_code == self.game.product_code)
        {
            *current = self.game.clone();
        }
        if let Ok(json) = serde_json::to_string(&self.profiles) {
            storage.set_string("games", json);
        }
        storage.set_string("selected-product", self.game.product_code.clone());
        if let Ok(json) = serde_json::to_string(&self.game) {
            storage.set_string("game", json);
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll(ui.ctx());
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG))
            .show(ui, |ui| {
                if self.library {
                    self.library_view(ui);
                } else {
                    self.login_view(ui);
                }
            });
        if self.app_settings {
            self.app_settings_view(ui.ctx());
        }
        if self.settings {
            self.settings_view(ui.ctx());
        }
        if self.chat.open {
            self.chat_view(ui.ctx());
        }
        if self.diagnostics {
            egui::Window::new(format!("Launch details · {}", self.game.name))
                .id(egui::Id::new("launch-details-window"))
                .open(&mut self.diagnostics)
                .default_width(650.0)
                .show(ui.ctx(), |ui| {
                    ui.label(runner::game_log_path(&self.game).display().to_string());
                    if ui.button("Refresh").clicked() {
                        self.log = runner::read_log(&self.game).unwrap_or_else(|e| e.to_string());
                    }
                    ui.add(
                        egui::TextEdit::multiline(&mut self.log)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(18)
                            .desired_width(f32::INFINITY)
                            .interactive(false),
                    );
                });
        }
    }
}
fn brand(p: &egui::Painter, pos: Pos2, full: bool, art: &Artwork) {
    let (texture, size) = if full {
        (&art.bnet_wordmark, egui::vec2(270.0, 45.0))
    } else {
        (&art.bnet, egui::vec2(36.0, 36.0))
    };
    p.image(
        texture.id(),
        Rect::from_min_size(pos, size),
        Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
}
fn cover(p: &egui::Painter, texture: &TextureHandle, rect: Rect) {
    let image = texture.size_vec2();
    let image_ratio = image.x / image.y;
    let ratio = rect.width() / rect.height();
    let uv = if image_ratio > ratio {
        let w = ratio / image_ratio;
        Rect::from_min_max(
            egui::pos2((1.0 - w) / 2.0, 0.0),
            egui::pos2((1.0 + w) / 2.0, 1.0),
        )
    } else {
        let h = image_ratio / ratio;
        Rect::from_min_max(
            egui::pos2(0.0, (1.0 - h) / 2.0),
            egui::pos2(1.0, (1.0 + h) / 2.0),
        )
    };
    p.image(texture.id(), rect, uv, Color32::WHITE);
}
fn shade(p: &egui::Painter, r: Rect) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(r.left_top(), Color32::from_black_alpha(190));
    mesh.colored_vertex(r.right_top(), Color32::from_black_alpha(10));
    mesh.colored_vertex(r.right_bottom(), Color32::from_black_alpha(35));
    mesh.colored_vertex(r.left_bottom(), Color32::from_black_alpha(230));
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(egui::Shape::mesh(mesh));
}
fn main() -> eframe::Result {
    let mut profile = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--profile" {
            let Some(path) = args.next() else {
                eprintln!("--profile requires a JSON file");
                std::process::exit(2);
            };
            let result = std::fs::read(path)
                .map_err(|e| e.to_string())
                .and_then(|bytes| {
                    serde_json::from_slice::<runner::Game>(&bytes).map_err(|e| e.to_string())
                });
            match result {
                Ok(game) => profile = Some(game),
                Err(e) => {
                    eprintln!("Cannot load profile: {e}");
                    std::process::exit(2);
                }
            }
        }
    }
    #[cfg(debug_assertions)]
    if let Some(index) = std::env::args().position(|arg| {
        arg == "--exercise-native-update" || arg == "--exercise-native-update-cancel"
    }) {
        let Some(game) = profile else {
            eprintln!("Native update exercise requires --profile");
            std::process::exit(2);
        };
        let Some(directory) = std::env::args().nth(index + 1) else {
            eprintln!("Native update exercise requires a new fixture directory");
            std::process::exit(2);
        };
        let cancel = std::env::args().any(|arg| arg == "--exercise-native-update-cancel");
        if let Err(error) = patcher::exercise(&game, std::path::Path::new(&directory), cancel) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    if std::env::args().any(|arg| arg == "--plan-update") {
        let Some(game) = profile else {
            eprintln!("--plan-update requires --profile");
            std::process::exit(2);
        };
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        match patcher::run(
            &game,
            cancel,
            |progress| println!("{}", progress.message),
            true,
        ) {
            Ok(summary) => println!(
                "Native patch plan: {} · {} storage files · {} loose files · {} bytes",
                summary.version, summary.storage_files, summary.loose_files, summary.download_bytes
            ),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    if std::env::args().any(|arg| arg == "--check-updates") {
        let Some(game) = profile else {
            eprintln!("--check-updates requires --profile");
            std::process::exit(2);
        };
        match updates::check(&game) {
            Ok(status) => println!(
                "Installed: {} · Latest: {} · Update required: {}",
                status.installed.trim_start_matches("Base"),
                status.latest.name,
                status.needs_update()
            ),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    if std::env::args().any(|arg| arg == "--check-login-window") {
        match runner::check_login_window() {
            Ok(()) => println!("Login window startup passed on the desktop backend."),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    if std::env::args().any(|arg| arg == "--launch-game") {
        eprintln!("Direct launch is disabled. Sign in through the launcher, then press Play.");
        std::process::exit(2);
    }
    if std::env::args().any(|arg| arg == "--check-session") {
        let Some(game) = profile else {
            eprintln!("--check-session requires --profile");
            std::process::exit(2);
        };
        let result = runner::install_root(&game).and_then(|_| runner::probe(&game));
        match result {
            Ok(()) => println!(
                "SC2 ticket can be decrypted in the configured runtime. Online acceptance remains unverified."
            ),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    let library = std::env::args().any(|arg| arg == "--library");
    let icon = image::load_from_memory(include_bytes!("../assets/battlenet-icon.png"))
        .expect("bundled icon")
        .to_rgba8();
    let window_icon = egui::IconData {
        width: icon.width(),
        height: icon.height(),
        rgba: icon.into_raw(),
    };
    eframe::run_native(
        "Battle.net Linux",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_icon(window_icon)
                .with_app_id("io.github.battlenet_linux.Launcher")
                .with_inner_size(if library {
                    [1440.0, 940.0]
                } else {
                    [460.0, 760.0]
                })
                .with_min_inner_size(if library {
                    [960.0, 700.0]
                } else {
                    [420.0, 700.0]
                }),
            ..Default::default()
        },
        Box::new(move |cc| Ok(Box::new(Launcher::new(cc, profile)))),
    )
}
