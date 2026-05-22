use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    time::{Duration, Instant},
};

use client_core::{ClientConfig, ClientCore, DecryptedMessage, DeviceAuth, LocalDeviceKeys};
use eframe::egui;
use serde::{Deserialize, Serialize};

const DEFAULT_DEVICE_ID: &str = "main";
fn history_file() -> String {
    let profile = std::env::var("RSMSG_PROFILE").unwrap_or_else(|_| "default".to_string());
    format!(".rsmsg_chat_history.{profile}.json")
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        run_and_return: false,
        ..Default::default()
    };
    eframe::run_native(
        "rsmsg",
        options,
        Box::new(|_cc| Ok(Box::new(MessengerApp::new()))),
    )
}

#[derive(Clone, Serialize, Deserialize)]
struct ChatMessage {
    outgoing: bool,
    text: String,
    ts: i64,
}

#[derive(Default, Serialize, Deserialize)]
struct ChatHistory {
    #[serde(default)]
    chats: BTreeMap<String, Vec<ChatMessage>>,
    #[serde(default)]
    peer_by_device_uuid: BTreeMap<String, String>,
    #[serde(default)]
    device_uuid_by_peer: BTreeMap<String, String>,
}

impl ChatHistory {
    fn load() -> Self {
        let file = history_file();
        let path = Path::new(&file);
        if !path.exists() {
            return Self::default();
        }
        let Ok(raw) = fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str(&raw).unwrap_or_default()
    }

    fn save(&self) {
        if let Ok(raw) = serde_json::to_string_pretty(self) {
            let _ = fs::write(history_file(), raw);
        }
    }
}

struct MessengerApp {
    core: ClientCore,
    local_keys: LocalDeviceKeys,
    history: ChatHistory,
    server_input: String,
    nickname: String,
    password: String,
    auth: Option<DeviceAuth>,
    status: String,
    peer_nickname_input: String,
    peer_search_results: Vec<String>,
    selected_chat: String,
    message_input: String,
    last_sync_at: Instant,
}

impl MessengerApp {
    fn new() -> Self {
        let config = ClientConfig::local_default();
        let server_input = config.http_base.clone();
        let core = ClientCore::new(config);
        let local_keys = core.load_or_create_local_device_keys();
        Self {
            core,
            local_keys,
            history: ChatHistory::load(),
            server_input,
            nickname: String::new(),
            password: String::new(),
            auth: None,
            status: "Not logged in".to_string(),
            peer_nickname_input: String::new(),
            peer_search_results: Vec::new(),
            selected_chat: String::new(),
            message_input: String::new(),
            last_sync_at: Instant::now(),
        }
    }

    fn register_or_login(&mut self, create: bool) {
        if self.nickname.trim().is_empty() || self.password.len() < 6 {
            self.status = "Enter nickname and password (>=6)".to_string();
            return;
        }
        self.apply_server_config();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        if create {
            match rt.block_on(
                self.core
                    .register_user(self.nickname.clone(), self.password.clone()),
            ) {
                Ok(true) => self.status = "Account created".to_string(),
                Ok(false) => self.status = "Nickname already exists".to_string(),
                Err(err) => {
                    self.status = format!("Account create failed: {err}");
                    return;
                }
            }
        }

        match rt.block_on(
            self.core
                .login_user(self.nickname.clone(), self.password.clone()),
        ) {
            Ok(true) => {}
            Ok(false) => {
                self.status = "Invalid credentials".to_string();
                return;
            }
            Err(err) => {
                self.status = format!("User login failed: {err}");
                return;
            }
        }

        let req = self.core.build_register_request(
            self.nickname.clone(),
            DEFAULT_DEVICE_ID.to_string(),
            &self.local_keys,
        );
        if let Err(err) = rt.block_on(self.core.register_device(req)) {
            self.status = format!("Register failed: {err}");
            return;
        }
        match rt.block_on(
            self.core
                .login_device(self.nickname.clone(), DEFAULT_DEVICE_ID.to_string()),
        ) {
            Ok(auth) => {
                self.auth = Some(auth);
                self.status = format!("Logged in as {}", self.nickname);
            }
            Err(err) => self.status = format!("Login failed: {err}"),
        }
    }

