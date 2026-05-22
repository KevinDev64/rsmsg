use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

use client_core::{
    ClientConfig, ClientCore, DecryptedMessage, DeviceAuth, EncryptedMessagePayload,
    LocalDeviceKeys, OutgoingMessageStatus,
};
use eframe::egui;
use sha2::{Digest, Sha256};
use shared::FetchPrekeyBundleResponse;
use uuid::Uuid;

use crate::{
    history::{ChatHistory, ChatMessage, MessageStatus, now_ms},
    message_ui::render_message_bubble,
    settings::{AppSettings, AppTheme},
};

const DEFAULT_DEVICE_ID: &str = "main";
const MAX_FILE_BYTES: usize = 100 * 1024 * 1024;

pub struct MessengerApp {
    core: ClientCore,
    local_keys: Option<LocalDeviceKeys>,
    history: ChatHistory,
    server_input: String,
    nickname: String,
    password: String,
    auth: Option<DeviceAuth>,
    status: String,
    settings: AppSettings,
    settings_open: bool,
    peer_nickname_input: String,
    peer_search_results: Vec<String>,
    selected_chat: String,
    message_input: String,
    key_change_peer: Option<String>,
    login_rx: Option<Receiver<LoginResult>>,
    open_chat_rx: Option<Receiver<OpenChatResult>>,
    search_rx: Option<Receiver<SearchResult>>,
    send_rx: Option<Receiver<SendResult>>,
    sync_rx: Option<Receiver<SyncResult>>,
    read_ack_rx: Option<Receiver<ReadAckResult>>,
    save_file_rx: Option<Receiver<SaveFileResult>>,
    trust_rx: Option<Receiver<TrustResult>>,
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

struct SearchResult {
    result: Result<Vec<String>, String>,
}

struct SendResult {
    chat_name: String,
    message_index: usize,
    result: Result<(), String>,
}

enum SendContent {
    Text(String),
    File { file_name: String, path: PathBuf },
}

struct SyncResult {
    result: Result<SyncSuccess, String>,
}

struct SyncSuccess {
    peer_mappings: Vec<PeerMapping>,
    decrypted: Vec<DecryptedMessage>,
    statuses: Vec<OutgoingMessageStatus>,
}

struct PeerMapping {
    device_uuid: String,
    peer: String,
    bundle: FetchPrekeyBundleResponse,
}

struct ReadAckResult {
    chat_name: String,
    message_ids: Vec<String>,
    result: Result<(), String>,
}

struct SaveFileResult {
    file_name: String,
    result: Result<(), String>,
}

struct TrustResult {
    peer: String,
    result: Result<FetchPrekeyBundleResponse, String>,
}

impl MessengerApp {
    pub fn new() -> Self {
        let settings = AppSettings::load();
        let config = ClientConfig::local_default();
        let server_input = config.http_base.clone();
        let core = ClientCore::new(config);
        let nickname = settings.default_username.clone();
        Self {
            core,
            local_keys: None,
            history: ChatHistory::load(None),
            server_input,
            nickname,
            password: String::new(),
            auth: None,
            status: "Not logged in".to_string(),
            settings,
            settings_open: false,
            peer_nickname_input: String::new(),
            peer_search_results: Vec::new(),
            selected_chat: String::new(),
            message_input: String::new(),
            key_change_peer: None,
            login_rx: None,
            open_chat_rx: None,
            search_rx: None,
            send_rx: None,
            sync_rx: None,
            read_ack_rx: None,
            save_file_rx: None,
            trust_rx: None,
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
                    if self.settings.default_username.trim().is_empty() {
                        self.settings.default_username = self.nickname.clone();
                        self.settings.save();
                    }
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
            let core = self.core.clone();
            thread::spawn(move || {
                let rt = runtime();
                let _ = rt.block_on(core.logout_device(&auth));
            });
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
            self.mark_selected_chat_read(auth);
        }
        self.save_history();
        self.status = format!("Chat with @{} ready", success.peer);
    }

