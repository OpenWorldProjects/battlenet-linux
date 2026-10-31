// Separate process: the egui launcher itself does not link GTK or WebKit.
#[path = "../../src/auth.rs"]
mod auth;
use gtk::prelude::*;
use javascriptcore::ValueExt;
use serde::Deserialize;
use std::{
    cell::{Cell, RefCell},
    io::{IsTerminal, Read, Write},
    rc::Rc,
};
use webkit2gtk::{
    CookieManagerExt, NavigationPolicyDecisionExt, PolicyDecisionExt, SettingsExt, URIRequestExt,
    URISchemeRequestExt, WebContextExt, WebViewExt,
};

pub fn login_url(region: &str, product: &str) -> String {
    let host = match region {
        "EU" => "eu.battle.net",
        "KR" => "kr.battle.net",
        "CN" => "account.battlenet.com.cn",
        _ => "us.battle.net",
    };
    let mut url = url::Url::parse(&format!("https://{host}/login/en/")).unwrap();
    url.query_pairs_mut()
        .append_pair("externalChallenge", "login")
        .append_pair("app", product)
        .append_pair("ref", "bnet-linux-login://callback/");
    url.into()
}

fn finish_window() {
    // Navigation signals may fire synchronously before gtk::main starts.
    // Defer closing until the event loop is running instead of panicking.
    gtk::glib::idle_add_local_once(|| {
        if gtk::main_level() > 0 {
            gtk::main_quit();
        }
    });
}
type TicketState = Rc<RefCell<Option<auth::Ticket>>>;
fn capture(uri: &str, ticket: &TicketState, message: &gtk::Label) -> bool {
    if !uri.starts_with("http://localhost:0/") && !uri.starts_with("bnet-linux-login://callback/") {
        return false;
    }
    if ticket.borrow().is_some() {
        return true;
    }
    match auth::Ticket::parse(uri) {
        Ok(value) => {
            *ticket.borrow_mut() = Some(value);
            finish_window();
        }
        Err(_) => {
            eprintln!("BNET_EVENT=callback_rejected");
            message.set_text("Sign-in did not complete. Please retry.");
        }
    }
    // Consume the callback even when invalid; never show its token in a page.
    true
}
fn prepare_view(
    web: &webkit2gtk::WebView,
    ticket: &TicketState,
    message: &gtk::Label,
    popups: &Rc<RefCell<Vec<gtk::Window>>>,
    account: Option<&Rc<RefCell<Option<auth::Account>>>>,
    fixture: bool,
) {
    if let Some(account) = account {
        let result = account.clone();
        let reading = Rc::new(Cell::new(false));
        let status = message.clone();
        web.connect_load_changed(move |web, event| {
            if event == webkit2gtk::LoadEvent::Finished {
                read_account(web, &result, &reading, &status, fixture);
            }
        });
    }
    let game_mode = account.is_none();
    let origin_label = message.clone();
    web.connect_load_changed(move |web, event| {
        if event == webkit2gtk::LoadEvent::Started {
            if let Some(uri) = web.uri() {
                if let Ok(url) = url::Url::parse(&uri) {
                    if url.scheme() == "https" {
                        if let Some(host) = url.host_str() {
                            origin_label.set_text(&format!("Secure sign-in · {host}"));
                        }
                    }
                }
            }
        }
    });
    let result = ticket.clone();
    let status = message.clone();
    web.connect_decide_policy(move |_, decision, kind| {
        if matches!(
            kind,
            webkit2gtk::PolicyDecisionType::NavigationAction
                | webkit2gtk::PolicyDecisionType::NewWindowAction
        ) {
            if let Some(request) = decision
                .downcast_ref::<webkit2gtk::NavigationPolicyDecision>()
                .and_then(|nav| nav.navigation_action())
                .and_then(|action| action.request())
            {
                if let Some(uri) = request.uri() {
                    if game_mode && capture(&uri, &result, &status) {
                        decision.ignore();
                        return true;
                    }
                }
            }
        }
        false
    });
    let result = ticket.clone();
    let status = message.clone();
    web.connect_uri_notify(move |web| {
        if let Some(uri) = web.uri() {
            if game_mode && capture(&uri, &result, &status) {
                web.stop_loading();
            }
        }
    });
    let result = ticket.clone();
    let status = message.clone();
    web.connect_load_failed(move |_, _, uri, _| {
        if game_mode && capture(uri, &result, &status) {
            return true;
        }
        eprintln!("BNET_EVENT=network_failed");
        status.set_text("Could not load sign-in. Check your connection, then Retry.");
        false
    });
    let result = ticket.clone();
    let status = message.clone();
    let windows = popups.clone();
    let account_state = account.cloned();
    web.connect_create(move |parent, _| {
        let popup = webkit2gtk::WebView::with_related_view(parent);
        prepare_view(
            &popup,
            &result,
            &status,
            &windows,
            account_state.as_ref(),
            fixture,
        );
        let window = gtk::Window::new(gtk::WindowType::Toplevel);
        window.set_title("Battle.net · Complete sign-in");
        window.set_default_size(480, 760);
        window.add(&popup);
        window.show_all();
        let weak = window.downgrade();
        popup.connect_close(move |_| {
            if let Some(window) = weak.upgrade() {
                window.close();
            }
        });
        windows.borrow_mut().push(window);
        Some(popup.upcast())
    });
}
#[derive(Deserialize)]
struct AccountReply {
    #[serde(default)]
    authenticated: bool,
    account_id: Option<String>,
    battle_tag: Option<String>,
    error: Option<String>,
}
fn account_origin(uri: &str, fixture: bool) -> bool {
    let Ok(url) = url::Url::parse(uri) else {
        return false;
    };
    (url.scheme() == "https"
        && url.host_str() == Some("account.battle.net")
        && !url.path().starts_with("/oauth2")
        && !url.path().contains("login"))
        || (fixture
            && url.scheme() == "http"
            && url.host_str() == Some("127.0.0.1")
            && url.path() == "/account")
}
fn read_account(
    web: &webkit2gtk::WebView,
    account: &Rc<RefCell<Option<auth::Account>>>,
    reading: &Rc<Cell<bool>>,
    status: &gtk::Label,
    fixture: bool,
) {
    if reading.get()
        || account.borrow().is_some()
        || !web
            .uri()
            .as_deref()
            .is_some_and(|uri| account_origin(uri, fixture))
    {
        return;
    }
    reading.set(true);
    let result = account.clone();
    let busy = reading.clone();
    let message = status.clone();
    let weak = web.downgrade();
    web.call_async_javascript_function(
        include_str!("account.js"),
        None,
        Some("bnet-account"),
        None,
        None::<&gtk::gio::Cancellable>,
        move |value| {
            busy.set(false);
            let Some(web) = weak.upgrade() else {
                return;
            };
            if !web
                .uri()
                .as_deref()
                .is_some_and(|uri| account_origin(uri, fixture))
            {
                return;
            }
            match value
                .ok()
                .and_then(|value| serde_json::from_str::<AccountReply>(&value.to_str()).ok())
            {
                Some(reply) if reply.authenticated => {
                    let profile = auth::Account {
                        account_id: reply.account_id.unwrap_or_default(),
                        battle_tag: reply.battle_tag,
                    };
                    if profile.validate().is_ok() {
                        *result.borrow_mut() = Some(profile);
                        eprintln!("BNET_EVENT=account_verified");
                        finish_window();
                    } else {
                        eprintln!("BNET_EVENT=account_profile_failed");
                        message.set_text("Could not verify the account profile. Please retry.");
                    }
                }
                Some(reply) if reply.error.is_none() => {}
                _ => {
                    eprintln!("BNET_EVENT=account_profile_failed");
                    message.set_text("Could not verify the Battle.net session. Please retry.");
                }
            }
        },
    );
}
fn main() {
    if std::env::args().nth(1).as_deref() == Some("--protocol-version") {
        println!("bnet-login/2");
        return;
    }
    if std::io::stdout().is_terminal() {
        std::process::exit(2);
    }
    if gtk::init().is_err() {
        eprintln!("BNET_EVENT=display_failed");
        std::process::exit(3);
    }
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--choose-path") {
        let folder = args.get(2).map(String::as_str) == Some("folder");
        let title = args.get(3).map(String::as_str).unwrap_or("Choose a path");
        let dialog = gtk::FileChooserNative::new(
            Some(title),
            None::<&gtk::Window>,
            if folder {
                gtk::FileChooserAction::SelectFolder
            } else {
                gtk::FileChooserAction::Open
            },
            Some("Select"),
            Some("Cancel"),
        );
        dialog.set_modal(true);
        dialog.set_select_multiple(false);
        if let Some(path) = args.get(4).map(std::path::Path::new) {
            if path.is_dir() {
                dialog.set_current_folder(path);
            } else if let Some(parent) = path.parent() {
                if parent.is_dir() {
                    dialog.set_current_folder(parent);
                }
            }
        }
        let selected = if dialog.run() == gtk::ResponseType::Accept {
            dialog.filename().map(|p| p.to_string_lossy().into_owned())
        } else {
            None
        };
        println!("{}", serde_json::to_string(&selected).unwrap());
        return;
    }
    let region = args.get(1).cloned().unwrap_or_else(|| "EU".into());
    if !matches!(region.as_str(), "EU" | "US" | "KR" | "CN") {
        std::process::exit(2);
    }
    let argument = |name: &str| {
        args.iter()
            .position(|arg| arg == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let fixture = cfg!(debug_assertions) && args.iter().any(|arg| arg.starts_with("--test-"));
    let product = argument("--game");
    if product.as_ref().is_some_and(|code| {
        !matches!(
            code.as_str(),
            "app"
                | "W1R"
                | "W2R"
                | "S2"
                | "WoW"
                | "WTCG"
                | "D3"
                | "Hero"
                | "W3"
                | "Fen"
                | "Pro"
                | "OSI"
                | "S1"
        )
    }) {
        std::process::exit(2);
    }
    let account_mode =
        product.is_none() && (!fixture || args.iter().any(|arg| arg == "--test-account"));
    let chat_mode = product.as_deref() == Some("app");
    let challenge = if args.iter().any(|a| a == "--chat-start-stdin") {
        if !chat_mode {
            std::process::exit(2);
        }
        let mut raw = zeroize::Zeroizing::new(String::new());
        if std::io::stdin()
            .take(8193)
            .read_to_string(&mut raw)
            .is_err()
            || raw.len() > 8192
        {
            std::process::exit(2);
        }
        let Ok(mut url) = url::Url::parse(&raw) else {
            std::process::exit(2);
        };
        if url.scheme() != "https"
            || url.port().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
            || !matches!(
                url.host_str(),
                Some(
                    "eu.account.battle.net"
                        | "us.account.battle.net"
                        | "kr.account.battle.net"
                        | "account.battle.net"
                        | "eu.battle.net"
                        | "us.battle.net"
                        | "kr.battle.net"
                )
            )
            || !url.path().starts_with("/login/")
        {
            std::process::exit(2);
        }
        let pairs: Vec<_> = url
            .query_pairs()
            .filter(|(key, _)| key != "ref")
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        if !pairs
            .iter()
            .any(|(k, v)| k == "app" && v.eq_ignore_ascii_case("app"))
        {
            std::process::exit(2);
        }
        url.set_query(None);
        {
            let mut query = url.query_pairs_mut();
            for (k, v) in pairs {
                query.append_pair(&k, &v);
            }
            query.append_pair("ref", "bnet-linux-login://callback/");
        }
        Some(url.to_string())
    } else {
        None
    };
    let start = if let Some(challenge) = challenge {
        challenge
    } else if account_mode {
        "https://account.battle.net/oauth2/authorization/account-settings".into()
    } else {
        login_url(&region, product.as_deref().unwrap_or("S2"))
    };
    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_application_prefer_dark_theme(true);
    }
    let css = gtk::CssProvider::new();
    let _=css.load_from_data(b".bnet-login-content { background-color:#14161c; } .bnet-login-status { color:#a3a9b6; } .bnet-login-controls button { background-image:none; background-color:#272a34; color:#e7eaf0; border:1px solid #3a3f4a; border-radius:4px; box-shadow:none; } .bnet-login-controls button:hover { background-color:#343946; }");
    if let Some(screen) = gtk::gdk::Screen::default() {
        gtk::StyleContext::add_provider_for_screen(
            &screen,
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    let context = if let Some(directory) = argument("--session-dir") {
        let directory = std::path::Path::new(&directory);
        let manager = webkit2gtk::WebsiteDataManager::builder()
            .base_data_directory(directory.join("data").to_string_lossy())
            .base_cache_directory(directory.join("cache").to_string_lossy())
            .build();
        let context = webkit2gtk::WebContext::with_website_data_manager(&manager);
        if let Some(cookies) = context.cookie_manager() {
            cookies.set_persistent_storage(
                &directory.join("cookies.sqlite").to_string_lossy(),
                webkit2gtk::CookiePersistentStorage::Sqlite,
            );
        }
        context
    } else {
        webkit2gtk::WebContext::new_ephemeral()
    };
    let web = webkit2gtk::WebView::with_context(&context);
    if let Some(settings) = WebViewExt::settings(&web) {
        settings.set_hardware_acceleration_policy(webkit2gtk::HardwareAccelerationPolicy::Never);
        settings.set_enable_write_console_messages_to_stdout(false);
    }
    let ticket = Rc::new(RefCell::new(None));
    let account = Rc::new(RefCell::new(None));
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    if let Ok(icon) = gtk::gdk_pixbuf::Pixbuf::from_read(std::io::Cursor::new(include_bytes!(
        "../../assets/battlenet-icon.png"
    ))) {
        window.set_icon(Some(&icon));
    }
    window.set_title(if account_mode {
        "Battle.net · Log in"
    } else if chat_mode {
        "Battle.net · Authorize chat"
    } else {
        "Battle.net · Authorize game"
    });
    window.set_default_size(480, 760);
    window.set_position(gtk::WindowPosition::Center);
    let layout = gtk::Box::new(gtk::Orientation::Vertical, 0);
    layout.style_context().add_class("bnet-login-content");
    let address = gtk::Label::new(Some(if account_mode {
        "Secure sign-in · account.battle.net"
    } else if chat_mode {
        "Battle.net · Chat authorization"
    } else {
        "Battle.net · Game authorization"
    }));
    address.style_context().add_class("bnet-login-status");
    address.set_margin_top(10);
    address.set_margin_bottom(10);
    layout.pack_start(&address, false, false, 0);
    layout.pack_start(&web, true, true, 0);
    window.add(&layout);
    let result = ticket.clone();
    let status = address.clone();
    context.register_uri_scheme("bnet-linux-login", move |request| {
        if !account_mode {
            if let Some(uri) = request.uri() {
                capture(&uri, &result, &status);
            }
        }
        let bytes = gtk::glib::Bytes::from_static(b"Returning to Battle.net Linux...");
        let stream = gtk::gio::MemoryInputStream::from_bytes(&bytes);
        request.finish(&stream, bytes.len() as i64, Some("text/plain"));
    });
    let popups = Rc::new(RefCell::new(Vec::new()));
    prepare_view(
        &web,
        &ticket,
        &address,
        &popups,
        if account_mode { Some(&account) } else { None },
        fixture,
    );
    web.connect_web_process_terminated(|_, _| eprintln!("BNET_EVENT=web_process_terminated"));
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    controls.style_context().add_class("bnet-login-controls");
    controls.set_margin_start(12);
    controls.set_margin_end(12);
    controls.set_margin_top(8);
    controls.set_margin_bottom(8);
    let retry = gtk::Button::with_label("Retry sign-in");
    let cancel = gtk::Button::with_label("Cancel");
    let retry_web = web.clone();
    let retry_status = address.clone();
    let retry_start = start.clone();
    retry.connect_clicked(move |_| {
        retry_status.set_text("Connecting to Battle.net…");
        retry_web.load_uri(&retry_start);
    });
    cancel.connect_clicked(|_| {
        eprintln!("BNET_EVENT=cancelled");
        finish_window();
    });
    controls.pack_end(&cancel, false, false, 0);
    controls.pack_end(&retry, false, false, 0);
    layout.pack_start(&controls, false, false, 0);
    window.connect_delete_event(|_, _| {
        eprintln!("BNET_EVENT=window_closed");
        finish_window();
        gtk::glib::Propagation::Proceed
    });
    window.show_all();
    eprintln!("BNET_EVENT=window_opened");
    let startup_check = args.iter().any(|arg| arg == "--startup-check");
    if startup_check {
        gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(1200), finish_window);
    } else if fixture && args.iter().any(|arg| arg == "--test-callback") {
        web.load_uri("http://localhost:0/?ST=EU-00000000000000000000000000000000-12345");
    } else if fixture
        && args
            .iter()
            .any(|arg| arg == "--test-redirect" || arg == "--test-account")
    {
        if let Some(settings) = WebViewExt::settings(&web) {
            settings.set_javascript_can_open_windows_automatically(true);
        }
        let url = std::env::var("BNET_TEST_LOGIN_URL").expect("test fixture URL");
        let parsed = url::Url::parse(&url).expect("test fixture URL");
        assert!(parsed.scheme() == "http" && parsed.host_str() == Some("127.0.0.1"));
        web.load_uri(&url);
    } else {
        web.load_uri(&start);
    }
    gtk::main();
    if startup_check {
        println!("ready");
        return;
    }
    if account_mode {
        if let Some(account) = account.borrow_mut().take() {
            let _ = serde_json::to_writer(std::io::stdout().lock(), &account);
        } else {
            std::process::exit(1);
        }
    } else if let Some(ticket) = ticket.borrow_mut().take() {
        let mut stdout = std::io::stdout().lock();
        let _ = stdout.write_all(ticket.expose().as_bytes());
        let _ = stdout.flush();
    } else {
        std::process::exit(1);
    }
}