    fn apply_server_config(&mut self) {
        let config = ClientConfig::for_server(&self.server_input);
        self.server_input = config.http_base.clone();
        self.core = ClientCore::new(config);
        self.local_keys = self.core.load_or_create_local_device_keys();
    }

    fn logout(&mut self) {
        self.auth = None;
        self.password.clear();
        self.status = "Logged out".to_string();
    }

    fn open_chat(&mut self) {
        let Some(auth) = self.auth.clone() else {
            self.status = "Log in first".to_string();
            return;
        };
        if self.peer_nickname_input.trim().is_empty() {
            self.status = "Enter peer nickname".to_string();
            return;
        }
        let peer = self.peer_nickname_input.trim().to_string();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        let resolved = rt.block_on(
            self.core
                .resolve_user_device(peer.clone(), DEFAULT_DEVICE_ID.to_string()),
        );
        let Ok(resolved_uuid) = resolved else {
            self.status = "Peer not found".to_string();
            return;
        };

        let derive = rt.block_on(self.core.derive_peer_shared_key(
            &self.local_keys,
            peer.clone(),
            DEFAULT_DEVICE_ID.to_string(),
        ));
        match derive {
            Ok((_key, bundle)) => {
                if bundle.device_uuid != resolved_uuid {
                    self.status = "Peer resolve mismatch, retry".to_string();
                    return;
                }
                self.selected_chat = peer.clone();
                self.history.chats.entry(peer.clone()).or_default();
                self.history
                    .peer_by_device_uuid
                    .insert(resolved_uuid.clone(), peer.clone());
                self.history
                    .device_uuid_by_peer
                    .insert(peer.clone(), resolved_uuid);
                self.history.save();
                self.status = format!("Chat with @{peer} ready");
            }
            Err(err) => {
                self.status = format!("Open chat failed: {err}");
                let _ = auth;
            }
        }
    }