    fn search_users(&mut self) {
        if self.search_rx.is_some() {
            return;
        }
        if self.auth.is_none() {
            self.status = "Log in first".to_string();
            self.peer_search_results.clear();
            return;
        }
        let query = self.peer_nickname_input.clone();
        let core = self.core.clone();
        let (tx, rx) = mpsc::channel();
        self.search_rx = Some(rx);
        self.status = "Searching users...".to_string();
        thread::spawn(move || {
            let rt = runtime();
            let result = rt
                .block_on(core.search_users(query))
                .map_err(|err| format!("Search failed: {err}"));
            let _ = tx.send(SearchResult { result });
        });
    }

    fn poll_search_result(&mut self) {
        let Some(rx) = self.search_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(search) => match search.result {
                Ok(users) => {
                    self.peer_search_results = users;
                    if self.peer_search_results.is_empty() {
                        self.status = "No users found".to_string();
                    } else {
                        self.status = format!("Found {} users", self.peer_search_results.len());
                    }
                }
                Err(err) => self.status = err,
            },
            Err(mpsc::TryRecvError::Empty) => self.search_rx = Some(rx),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.status = "Search worker stopped".to_string();
            }
        }
    }

    fn send_current_message(&mut self) {
        if self.send_rx.is_some() {
            return;
        }
        let Some(auth) = self.auth.clone() else {
            self.status = "Log in first".to_string();
            return;
        };
        if self.selected_chat.is_empty() {
            self.status = "Select chat first".to_string();
            return;
        }
        let Some(peer_device_uuid) = self.selected_chat_session() else {
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
                file_name: None,
                file_size: None,
                file_data_b64: None,
                blob_id: None,
                file_key_b64: None,
            });
            chat.len() - 1
        };
        self.message_input.clear();
        self.save_history();
        let (tx, rx) = mpsc::channel();
        self.send_rx = Some(rx);
        let core = self.core.clone();
        let send_chat_name = chat_name.clone();
        thread::spawn(move || {
            let result = run_send_flow(
                core,
                auth,
                peer_device_uuid,
                SendContent::Text(text),
                message_id,
            );
            let _ = tx.send(SendResult {
                chat_name: send_chat_name,
                message_index,
                result,
            });
        });
    }

    fn send_file(&mut self) {
        if self.send_rx.is_some() {
            return;
        }
        let Some(auth) = self.auth.clone() else {
            self.status = "Log in first".to_string();
            return;
        };
        if self.selected_chat.is_empty() {
            self.status = "Select chat first".to_string();
            return;
        }
        let Some(peer_device_uuid) = self.selected_chat_session() else {
            return;
        };
        let Some(path) = rfd::FileDialog::new().pick_file() else {
            return;
        };
        let Ok(metadata) = std::fs::metadata(&path) else {
            self.status = "Could not inspect selected file".to_string();
            return;
        };
        if metadata.len() > MAX_FILE_BYTES as u64 {
            self.status = "File is too large. Limit is 100 MB".to_string();
            return;
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("file")
            .to_string();
        let chat_name = self.selected_chat.clone();
        let message_id = Uuid::new_v4().to_string();
        let message_index = {
            let chat = self.history.chats.entry(chat_name.clone()).or_default();
            chat.push(ChatMessage {
                outgoing: true,
                text: format!("File: {file_name}"),
                ts: now_ms(),
                status: MessageStatus::Sending,
                message_id: Some(message_id.clone()),
                file_name: Some(file_name.clone()),
                file_size: Some(metadata.len()),
                file_data_b64: None,
                blob_id: None,
                file_key_b64: None,
            });
            chat.len() - 1
        };
        self.save_history();
        let (tx, rx) = mpsc::channel();
        self.send_rx = Some(rx);
        let core = self.core.clone();
        let send_chat_name = chat_name.clone();
        thread::spawn(move || {
            let result = run_send_flow(
                core,
                auth,
                peer_device_uuid,
                SendContent::File { file_name, path },
                message_id,
            );
            let _ = tx.send(SendResult {
                chat_name: send_chat_name,
                message_index,
                result,
            });
        });
    }

    fn save_file_message(&mut self, index: usize) {
        if self.save_file_rx.is_some() {
            return;
        }
        let Some(auth) = self.auth.clone() else {
            return;
        };
        let Some(message) = self
            .history
            .chats
            .get(&self.selected_chat)
            .and_then(|messages| messages.get(index))
            .cloned()
        else {
            return;
        };
        let Some(file_name) = message.file_name.clone() else {
            return;
        };
        let Some(path) = rfd::FileDialog::new().set_file_name(&file_name).save_file() else {
            return;
        };
        if let Some(data_b64) = message.file_data_b64 {
            let (tx, rx) = mpsc::channel();
            self.save_file_rx = Some(rx);
            self.status = format!("Saving {file_name}...");
            thread::spawn(move || {
                let result =
                    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data_b64)
                        .map_err(|err| format!("Could not decode file: {err}"))
                        .and_then(|data| {
                            std::fs::write(path, data)
                                .map_err(|err| format!("Could not save file: {err}"))
                        });
                let _ = tx.send(SaveFileResult { file_name, result });
            });
            return;
        }
        let (Some(blob_id), Some(file_key_b64)) = (message.blob_id, message.file_key_b64) else {
            self.status = "File data is unavailable".to_string();
            return;
        };
        let core = self.core.clone();
        let (tx, rx) = mpsc::channel();
        self.save_file_rx = Some(rx);
        self.status = format!("Saving {file_name}...");
        thread::spawn(move || {
            let result = run_save_file_flow(core, auth, blob_id, file_key_b64, path)
                .map_err(|err| format!("Could not save file: {err}"));
            let _ = tx.send(SaveFileResult { file_name, result });
        });
    }

    fn poll_send_result(&mut self) {
        let Some(rx) = self.send_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(sent) => {
                match sent.result {
                    Ok(()) => {
                        self.update_message_status(
                            &sent.chat_name,
                            sent.message_index,
                            MessageStatus::Sent,
                        );
                        self.status = "Sent".to_string();
                    }
                    Err(err) => {
                        self.update_message_status(
                            &sent.chat_name,
                            sent.message_index,
                            MessageStatus::Failed,
                        );
                        self.status = err;
                    }
                }
                self.save_history();
            }
            Err(mpsc::TryRecvError::Empty) => self.send_rx = Some(rx),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.status = "Send worker stopped".to_string();
            }
        }
    }

    fn poll_save_file_result(&mut self) {
        let Some(rx) = self.save_file_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(saved) => match saved.result {
                Ok(()) => self.status = format!("Saved {}", saved.file_name),
                Err(err) => self.status = err,
            },
            Err(mpsc::TryRecvError::Empty) => self.save_file_rx = Some(rx),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.status = "Save worker stopped".to_string();
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

    fn selected_chat_session(&mut self) -> Option<String> {
        if self.selected_chat.is_empty() {
            self.status = "Select chat first".to_string();
            return None;
        }

        let known_uuid = self
            .history
            .device_uuid_by_peer
            .get(&self.selected_chat)
            .cloned();
        if let Some(known_uuid) = known_uuid {
            if self.core.has_peer_session(&known_uuid) {
                return Some(known_uuid);
            }
        }
        self.status = "Re-open chat before sending".to_string();
        None
    }

    fn sync_incoming(&mut self) {
        if self.sync_rx.is_some() {
            return;
        }
        let Some(auth) = self.auth.clone() else {
            return;
        };
        let Some(local_keys) = self.local_keys.clone() else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        self.sync_rx = Some(rx);
        let core = self.core.clone();
        let peer_by_device_uuid = self.history.peer_by_device_uuid.clone();
        let outgoing_message_ids = self.outgoing_message_ids();
        thread::spawn(move || {
            let result = run_sync_flow(
                core,
                local_keys,
                auth,
                peer_by_device_uuid,
                outgoing_message_ids,
            );
            let _ = tx.send(SyncResult { result });
        });
        self.last_sync_at = Instant::now();
    }

    fn poll_sync_result(&mut self) {
        let Some(rx) = self.sync_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(sync) => {
                if let Ok(success) = sync.result {
                    self.apply_sync_success(success);
                }
            }
            Err(mpsc::TryRecvError::Empty) => self.sync_rx = Some(rx),
            Err(mpsc::TryRecvError::Disconnected) => {}
        }
    }

    fn apply_sync_success(&mut self, success: SyncSuccess) {
        for mapping in success.peer_mappings {
            if !self.verify_or_pin_peer_identity(&mapping.peer, &mapping.bundle) {
                continue;
            }
            self.history
                .peer_by_device_uuid
                .insert(mapping.device_uuid.clone(), mapping.peer.clone());
            self.history
                .device_uuid_by_peer
                .insert(mapping.peer.clone(), mapping.bundle.device_uuid);
            self.history.chats.entry(mapping.peer).or_default();
        }
        for msg in success.decrypted {
            self.push_incoming(msg);
        }
        self.apply_outgoing_statuses(success.statuses);
        if let Some(auth) = self.auth.clone() {
            self.mark_selected_chat_read(auth);
        }
        self.save_history();
    }

    fn mark_selected_chat_read(&mut self, auth: DeviceAuth) {
        if self.read_ack_rx.is_some() {
            return;
        }
        if self.selected_chat.is_empty() {
            return;
        }
        let chat_name = self.selected_chat.clone();
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
        for message in messages {
            if !message.outgoing {
                message.status = MessageStatus::Read;
            }
        }
        self.history.unread_by_peer.remove(&self.selected_chat);
        let core = self.core.clone();
        let ack_ids = message_ids.clone();
        let (tx, rx) = mpsc::channel();
        self.read_ack_rx = Some(rx);
        thread::spawn(move || {
            let rt = runtime();
            let result = rt
                .block_on(core.ack_messages(&auth, ack_ids))
                .map_err(|err| format!("Read ack failed: {err}"));
            let _ = tx.send(ReadAckResult {
                chat_name,
                message_ids,
                result,
            });
        });
    }

    fn poll_read_ack_result(&mut self) {
        let Some(rx) = self.read_ack_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(ack) => {
                if ack.result.is_err() {
                    if let Some(messages) = self.history.chats.get_mut(&ack.chat_name) {
                        for message in messages {
                            if message
                                .message_id
                                .as_deref()
                                .is_some_and(|id| ack.message_ids.iter().any(|ack_id| ack_id == id))
                            {
                                message.status = MessageStatus::Delivered;
                            }
                        }
                    }
                    *self
                        .history
                        .unread_by_peer
                        .entry(ack.chat_name)
                        .or_default() += ack.message_ids.len() as u32;
                }
            }
            Err(mpsc::TryRecvError::Empty) => self.read_ack_rx = Some(rx),
            Err(mpsc::TryRecvError::Disconnected) => {}
        }
    }

    fn outgoing_message_ids(&self) -> Vec<String> {
        self.history
            .chats
            .values()
            .flat_map(|messages| messages.iter())
            .filter(|message| message.outgoing && message.status != MessageStatus::Read)
            .filter_map(|message| message.message_id.clone())
            .collect()
    }

    fn apply_outgoing_statuses(&mut self, statuses: Vec<OutgoingMessageStatus>) {
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
        if self.trust_rx.is_some() {
            return;
        }
        let Some(peer) = self.key_change_peer.clone() else {
            return;
        };
        let Some(local_keys) = self.local_keys.clone() else {
            self.status = "Log in first".to_string();
            return;
        };
        let core = self.core.clone();
        let trust_peer = peer.clone();
        let (tx, rx) = mpsc::channel();
        self.trust_rx = Some(rx);
        self.status = format!("Refreshing @{peer} identity...");
        thread::spawn(move || {
            let rt = runtime();
            let result = rt
                .block_on(core.derive_peer_shared_key(
                    &local_keys,
                    trust_peer.clone(),
                    DEFAULT_DEVICE_ID.to_string(),
                ))
                .map(|(_key, bundle)| bundle)
                .map_err(|err| format!("Could not refresh @{trust_peer} identity: {err}"));
            let _ = tx.send(TrustResult {
                peer: trust_peer,
                result,
            });
        });
    }

    fn poll_trust_result(&mut self) {
        let Some(rx) = self.trust_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(trust) => match trust.result {
                Ok(bundle) => {
                    self.history
                        .peer_identity_key_by_peer
                        .insert(trust.peer.clone(), bundle.identity_key_b64);
                    self.history
                        .peer_signing_identity_key_by_peer
                        .insert(trust.peer.clone(), bundle.signing_identity_key_b64);
                    self.key_change_peer = None;
                    self.save_history();
                    self.status = format!("Trusted new identity for @{}", trust.peer);
                }
                Err(err) => self.status = err,
            },
            Err(mpsc::TryRecvError::Empty) => self.trust_rx = Some(rx),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.status = "Trust worker stopped".to_string();
            }
        }
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
        let payload = serde_json::from_str::<EncryptedMessagePayload>(&msg.plaintext).ok();
        let (text, file_name, file_size, file_data_b64, blob_id, file_key_b64) = match payload {
            Some(EncryptedMessagePayload::File {
                file_name,
                file_size,
                data_b64,
                blob_id,
                file_key_b64,
                ..
            }) => (
                format!("File: {file_name}"),
                Some(file_name),
                Some(file_size),
                data_b64,
                blob_id,
                file_key_b64,
            ),
            None => (msg.plaintext, None, None, None, None, None),
        };
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
            text,
            ts: msg.created_at_unix_ms,
            status: MessageStatus::Delivered,
            message_id: Some(msg.message_id),
            file_name,
            file_size,
            file_data_b64,
            blob_id,
            file_key_b64,
        });
        if self.selected_chat != nick {
            *self.history.unread_by_peer.entry(nick).or_default() += 1;
        }
    }
}

