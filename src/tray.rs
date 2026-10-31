use eframe::egui;
use ksni::blocking::TrayMethods;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender},
};
#[derive(Debug)]
pub enum Action {
    Show,
    Game(String),
    Quit,
    Unavailable,
}
pub struct Controller {
    pub events: Receiver<Action>,
    available: Arc<AtomicBool>,
}
impl Controller {
    pub fn available(&self) -> bool {
        self.available.load(Ordering::Relaxed)
    }
    pub fn start(context: egui::Context) -> Self {
        let (tx, events) = mpsc::channel();
        let available = Arc::new(AtomicBool::new(false));
        let active = available.clone();
        std::thread::spawn(move || {
            let source = image::load_from_memory(include_bytes!("../assets/battlenet-icon.png"))
                .expect("bundled tray icon");
            // SNI hosts choose a suitable pixmap. Use network-order ARGB and
            // avoid a theme name that is unavailable outside the Flatpak.
            let icons = [16, 22, 24, 32, 48]
                .into_iter()
                .map(|size| {
                    let icon = source
                        .resize_exact(size, size, image::imageops::FilterType::Lanczos3)
                        .to_rgba8();
                    ksni::Icon {
                        width: size as i32,
                        height: size as i32,
                        data: icon
                            .pixels()
                            .flat_map(|p| [p[3], p[0], p[1], p[2]])
                            .collect(),
                    }
                })
                .collect();
            let failure_tx = tx.clone();
            let failure_context = context.clone();
            let tray = Item {
                tx,
                context,
                available: active,
                icons,
            };
            let builder = tray
                .assume_sni_available(true)
                .disable_dbus_name(std::env::var_os("FLATPAK_ID").is_some());
            match builder.spawn() {
                Ok(_handle) => loop {
                    std::thread::park();
                },
                Err(_) => {
                    eprintln!("Could not register the system tray item.");
                    let _ = failure_tx.send(Action::Unavailable);
                    failure_context.request_repaint();
                }
            }
        });
        Self { events, available }
    }
}
struct Item {
    tx: Sender<Action>,
    context: egui::Context,
    available: Arc<AtomicBool>,
    icons: Vec<ksni::Icon>,
}
impl Item {
    fn send(&self, action: Action) {
        let _ = self.tx.send(action);
        self.context.request_repaint();
    }
}
impl ksni::Tray for Item {
    fn id(&self) -> String {
        "battlenet-linux".into()
    }
    fn title(&self) -> String {
        "Battle.net Linux".into()
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.icons.clone()
    }
    fn activate(&mut self, _: i32, _: i32) {
        self.send(Action::Show);
    }
    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "Battle.net Linux".into(),
            description: "Blizzard game launcher".into(),
            icon_pixmap: self.icon_pixmap(),
            ..Default::default()
        }
    }
    fn watcher_online(&self) {
        self.available.store(true, Ordering::Relaxed);
        self.context.request_repaint();
    }
    fn watcher_offline(&self, _: ksni::OfflineReason) -> bool {
        if self.available.swap(false, Ordering::Relaxed) {
            self.send(Action::Unavailable);
        }
        true
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;
        let mut menu = vec![
            StandardItem {
                label: "Show launcher".into(),
                activate: Box::new(|tray: &mut Self| tray.send(Action::Show)),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
        ];
        for product in crate::catalog::PRODUCTS {
            let code = product.code.to_owned();
            menu.push(
                StandardItem {
                    label: product.name.into(),
                    activate: Box::new(move |tray: &mut Self| {
                        tray.send(Action::Game(code.clone()))
                    }),
                    ..Default::default()
                }
                .into(),
            );
        }
        menu.push(ksni::MenuItem::Separator);
        menu.push(
            StandardItem {
                label: "Quit launcher".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|tray: &mut Self| tray.send(Action::Quit)),
                ..Default::default()
            }
            .into(),
        );
        menu
    }
}