    fn search_users(&mut self) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        match rt.block_on(self.core.search_users(self.peer_nickname_input.clone())) {
            Ok(users) => {
                self.peer_search_results = users;
                if self.peer_search_results.is_empty() {
                    self.status = "No users found".to_string();
                } else {
                    self.status = format!("Found {} users", self.peer_search_results.len());
                }
            }
            Err(err) => self.status = format!("Search failed: {err}"),
        }
    }

    fn send_current_message(&mut self) {
        let Some(auth) = self.auth.clone() else {
            self.status = "Log in first".to_string();
            return;
        };
        if self.selected_chat.is_empty() {
            self.status = "Select chat first".to_string();
            return;
        }
        let Some(peer_device_uuid) = self.ensure_selected_chat_session() else {
            return;
        };
        if self.message_input.trim().is_empty() {
            return;
        }
        let text = self.message_input.clone();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        match rt.block_on(
            self.core
                .send_text_to_peer(&auth, peer_device_uuid, text.clone()),
        ) {
            Ok(true) => {
                self.history
                    .chats
                    .entry(self.selected_chat.clone())
                    .or_default()
                    .push(ChatMessage {
                        outgoing: true,
                        text,
                        ts: chrono_like_now_ms(),
                    });
                self.history.save();
                self.message_input.clear();
                self.status = "Sent".to_string();
            }
            Ok(false) => self.status = "Peer session missing. Re-open chat.".to_string(),
            Err(err) => self.status = format!("Send failed: {err}"),
        }
    }

    fn ensure_selected_chat_session(&mut self) -> Option<String> {
        if self.selected_chat.is_empty() {
            self.status = "Select chat first".to_string();
            return None;
        }

        let peer = self.selected_chat.clone();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        let resolved = rt.block_on(
            self.core
                .resolve_user_device(peer.clone(), DEFAULT_DEVICE_ID.to_string()),
        );
        let Ok(resolved_uuid) = resolved else {
            self.status = "Peer not found".to_string();
            return None;
        };

        let derive = rt.block_on(self.core.derive_peer_shared_key(
            &self.local_keys,
            peer.clone(),
            DEFAULT_DEVICE_ID.to_string(),
        ));
        let Ok((_key, bundle)) = derive else {
            self.status = "Could not prepare peer session".to_string();
            return None;
        };
        if bundle.device_uuid != resolved_uuid {
            self.status = "Peer resolve mismatch, retry".to_string();
            return None;
        }
        self.history
            .peer_by_device_uuid
            .insert(resolved_uuid.clone(), peer.clone());
        self.history
            .device_uuid_by_peer
            .insert(peer.clone(), resolved_uuid.clone());
        self.history.save();

        Some(resolved_uuid)
    }

    fn sync_incoming(&mut self) {
        let Some(auth) = self.auth.clone() else {
            return;
        };
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let pending = rt.block_on(self.core.fetch_pending(&auth, Some(100)));
        let Ok(pending) = pending else {
            return;
        };
        for item in &pending {
            if let Some(peer) = self.history.peer_by_device_uuid.get(&item.from_device_uuid) {
                let _ = rt.block_on(self.core.derive_peer_shared_key(
                    &self.local_keys,
                    peer.clone(),
                    DEFAULT_DEVICE_ID.to_string(),
                ));
            }
        }
        let (decrypted, ack_ids) = self.core.decrypt_pending_with_sessions(pending);
        if !ack_ids.is_empty() {
            let _ = rt.block_on(self.core.ack_messages(&auth, ack_ids));
        }
        for msg in decrypted {
            self.push_incoming(msg);
        }
        self.history.save();
        self.last_sync_at = Instant::now();
    }

    fn push_incoming(&mut self, msg: DecryptedMessage) {
        let nick = self
            .history
            .peer_by_device_uuid
            .get(&msg.from_device_uuid)
            .cloned()
            .unwrap_or_else(|| {
                format!(
                    "unknown:{}",
                    &msg.from_device_uuid[..8.min(msg.from_device_uuid.len())]
                )
            });
        let chat = self.history.chats.entry(nick).or_default();
        chat.push(ChatMessage {
            outgoing: false,
            text: msg.plaintext,
            ts: msg.created_at_unix_ms,
        });
    }
}