impl eframe::App for MessengerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_theme(ctx);
        self.poll_login_result();
        self.poll_open_chat_result();
        self.poll_search_result();
        self.poll_send_result();
        self.poll_sync_result();
        self.poll_read_ack_result();
        self.poll_save_file_result();
        self.poll_trust_result();
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
                if ui.button("Settings").clicked() {
                    self.settings_open = true;
                }
                if ui.button("Logout").clicked() {
                    self.logout();
                }
                ui.separator();
                ui.collapsing("Security", |ui| {
                    if let Some(local) = self.local_safety_number() {
                        ui.label("Your device fingerprint");
                        ui.monospace(local);
                    }
                    if !self.selected_chat.is_empty() {
                        ui.separator();
                        ui.label(format!("@{} fingerprint", self.selected_chat));
                        if let Some(peer) = self.peer_safety_number(&self.selected_chat) {
                            ui.monospace(peer);
                        } else {
                            ui.label("Open chat to pin identity key");
                        }
                    }
                });
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
                if ui.button("Settings").clicked() {
                    self.settings_open = true;
                }
            }

            ui.separator();
            ui.heading("New chat");
            ui.label("Peer nickname");
            let logged_in = self.auth.is_some();
            ui.add_sized(
                [160.0, 22.0],
                egui::TextEdit::singleline(&mut self.peer_nickname_input).interactive(logged_in),
            );
            let search_busy = self.search_rx.is_some();
            if ui
                .add_enabled(logged_in && !search_busy, egui::Button::new("Search users"))
                .clicked()
            {
                self.search_users();
            }
            for nick in &self.peer_search_results {
                if ui.button(format!("@{nick}")).clicked() {
                    self.peer_nickname_input = nick.clone();
                }
            }
            if ui
                .add_enabled(logged_in, egui::Button::new("Open chat"))
                .clicked()
            {
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
                    if selected {
                        self.selected_chat.clear();
                    } else {
                        self.selected_chat = nick.clone();
                        if let Some(auth) = self.auth.clone() {
                            self.mark_selected_chat_read(auth);
                        }
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
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("Welcome to rsmsg");
                        ui.label("1) Enter your nickname");
                        ui.label("2) Press Register / Login");
                        ui.label("3) Open chat by peer nickname");
                    });
                });
                return;
            }

            if self.selected_chat.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.heading("Select or start chat");
                });
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
                    let mut save_index = None;
                    if let Some(messages) = self.history.chats.get(&self.selected_chat) {
                        for (index, m) in messages.iter().enumerate() {
                            if render_message_bubble(ui, m, &self.selected_chat) {
                                save_index = Some(index);
                            }
                        }
                    }
                    if let Some(index) = save_index {
                        self.save_file_message(index);
                    }
                });

            ui.separator();
            let response = ui.add_sized(
                [ui.available_width(), 56.0],
                egui::TextEdit::multiline(&mut self.message_input).hint_text("Message"),
            );
            let send_by_enter = response.has_focus()
                && ui.input(|input| {
                    input.key_pressed(egui::Key::Enter)
                        && !input.modifiers.shift
                        && !input.modifiers.ctrl
                        && !input.modifiers.command
                });
            if send_by_enter {
                while self.message_input.ends_with(['\n', '\r']) {
                    self.message_input.pop();
                }
                self.send_current_message();
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(self.send_rx.is_none(), egui::Button::new("Send"))
                    .clicked()
                {
                    self.send_current_message();
                }
                if ui
                    .add_enabled(self.send_rx.is_none(), egui::Button::new("Attach file"))
                    .clicked()
                {
                    self.send_file();
                }
            });
        });

        if self.settings_open {
            self.render_settings_window(ctx);
        }
    }
}

