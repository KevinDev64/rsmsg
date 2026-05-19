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
const HISTORY_FILE: &str = ".rsmsg_chat_history.json";

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
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
    chats: BTreeMap<String, Vec<ChatMessage>>,
    peer_by_device_uuid: BTreeMap<String, String>,
}

impl ChatHistory {
    fn load() -> Self {
        let path = Path::new(HISTORY_FILE);
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
            let _ = fs::write(HISTORY_FILE, raw);
        }
    }
}

struct MessengerApp {
    core: ClientCore,
    local_keys: LocalDeviceKeys,
    history: ChatHistory,
    nickname: String,
    password: String,
    auth: Option<DeviceAuth>,
    status: String,
    peer_nickname_input: String,
    selected_chat: String,
    peer_device_uuid: String,
    message_input: String,
    last_sync_at: Instant,
}

impl MessengerApp {
    fn new() -> Self {
        let core = ClientCore::new(ClientConfig::local_default());
        let local_keys = core.load_or_create_local_device_keys();
        Self {
            core,
            local_keys,
            history: ChatHistory::load(),
            nickname: String::new(),
            password: String::new(),
            auth: None,
            status: "Not logged in".to_string(),
            peer_nickname_input: String::new(),
            selected_chat: String::new(),
            peer_device_uuid: String::new(),
            message_input: String::new(),
            last_sync_at: Instant::now(),
        }
    }

    fn register_or_login(&mut self, create: bool) {
        if self.nickname.trim().is_empty() || self.password.len() < 6 {
            self.status = "Enter nickname and password (>=6)".to_string();
            return;
        }
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
        let derive = rt.block_on(self.core.derive_peer_shared_key(
            &self.local_keys,
            peer.clone(),
            DEFAULT_DEVICE_ID.to_string(),
        ));
        match derive {
            Ok((_key, bundle)) => {
                self.peer_device_uuid = bundle.device_uuid;
                self.selected_chat = peer.clone();
                self.history.chats.entry(peer.clone()).or_default();
                self.history
                    .peer_by_device_uuid
                    .insert(self.peer_device_uuid.clone(), peer.clone());
                self.history.save();
                self.status = format!("Chat with @{peer} ready");
            }
            Err(err) => {
                self.status = format!("Open chat failed: {err}");
                let _ = auth;
            }
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
        if self.message_input.trim().is_empty() {
            return;
        }
        let text = self.message_input.clone();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        match rt.block_on(self.core.send_text_to_peer(
            &auth,
            self.peer_device_uuid.clone(),
            text.clone(),
        )) {
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

            ui.separator();
            ui.heading("New chat");
            ui.label("Peer nickname");
            ui.text_edit_singleline(&mut self.peer_nickname_input);
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

            egui::ScrollArea::vertical().show(ui, |ui| {
                if let Some(messages) = self.history.chats.get(&self.selected_chat) {
                    for m in messages {
                        let who = if m.outgoing { "You" } else { "Peer" };
                        ui.label(format!("{who}: {}", m.text));
                    }
                }
            });

            ui.separator();
            ui.text_edit_multiline(&mut self.message_input);
            if ui.button("Send").clicked() {
                self.send_current_message();
            }
        });
    }
}

fn chrono_like_now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let Ok(dur) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return 0;
    };
    (dur.as_secs() as i64) * 1000 + (dur.subsec_millis() as i64)
}