impl eframe::App for MessengerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(800));
        if self.auth.is_some() && self.last_sync_at.elapsed() >= Duration::from_secs(2) {
            self.sync_incoming();
        }

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("rsmsg");
                ui.separator();
                ui.label(&self.status);
            });
        });

        egui::SidePanel::left("left").show(ctx, |ui| {
            ui.heading("Account");
            if self.auth.is_some() {
                ui.label(format!("@{}", self.nickname));
                ui.label(&self.server_input);
                if ui.button("Logout").clicked() {
                    self.logout();
                }
            } else {
                ui.label("Server");
                ui.text_edit_singleline(&mut self.server_input);
                ui.label("Nickname");
                ui.text_edit_singleline(&mut self.nickname);
                ui.label("Password");
                ui.add(egui::TextEdit::singleline(&mut self.password).password(true));
                if ui.button("Register").clicked() {
                    self.register_or_login(true);
                }
                if ui.button("Login").clicked() {
                    self.register_or_login(false);
                }
            }

            ui.separator();
            ui.heading("New chat");
            ui.label("Peer nickname");
            ui.text_edit_singleline(&mut self.peer_nickname_input);
            if ui.button("Search users").clicked() {
                self.search_users();
            }
            for nick in &self.peer_search_results {
                if ui.button(format!("@{nick}")).clicked() {
                    self.peer_nickname_input = nick.clone();
                }
            }
            if ui.button("Open chat").clicked() {
                self.open_chat();
            }

            ui.separator();
            ui.heading("Chats");
            for nick in self.history.chats.keys() {
                let selected = self.selected_chat == *nick;
                if ui.selectable_label(selected, format!("@{nick}")).clicked() {
                    self.selected_chat = nick.clone();
                }
            }
            if ui.button("Sync incoming").clicked() {
                self.sync_incoming();
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.auth.is_none() {
                ui.heading("Welcome to rsmsg");
                ui.label("1) Enter your nickname");
                ui.label("2) Press Register / Login");
                ui.label("3) Open chat by peer nickname");
                return;
            }

            if self.selected_chat.is_empty() {
                ui.heading("Select or create a chat");
                return;
            }

            ui.heading(format!("Chat with @{}", self.selected_chat));
            ui.separator();

            let composer_height = 96.0;
            let history_height = (ui.available_height() - composer_height).max(120.0);
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .max_height(history_height)
                .show(ui, |ui| {
                    if let Some(messages) = self.history.chats.get(&self.selected_chat) {
                        for m in messages {
                            render_message_bubble(ui, m, &self.selected_chat);
                        }
                    }
                });

            ui.separator();
            let response = ui.add_sized(
                [ui.available_width(), 56.0],
                egui::TextEdit::multiline(&mut self.message_input).hint_text("Message"),
            );
            let send_by_enter = response.has_focus()
                && ui.input(|input| input.key_pressed(egui::Key::Enter) && !input.modifiers.shift);
            if send_by_enter {
                while self.message_input.ends_with(['\n', '\r']) {
                    self.message_input.pop();
                }
                self.send_current_message();
            }
            ui.horizontal(|ui| {
                if ui.button("Send").clicked() {
                    self.send_current_message();
                }
            });
        });
    }
}

fn render_message_bubble(ui: &mut egui::Ui, message: &ChatMessage, peer: &str) {
    let bubble_color = if message.outgoing {
        egui::Color32::from_rgb(56, 120, 255)
    } else {
        egui::Color32::from_rgb(44, 48, 58)
    };
    let text_color = egui::Color32::WHITE;
    let meta = if message.outgoing {
        format!("You · {}", format_message_time(message.ts))
    } else {
        format!("@{peer} · {}", format_message_time(message.ts))
    };
    let max_width = ui.available_width() * 0.72;
    let frame = egui::Frame::new()
        .fill(bubble_color)
        .corner_radius(egui::CornerRadius::same(14))
        .inner_margin(egui::Margin::symmetric(12, 8));

    if message.outgoing {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
            frame.show(ui, |ui| {
                render_bubble_content(ui, &meta, &message.text, text_color, max_width)
            });
        });
    } else {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
            frame.show(ui, |ui| {
                render_bubble_content(ui, &meta, &message.text, text_color, max_width)
            });
        });
    }
    ui.add_space(6.0);
}

fn render_bubble_content(
    ui: &mut egui::Ui,
    meta: &str,
    text: &str,
    text_color: egui::Color32,
    max_width: f32,
) {
    ui.set_max_width(max_width);
    ui.add_sized(
        [max_width, 0.0],
        egui::Label::new(egui::RichText::new(meta).small().color(text_color)).wrap(),
    );
    ui.add_sized(
        [max_width, 0.0],
        egui::Label::new(egui::RichText::new(text).color(text_color)).wrap(),
    );
}

fn chrono_like_now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let Ok(dur) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return 0;
    };
    (dur.as_secs() as i64) * 1000 + (dur.subsec_millis() as i64)
}

fn format_message_time(ts_ms: i64) -> String {
    let seconds = (ts_ms / 1000).rem_euclid(86_400);
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    format!("{hours:02}:{minutes:02}")
}
