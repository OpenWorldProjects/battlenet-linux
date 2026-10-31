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
            let icon = image::load_from_memory(include_bytes!("../assets/battlenet-icon.png"))
                .expect("bundled tray icon")
                .to_rgba8();
            let width = icon.width() as i32;
            let height = icon.height() as i32;
            let data = icon
                .pixels()
                .flat_map(|p| [p[3], p[0], p[1], p[2]])
                .collect();
            let tray = Item {
                tx,
                context,
                available: active,
                icon: ksni::Icon {
                    width,
                    height,
                    data,
                },
            };
            let builder = tray
                .assume_sni_available(true)
                .disable_dbus_name(std::env::var_os("FLATPAK_ID").is_some());
            if let Ok(_handle) = builder.spawn() {
                loop {
                    std::thread::park();
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
    icon: ksni::Icon,
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
    fn icon_name(&self) -> String {
        "battlenet-linux".into()
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![self.icon.clone()]
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
