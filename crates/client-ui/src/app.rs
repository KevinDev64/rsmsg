use std::{
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

use client_core::{ClientConfig, ClientCore, DecryptedMessage, DeviceAuth, LocalDeviceKeys};
use eframe::egui;
use shared::FetchPrekeyBundleResponse;
use uuid::Uuid;

use crate::{
    history::{ChatHistory, ChatMessage, MessageStatus, now_ms},
    message_ui::render_message_bubble,
};

const DEFAULT_DEVICE_ID: &str = "main";

pub struct MessengerApp {
    core: ClientCore,
    local_keys: Option<LocalDeviceKeys>,
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
    key_change_peer: Option<String>,
    login_rx: Option<Receiver<LoginResult>>,
    open_chat_rx: Option<Receiver<OpenChatResult>>,
    last_sync_at: Instant,
}

struct LoginResult {
    result: Result<LoginSuccess, String>,
}

struct LoginSuccess {
    core: ClientCore,
    local_keys: LocalDeviceKeys,
    history: ChatHistory,
    auth: DeviceAuth,
    server_input: String,
    status: String,
}

struct OpenChatResult {
    result: Result<OpenChatSuccess, String>,
}

struct OpenChatSuccess {
    peer: String,
    resolved_uuid: String,
    bundle: FetchPrekeyBundleResponse,
}

impl MessengerApp {
    pub fn new() -> Self {
        let config = ClientConfig::local_default();
        let server_input = config.http_base.clone();
        let core = ClientCore::new(config);
        Self {
            core,
            local_keys: None,
            history: ChatHistory::load(None),
            server_input,
            nickname: String::new(),
            password: String::new(),
            auth: None,
            status: "Not logged in".to_string(),
            peer_nickname_input: String::new(),
            peer_search_results: Vec::new(),
            selected_chat: String::new(),
            message_input: String::new(),
            key_change_peer: None,
            login_rx: None,
            open_chat_rx: None,
            last_sync_at: Instant::now(),
        }
    }

    fn register_or_login(&mut self, create: bool) {
        if self.login_rx.is_some() {
            return;
        }
        if self.nickname.trim().is_empty() || self.password.len() < 6 {
            self.status = "Enter nickname and password (>=6)".to_string();
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.login_rx = Some(rx);
        self.status = if create {
            "Creating account...".to_string()
        } else {
            "Logging in...".to_string()
        };
        let nickname = self.nickname.clone();
        let password = self.password.clone();
        let server_input = self.server_input.clone();
        thread::spawn(move || {
            let result = run_login_flow(create, nickname, password, server_input);
            let _ = tx.send(LoginResult { result });
        });
    }

    fn poll_login_result(&mut self) {
        let Some(rx) = self.login_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(login) => match login.result {
                Ok(success) => {
                    self.core = success.core;
                    self.local_keys = Some(success.local_keys);
                    self.history = success.history;
                    self.auth = Some(success.auth);
                    self.server_input = success.server_input;
                    self.status = success.status;
                }
                Err(err) => self.status = err,
            },
            Err(mpsc::TryRecvError::Empty) => {
                self.login_rx = Some(rx);
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.status = "Login worker stopped".to_string();
            }
        }
    }

    fn apply_server_config(&mut self) {
        let config = ClientConfig::for_server(&self.server_input);
        self.server_input = config.http_base.clone();
        self.core = ClientCore::new(config);
        self.local_keys = None;
    }

    fn logout(&mut self) {
        if let Some(auth) = self.auth.clone() {
            let rt = runtime();
            let _ = rt.block_on(self.core.logout_device(&auth));
        }
        self.auth = None;
        self.local_keys = None;
        self.history = ChatHistory::load(None);
        self.password.clear();
        self.status = "Logged out".to_string();
    }

    fn open_chat(&mut self) {
        if self.open_chat_rx.is_some() {
            return;
        }
        let Some(_auth) = self.auth.clone() else {
            self.status = "Log in first".to_string();
            return;
        };
        if self.peer_nickname_input.trim().is_empty() {
            self.status = "Enter peer nickname".to_string();
            return;
        }
        let peer = self.peer_nickname_input.trim().to_string();
        let Some(local_keys) = self.local_keys.clone() else {
            self.status = "Log in first".to_string();
            return;
        };
        let (tx, rx) = mpsc::channel();
        self.open_chat_rx = Some(rx);
        self.status = format!("Opening chat with @{peer}...");
        let core = self.core.clone();
        thread::spawn(move || {
            let result = run_open_chat_flow(core, local_keys, peer);
            let _ = tx.send(OpenChatResult { result });
        });
    }

    fn poll_open_chat_result(&mut self) {
        let Some(rx) = self.open_chat_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(opened) => match opened.result {
                Ok(success) => self.apply_open_chat_success(success),
                Err(err) => self.status = err,
            },
            Err(mpsc::TryRecvError::Empty) => self.open_chat_rx = Some(rx),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.status = "Open chat worker stopped".to_string();
            }
        }
    }

    fn apply_open_chat_success(&mut self, success: OpenChatSuccess) {
        if success.bundle.device_uuid != success.resolved_uuid {
            self.status = "Peer resolve mismatch, retry".to_string();
            return;
        }
        if !self.verify_or_pin_peer_identity(&success.peer, &success.bundle) {
            return;
        }
        self.selected_chat = success.peer.clone();
        self.history.chats.entry(success.peer.clone()).or_default();
        self.history
            .peer_by_device_uuid
            .insert(success.resolved_uuid.clone(), success.peer.clone());
        self.history
            .device_uuid_by_peer
            .insert(success.peer.clone(), success.resolved_uuid);
        if let Some(auth) = self.auth.clone() {
            let rt = runtime();
            self.mark_selected_chat_read(&rt, &auth);
        }
        self.save_history();
        self.status = format!("Chat with @{} ready", success.peer);
    }

    fn search_users(&mut self) {
        let rt = runtime();
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
        let chat_name = self.selected_chat.clone();
        let message_id = Uuid::new_v4().to_string();
        let message_index = {
            let chat = self.history.chats.entry(chat_name.clone()).or_default();
            chat.push(ChatMessage {
                outgoing: true,
                text: text.clone(),
                ts: now_ms(),
                status: MessageStatus::Sending,
                message_id: Some(message_id.clone()),
            });
            chat.len() - 1
        };
        self.message_input.clear();
        self.save_history();
        let rt = runtime();
        match rt.block_on(self.core.send_text_to_peer_with_id(
            &auth,
            peer_device_uuid,
            text.clone(),
            message_id,
        )) {
            Ok(true) => {
                self.update_message_status(&chat_name, message_index, MessageStatus::Sent);
                self.save_history();
                self.status = "Sent".to_string();
            }
            Ok(false) => {
                self.update_message_status(&chat_name, message_index, MessageStatus::Failed);
                self.save_history();
                self.status = "Peer session missing. Re-open chat.".to_string();
            }
            Err(err) => {
                self.update_message_status(&chat_name, message_index, MessageStatus::Failed);
                self.save_history();
                self.status = format!("Send failed: {err}");
            }
        }
    }

    fn update_message_status(&mut self, chat_name: &str, index: usize, status: MessageStatus) {
        if let Some(message) = self
            .history
            .chats
            .get_mut(chat_name)
            .and_then(|messages| messages.get_mut(index))
        {
            message.status = status;
        }
    }

    fn ensure_selected_chat_session(&mut self) -> Option<String> {
        if self.selected_chat.is_empty() {
            self.status = "Select chat first".to_string();
            return None;
        }

        let peer = self.selected_chat.clone();
        let rt = runtime();

        let resolved = rt.block_on(
            self.core
                .resolve_user_device(peer.clone(), DEFAULT_DEVICE_ID.to_string()),
        );
        let Ok(resolved_uuid) = resolved else {
            self.status = "Peer not found".to_string();
            return None;
        };

        let known_uuid = self.history.device_uuid_by_peer.get(&peer).cloned();
        if known_uuid.as_deref() == Some(resolved_uuid.as_str())
            && self.core.has_peer_session(&resolved_uuid)
        {
            return Some(resolved_uuid);
        }

        let derive = rt.block_on(self.core.derive_peer_shared_key(
            self.local_keys.as_ref().expect("local keys"),
            peer.clone(),
            DEFAULT_DEVICE_ID.to_string(),
        ));
        let Ok((_key, bundle)) = derive else {
            let err = derive.err().map(|err| err.to_string()).unwrap_or_default();
            self.status = format!("Could not prepare peer session: {err}");
            return None;
        };
        if bundle.device_uuid != resolved_uuid {
            self.status = "Peer resolve mismatch, retry".to_string();
            return None;
        }
        if !self.verify_or_pin_peer_identity(&peer, &bundle) {
            return None;
        }
        self.history
            .peer_by_device_uuid
            .insert(resolved_uuid.clone(), peer.clone());
        self.history
            .device_uuid_by_peer
            .insert(peer.clone(), resolved_uuid.clone());
        self.save_history();

        Some(resolved_uuid)
    }

    fn sync_incoming(&mut self) {
        let Some(auth) = self.auth.clone() else {
            return;
        };
        let rt = runtime();
        let pending = rt.block_on(self.core.fetch_pending(&auth, Some(100)));
        let Ok(pending) = pending else {
            return;
        };
        for item in &pending {
            let peer =
                if let Some(peer) = self.history.peer_by_device_uuid.get(&item.from_device_uuid) {
                    Some(peer.clone())
                } else {
                    rt.block_on(self.core.resolve_device_user(item.from_device_uuid.clone()))
                        .ok()
                };
            if let Some(peer) = peer {
                if let Ok((_key, bundle)) = rt.block_on(self.core.derive_peer_shared_key(
                    self.local_keys.as_ref().expect("local keys"),
                    peer.clone(),
                    DEFAULT_DEVICE_ID.to_string(),
                )) {
                    if !self.verify_or_pin_peer_identity(&peer, &bundle) {
                        continue;
                    }
                    self.history
                        .peer_by_device_uuid
                        .insert(item.from_device_uuid.clone(), peer.clone());
                    self.history
                        .device_uuid_by_peer
                        .insert(peer.clone(), bundle.device_uuid);
                    self.history.chats.entry(peer).or_default();
                }
            }
        }
        let (decrypted, _ack_ids) = self.core.decrypt_pending_with_sessions(pending);
        for msg in decrypted {
            self.push_incoming(msg);
        }
        self.mark_selected_chat_read(&rt, &auth);
        self.sync_outgoing_statuses(&rt, &auth);
        self.save_history();
        self.last_sync_at = Instant::now();
    }

    fn mark_selected_chat_read(&mut self, rt: &tokio::runtime::Runtime, auth: &DeviceAuth) {
        if self.selected_chat.is_empty() {
            return;
        }
        let Some(messages) = self.history.chats.get_mut(&self.selected_chat) else {
            return;
        };
        let message_ids: Vec<String> = messages
            .iter()
            .filter(|message| !message.outgoing && message.status != MessageStatus::Read)
            .filter_map(|message| message.message_id.clone())
            .collect();
        if message_ids.is_empty() {
            self.history.unread_by_peer.remove(&self.selected_chat);
            return;
        }
        if rt
            .block_on(self.core.ack_messages(auth, message_ids))
            .is_ok()
        {
            for message in messages {
                if !message.outgoing {
                    message.status = MessageStatus::Read;
                }
            }
            self.history.unread_by_peer.remove(&self.selected_chat);
        }
    }

    fn sync_outgoing_statuses(&mut self, rt: &tokio::runtime::Runtime, auth: &DeviceAuth) {
        let message_ids: Vec<String> = self
            .history
            .chats
            .values()
            .flat_map(|messages| messages.iter())
            .filter(|message| message.outgoing && message.status != MessageStatus::Read)
            .filter_map(|message| message.message_id.clone())
            .collect();
        if message_ids.is_empty() {
            return;
        }
        let Ok(statuses) = rt.block_on(self.core.message_statuses(auth, message_ids)) else {
            return;
        };
        for status in statuses {
            let next = if status.read {
                MessageStatus::Read
            } else if status.delivered {
                MessageStatus::Delivered
            } else {
                MessageStatus::Sent
            };
            for messages in self.history.chats.values_mut() {
                for message in messages {
                    if message.message_id.as_deref() == Some(status.message_id.as_str()) {
                        message.status = next;
                    }
                }
            }
        }
    }

    fn verify_or_pin_peer_identity(
        &mut self,
        peer: &str,
        bundle: &FetchPrekeyBundleResponse,
    ) -> bool {
        let pinned_identity = self.history.peer_identity_key_by_peer.get(peer);
        let pinned_signing = self.history.peer_signing_identity_key_by_peer.get(peer);
        let identity_changed = pinned_identity
            .map(|key| key != &bundle.identity_key_b64)
            .unwrap_or(false);
        let signing_changed = pinned_signing
            .map(|key| key != &bundle.signing_identity_key_b64)
            .unwrap_or(false);
        if identity_changed || signing_changed {
            self.key_change_peer = Some(peer.to_string());
            self.status = format!("Security warning: @{peer} changed identity key");
            return false;
        }
        self.history
            .peer_identity_key_by_peer
            .entry(peer.to_string())
            .or_insert_with(|| bundle.identity_key_b64.clone());
        self.history
            .peer_signing_identity_key_by_peer
            .entry(peer.to_string())
            .or_insert_with(|| bundle.signing_identity_key_b64.clone());
        true
    }

    fn trust_new_peer_identity(&mut self) {
        let Some(peer) = self.key_change_peer.clone() else {
            return;
        };
        let rt = runtime();
        let Ok((_key, bundle)) = rt.block_on(self.core.derive_peer_shared_key(
            self.local_keys.as_ref().expect("local keys"),
            peer.clone(),
            DEFAULT_DEVICE_ID.to_string(),
        )) else {
            self.status = format!("Could not refresh @{peer} identity");
            return;
        };
        self.history
            .peer_identity_key_by_peer
            .insert(peer.clone(), bundle.identity_key_b64);
        self.history
            .peer_signing_identity_key_by_peer
            .insert(peer.clone(), bundle.signing_identity_key_b64);
        self.key_change_peer = None;
        self.save_history();
        self.status = format!("Trusted new identity for @{peer}");
    }

    fn save_history(&self) {
        self.history.save(self.password_for_local_storage());
    }

    fn password_for_local_storage(&self) -> Option<&str> {
        if self.auth.is_some() && !self.password.is_empty() {
            Some(&self.password)
        } else {
            None
        }
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
        let chat = self.history.chats.entry(nick.clone()).or_default();
        chat.push(ChatMessage {
            outgoing: false,
            text: msg.plaintext,
            ts: msg.created_at_unix_ms,
            status: MessageStatus::Delivered,
            message_id: Some(msg.message_id),
        });
        if self.selected_chat != nick {
            *self.history.unread_by_peer.entry(nick).or_default() += 1;
        }
    }
}