impl MessengerApp {
    fn apply_theme(&self, ctx: &egui::Context) {
        match self.settings.theme {
            AppTheme::System => ctx.set_theme(egui::ThemePreference::System),
            AppTheme::Light => ctx.set_theme(egui::ThemePreference::Light),
            AppTheme::Dark => ctx.set_theme(egui::ThemePreference::Dark),
        }
    }

    fn render_settings_window(&mut self, ctx: &egui::Context) {
        let mut open = self.settings_open;
        egui::Window::new("Settings")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(360.0)
            .show(ctx, |ui| {
                ui.heading("Appearance");
                let mut changed = false;
                changed |= ui
                    .radio_value(&mut self.settings.theme, AppTheme::System, "System")
                    .changed();
                changed |= ui
                    .radio_value(&mut self.settings.theme, AppTheme::Light, "Light")
                    .changed();
                changed |= ui
                    .radio_value(&mut self.settings.theme, AppTheme::Dark, "Dark")
                    .changed();
                if changed {
                    self.apply_theme(ctx);
                    self.settings.save();
                }

                ui.separator();
                ui.heading("Profile");
                ui.label("Default username");
                let username_changed = ui
                    .text_edit_singleline(&mut self.settings.default_username)
                    .changed();
                if username_changed {
                    self.settings.default_username =
                        self.settings.default_username.trim().to_string();
                    if self.auth.is_none() {
                        self.nickname = self.settings.default_username.clone();
                    }
                    self.settings.save();
                }
                if ui.button("Use current username").clicked() {
                    self.settings.default_username = self.nickname.trim().to_string();
                    self.settings.save();
                }

                ui.separator();
                ui.heading("Connection");
                ui.label("Server");
                ui.text_edit_singleline(&mut self.server_input);
                if self.auth.is_some() {
                    if ui.button("Apply after logout").clicked() {
                        self.logout();
                        self.apply_server_config();
                    }
                }

                ui.separator();
                ui.heading("About");
                ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                ui.label("Creator KevinDev64 <kevindev56@yandex.ru>");

                ui.separator();
                if ui.button("Close").clicked() {
                    self.settings_open = false;
                }
            });
        self.settings_open = open && self.settings_open;
    }