impl eframe::App for MessengerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_login_result();
        self.poll_open_chat_result();
        ctx.request_repaint_after(Duration::from_millis(800));
        if self.auth.is_some() && self.last_sync_at.elapsed() >= Duration::from_secs(2) {
            self.sync_incoming();
        }

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("rsmsg");
                ui.separator();
                ui.label(&self.status);
                if let Some(peer) = &self.key_change_peer {
                    ui.separator();
                    ui.label(format!("@{peer} key changed"));
                    if ui.button("Trust new key").clicked() {
                        self.trust_new_peer_identity();
                    }
                }
            });
        });

        egui::SidePanel::left("left").show(ctx, |ui| {
            ui.heading("Account");
            if self.auth.is_some() {
                ui.label(format!("@{}", self.nickname));
                ui.collapsing("Settings", |ui| {
                    ui.label("Server");
                    ui.text_edit_singleline(&mut self.server_input);
                    ui.label("Nickname");
                    ui.text_edit_singleline(&mut self.nickname);
                    if ui.button("Apply after logout").clicked() {
                        self.logout();
                        self.apply_server_config();
                    }
                });
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
                let login_busy = self.login_rx.is_some();
                if ui
                    .add_enabled(!login_busy, egui::Button::new("Register"))
                    .clicked()
                {
                    self.register_or_login(true);
                }
                if ui
                    .add_enabled(!login_busy, egui::Button::new("Login"))
                    .clicked()
                {
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
            let chat_names: Vec<String> = self.history.chats.keys().cloned().collect();
            for nick in chat_names {
                let selected = self.selected_chat == nick;
                let unread = self.history.unread_by_peer.get(&nick).copied().unwrap_or(0);
                let label = if unread > 0 {
                    format!("@{nick} ({unread})")
                } else {
                    format!("@{nick}")
                };
                if ui.selectable_label(selected, label).clicked() {
                    self.selected_chat = nick.clone();
                    if let Some(auth) = self.auth.clone() {
                        let rt = runtime();
                        self.mark_selected_chat_read(&rt, &auth);
                    }
                    self.save_history();
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

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
}

fn run_login_flow(
    create: bool,
    nickname: String,
    password: String,
    server_input: String,
) -> Result<LoginSuccess, String> {
    let config = ClientConfig::for_server(&server_input);
    let normalized_server = config.http_base.clone();
    let core = ClientCore::new(config);
    let rt = runtime();

    if create {
        match rt.block_on(core.register_user(nickname.clone(), password.clone())) {
            Ok(true) => {}
            Ok(false) => {}
            Err(err) => return Err(format!("Account create failed: {err}")),
        }
    }

    match rt.block_on(core.login_user(nickname.clone(), password.clone())) {
        Ok(true) => {}
        Ok(false) => return Err("Invalid credentials".to_string()),
        Err(err) => return Err(format!("User login failed: {err}")),
    }

    core.unlock_local_storage(password.clone());
    let history = ChatHistory::load(Some(&password));
    let local_keys = core.load_or_create_local_device_keys();
    let req = core
        .build_register_request(nickname.clone(), DEFAULT_DEVICE_ID.to_string(), &local_keys)
        .map_err(|err| format!("Register failed: {err}"))?;
    if let Err(err) = rt.block_on(core.register_device(req)) {
        return Err(format!("Register failed: {err}"));
    }
    let auth = rt
        .block_on(core.login_device(nickname.clone(), DEFAULT_DEVICE_ID.to_string()))
        .map_err(|err| format!("Login failed: {err}"))?;

    Ok(LoginSuccess {
        core,
        local_keys,
        history,
        auth,
        server_input: normalized_server,
        status: format!("Logged in as {nickname}"),
    })
}

fn run_open_chat_flow(
    core: ClientCore,
    local_keys: LocalDeviceKeys,
    peer: String,
) -> Result<OpenChatSuccess, String> {
    let rt = runtime();
    let resolved_uuid = rt
        .block_on(core.resolve_user_device(peer.clone(), DEFAULT_DEVICE_ID.to_string()))
        .map_err(|err| format!("Peer not found: {err}"))?;
    let (_key, bundle) = rt
        .block_on(core.derive_peer_shared_key(
            &local_keys,
            peer.clone(),
            DEFAULT_DEVICE_ID.to_string(),
        ))
        .map_err(|err| format!("Open chat failed: {err}"))?;
    Ok(OpenChatSuccess {
        peer,
        resolved_uuid,
        bundle,
    })
}