    fn local_safety_number(&self) -> Option<String> {
        let keys = self.local_keys.as_ref()?;
        Some(format_safety_number(&[
            &keys.identity_public_b64,
            keys.signing_identity_public_b64
                .as_deref()
                .unwrap_or_default(),
        ]))
    }

    fn peer_safety_number(&self, peer: &str) -> Option<String> {
        let identity = self.history.peer_identity_key_by_peer.get(peer)?;
        let signing = self.history.peer_signing_identity_key_by_peer.get(peer)?;
        Some(format_safety_number(&[identity, signing]))
    }
}

fn format_safety_number(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"rsmsg-safety-number-v1");
    for part in parts {
        hasher.update([0]);
        hasher.update(part.as_bytes());
    }
    let digest = hasher.finalize();
    let mut groups = Vec::new();
    for chunk in digest.chunks(4).take(6) {
        let mut bytes = [0_u8; 4];
        bytes[..chunk.len()].copy_from_slice(chunk);
        groups.push(format!("{:05}", u32::from_be_bytes(bytes) % 100_000));
    }
    groups.join(" ")
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

fn run_send_flow(
    core: ClientCore,
    auth: DeviceAuth,
    peer_device_uuid: String,
    content: SendContent,
    message_id: String,
) -> Result<(), String> {
    let rt = runtime();
    let sent = match content {
        SendContent::Text(text) => {
            rt.block_on(core.send_text_to_peer_with_id(&auth, peer_device_uuid, text, message_id))
        }
        SendContent::File { file_name, path } => {
            let data = std::fs::read(path).map_err(|err| format!("File read failed: {err}"))?;
            if data.len() > MAX_FILE_BYTES {
                return Err("File is too large. Limit is 100 MB".to_string());
            }
            rt.block_on(core.send_file_blob_to_peer_with_id(
                &auth,
                peer_device_uuid,
                file_name,
                data,
                message_id,
            ))
        }
    };
    match sent {
        Ok(true) => Ok(()),
        Ok(false) => Err("Peer session missing. Re-open chat.".to_string()),
        Err(err) => Err(format!("Send failed: {err}")),
    }
}

fn run_save_file_flow(
    core: ClientCore,
    auth: DeviceAuth,
    blob_id: String,
    file_key_b64: String,
    path: PathBuf,
) -> Result<(), String> {
    let rt = runtime();
    let data = rt
        .block_on(core.fetch_file_blob(&auth, blob_id, file_key_b64))
        .map_err(|err| format!("fetch failed: {err}"))?;
    std::fs::write(path, data).map_err(|err| format!("write failed: {err}"))
}

fn run_sync_flow(
    core: ClientCore,
    local_keys: LocalDeviceKeys,
    auth: DeviceAuth,
    peer_by_device_uuid: BTreeMap<String, String>,
    outgoing_message_ids: Vec<String>,
) -> Result<SyncSuccess, String> {
    let rt = runtime();
    let pending = rt
        .block_on(core.fetch_pending(&auth, Some(100)))
        .map_err(|err| format!("Sync failed: {err}"))?;
    let mut peer_mappings = Vec::new();
    for item in &pending {
        let peer = if let Some(peer) = peer_by_device_uuid.get(&item.from_device_uuid) {
            Some(peer.clone())
        } else {
            rt.block_on(core.resolve_device_user(item.from_device_uuid.clone()))
                .ok()
        };
        if let Some(peer) = peer {
            if !core.has_peer_session(&item.from_device_uuid) {
                if let Ok((_key, bundle)) = rt.block_on(core.derive_peer_shared_key(
                    &local_keys,
                    peer.clone(),
                    DEFAULT_DEVICE_ID.to_string(),
                )) {
                    peer_mappings.push(PeerMapping {
                        device_uuid: item.from_device_uuid.clone(),
                        peer,
                        bundle,
                    });
                }
            }
        }
    }
    let (decrypted, _ack_ids) = core.decrypt_pending_with_sessions(pending);
    let statuses = if outgoing_message_ids.is_empty() {
        Vec::new()
    } else {
        rt.block_on(core.message_statuses(&auth, outgoing_message_ids))
            .unwrap_or_default()
    };
    Ok(SyncSuccess {
        peer_mappings,
        decrypted,
        statuses,
    })
}
