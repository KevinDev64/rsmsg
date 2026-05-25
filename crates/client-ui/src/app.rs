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
use shared::{CallSignalItem, FetchPrekeyBundleResponse};
use uuid::Uuid;

use crate::{
    history::{ChatHistory, ChatMessage, MessageStatus, now_ms},
    localization::Localization,
    media,
    message_ui::render_message_bubble,
    notifications,
    settings::{AppLanguage, AppSettings, AppTheme},
    tray::{AppTray, TrayCommand},
};

const DEFAULT_DEVICE_ID: &str = "main";
const MAX_FILE_BYTES: usize = 100 * 1024 * 1024;
const CALL_ANSWER_TIMEOUT: Duration = Duration::from_secs(45);

pub struct MessengerApp {
    core: ClientCore,
    local_keys: Option<LocalDeviceKeys>,
    history: ChatHistory,
    server_input: String,
    nickname: String,
    password: String,
    auth: Option<DeviceAuth>,
    status: String,
    tray: Option<AppTray>,
    hidden_to_tray: bool,
    quit_requested: bool,
    settings: AppSettings,
    localization: Localization,
    settings_open: bool,
    create_account_open: bool,
    delete_chat_confirm: Option<String>,
    register_nickname: String,
    register_password: String,
    register_invite_code: String,
    peer_nickname_input: String,
    peer_search_results: Vec<String>,
    blocked_users: Vec<String>,
    selected_chat: String,
    message_input: String,
    key_change_peer: Option<String>,
    login_rx: Option<Receiver<LoginResult>>,
    open_chat_rx: Option<Receiver<OpenChatResult>>,
    search_rx: Option<Receiver<SearchResult>>,
    block_rx: Option<Receiver<BlockResult>>,
    send_rx: Option<Receiver<SendResult>>,
    sync_rx: Option<Receiver<SyncResult>>,
    read_ack_rx: Option<Receiver<ReadAckResult>>,
    save_file_rx: Option<Receiver<SaveFileResult>>,
    trust_rx: Option<Receiver<TrustResult>>,
    call_rx: Option<Receiver<CallResult>>,
    call_signal_rx: Option<Receiver<CallSignalResult>>,
    webrtc_rx: Option<Receiver<WebRtcResult>>,
    active_call: Option<CallState>,
    microphone_devices: Vec<String>,
    speaker_devices: Vec<String>,
    camera_devices: Vec<String>,
    media_session: Option<media::MediaSession>,
    webrtc_session: Option<media::WebRtcSession>,
    audio_playback: Option<media::AudioPlayback>,
    media_failed_call_id: Option<String>,
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
    nickname: String,
    password: String,
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

struct BlockResult {
    action: BlockAction,
    result: Result<Vec<String>, String>,
}

enum BlockAction {
    List,
    Block(String),
    Unblock(String),
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

struct CallResult {
    result: Result<CallState, String>,
}

struct CallState {
    peer: String,
    peer_device_uuid: String,
    call_id: String,
    video: bool,
    microphone_muted: bool,
    camera_disabled: bool,
    incoming: bool,
    accepted: bool,
    signaling_status: String,
    started_at: Instant,
    last_signal_poll_at: Instant,
}

struct CallSignalResult {
    result: Result<Vec<CallSignalItem>, String>,
}

enum WebRtcAction {
    LocalOffer { offer_payload: String },
    LocalAnswer { answer_payload: String },
    RemoteAnswerApplied,
}

struct WebRtcResult {
    result: Result<(media::WebRtcSession, WebRtcAction), String>,
}

impl MessengerApp {
    fn t(&self, key: &str) -> String {
        self.localization.text(key)
    }

    fn tf(&self, key: &str, values: &[(&str, &str)]) -> String {
        let mut text = self.t(key);
        for (name, value) in values {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        text
    }

    fn localize_status_error(&self, error: &str) -> String {
        error
            .replace("invite code already used", &self.t("error.invite_used"))
            .replace("nickname already exists", &self.t("error.nickname_exists"))
            .replace(
                "recipient blocked you",
                &self.t("error.recipient_blocked_you"),
            )
            .replace(
                "you blocked recipient",
                &self.t("error.you_blocked_recipient"),
            )
            .replace("user is not online", &self.t("call.user_offline"))
    }

    fn is_user_blocked(&self, user_id: &str) -> bool {
        self.blocked_users.iter().any(|user| user == user_id)
    }

    fn language_label(&self, language: AppLanguage) -> String {
        match language {
            AppLanguage::System => self.t("settings.language_system"),
            AppLanguage::English => self.t("settings.language_en"),
            AppLanguage::Russian => self.t("settings.language_ru"),
        }
    }

    fn poll_tray(&mut self, ctx: &egui::Context) {
        let Some(tray) = &self.tray else {
            return;
        };
        let mut commands = Vec::new();
        while let Some(command) = tray.try_recv() {
            commands.push(command);
        }
        for command in commands {
            match command {
                TrayCommand::Show => self.show_from_tray(ctx),
                TrayCommand::Quit => {
                    self.quit_requested = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn hide_to_tray(&mut self, ctx: &egui::Context) {
        self.hidden_to_tray = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        self.status = self.t("status.hidden_to_tray");
    }

    fn show_from_tray(&mut self, ctx: &egui::Context) {
        self.hidden_to_tray = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    pub fn new() -> Self {
        let settings = AppSettings::load();
        let localization = Localization::load(settings.language.code());
        let config = ClientConfig::local_default();
        let server_input = config.http_base.clone();
        let core = ClientCore::new(config);
        let nickname = settings.default_username.clone();
        let tray = AppTray::new();
        Self {
            core,
            local_keys: None,
            history: ChatHistory::load(None),
            server_input,
            nickname,
            password: String::new(),
            auth: None,
            status: localization.text("status.not_logged_in"),
            tray,
            hidden_to_tray: false,
            quit_requested: false,
            settings,
            localization,
            settings_open: false,
            create_account_open: false,
            delete_chat_confirm: None,
            register_nickname: String::new(),
            register_password: String::new(),
            register_invite_code: String::new(),
            peer_nickname_input: String::new(),
            peer_search_results: Vec::new(),
            blocked_users: Vec::new(),
            selected_chat: String::new(),
            message_input: String::new(),
            key_change_peer: None,
            login_rx: None,
            open_chat_rx: None,
            search_rx: None,
            block_rx: None,
            send_rx: None,
            sync_rx: None,
            read_ack_rx: None,
            save_file_rx: None,
            trust_rx: None,
            call_rx: None,
            call_signal_rx: None,
            webrtc_rx: None,
            active_call: None,
            microphone_devices: media::microphone_devices(),
            speaker_devices: media::speaker_devices(),
            camera_devices: media::camera_devices(),
            media_session: None,
            webrtc_session: None,
            audio_playback: None,
            media_failed_call_id: None,
            last_sync_at: Instant::now(),
        }
    }

    fn refresh_media_devices(&mut self) {
        self.microphone_devices = media::microphone_devices();
        self.speaker_devices = media::speaker_devices();
        self.camera_devices = media::camera_devices();
    }

    fn media_device_label(&self, value: &str) -> String {
        if value == media::SYSTEM_DEFAULT_DEVICE {
            self.t("call.system_default")
        } else {
            value.to_string()
        }
    }

    fn ice_config(&self) -> media::IceConfig {
        media::IceConfig {
            servers: self
                .settings
                .ice_servers
                .lines()
                .map(str::trim)
                .filter(|server| !server.is_empty())
                .map(ToString::to_string)
                .collect(),
            turn_username: self.settings.turn_username.clone(),
            turn_password: self.settings.turn_password.clone(),
        }
    }

    fn sync_media_session(&mut self) {
        let Some(call) = self.active_call.as_ref() else {
            self.media_session = None;
            self.media_failed_call_id = None;
            return;
        };
        if !call.accepted || call.microphone_muted {
            self.media_session = None;
            return;
        }
        if self.media_session.is_some()
            || self.media_failed_call_id.as_deref() == Some(call.call_id.as_str())
        {
            return;
        }
        let audio_tx = self
            .webrtc_session
            .as_ref()
            .map(media::WebRtcSession::audio_sender);
        match media::start_microphone_capture_with_sender(&self.settings.microphone, audio_tx) {
            Ok(session) => self.media_session = Some(session),
            Err(err) => {
                let media_error = self.tf("call.media_error", &[("error", &err.to_string())]);
                self.media_failed_call_id = Some(call.call_id.clone());
                if let Some(call) = self.active_call.as_mut() {
                    call.signaling_status = media_error;
                }
            }
        }
    }

    fn register_or_login(&mut self, create: bool) {
        if self.login_rx.is_some() {
            return;
        }
        if self.nickname.trim().is_empty() || self.password.len() < 6 {
            self.status = self.t("error.enter_nickname_password");
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.login_rx = Some(rx);
        self.status = if create {
            self.t("status.creating_account")
        } else {
            self.t("status.logging_in")
        };
        let nickname = self.nickname.clone();
        let password = self.password.clone();
        let server_input = self.server_input.clone();
        let language = self.settings.language.code().to_string();
        thread::spawn(move || {
            let result = run_login_flow(create, nickname, password, server_input, None, language);
            let _ = tx.send(LoginResult { result });
        });
    }

    fn create_account(&mut self) {
        if self.login_rx.is_some() {
            return;
        }
        if self.register_nickname.trim().is_empty() || self.register_password.len() < 6 {
            self.status = self.t("error.enter_nickname_password");
            return;
        }
        if self.register_invite_code.trim().is_empty() {
            self.status = self.t("error.enter_invite");
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.login_rx = Some(rx);
        self.status = self.t("status.creating_account");
        let nickname = self.register_nickname.trim().to_string();
        let password = self.register_password.clone();
        let invite_code = self.register_invite_code.trim().to_string();
        let server_input = self.server_input.clone();
        let language = self.settings.language.code().to_string();
        thread::spawn(move || {
            let result = run_login_flow(
                true,
                nickname,
                password,
                server_input,
                Some(invite_code),
                language,
            );
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
                    self.nickname = success.nickname;
                    self.password = success.password;
                    self.server_input = success.server_input;
                    self.status = success.status;
                    self.create_account_open = false;
                    self.register_password.clear();
                    self.register_invite_code.clear();
                    self.refresh_blocked_users();
                    if self.settings.default_username.trim().is_empty() {
                        self.settings.default_username = self.nickname.clone();
                        self.settings.save();
                    }
                }
                Err(err) => self.status = self.localize_status_error(&err),
            },
            Err(mpsc::TryRecvError::Empty) => {
                self.login_rx = Some(rx);
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.status = self.t("status.login_worker_stopped");
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
        self.blocked_users.clear();
        self.password.clear();
        self.status = self.t("status.logged_out");
    }

    fn open_chat(&mut self) {
        if self.open_chat_rx.is_some() {
            return;
        }
        let Some(_auth) = self.auth.clone() else {
            self.status = self.t("error.log_in_first");
            return;
        };
        if self.peer_nickname_input.trim().is_empty() {
            self.status = self.t("status.enter_peer_nickname");
            return;
        }
        let peer = self.peer_nickname_input.trim().to_string();
        let Some(local_keys) = self.local_keys.clone() else {
            self.status = self.t("error.log_in_first");
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
                self.status = self.t("status.open_chat_worker_stopped");
            }
        }
    }

    fn apply_open_chat_success(&mut self, success: OpenChatSuccess) {
        if success.bundle.device_uuid != success.resolved_uuid {
            self.status = self.t("status.peer_resolve_mismatch");
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
            self.status = self.t("error.log_in_first");
            self.peer_search_results.clear();
            return;
        }
        let query = self.peer_nickname_input.clone();
        let core = self.core.clone();
        let (tx, rx) = mpsc::channel();
        self.search_rx = Some(rx);
        self.status = self.t("status.searching_users");
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
                        self.status = self.t("status.no_users_found");
                    } else {
                        self.status = self.tf(
                            "status.found_users",
                            &[("count", &self.peer_search_results.len().to_string())],
                        );
                    }
                }
                Err(err) => self.status = err,
            },
            Err(mpsc::TryRecvError::Empty) => self.search_rx = Some(rx),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.status = self.t("status.search_worker_stopped");
            }
        }
    }

    fn refresh_blocked_users(&mut self) {
        let Some(auth) = self.auth.clone() else {
            return;
        };
        self.run_block_action(auth, BlockAction::List);
    }

    fn block_selected_chat_user(&mut self) {
        let Some(auth) = self.auth.clone() else {
            self.status = self.t("error.log_in_first");
            return;
        };
        if self.selected_chat.is_empty() {
            self.status = self.t("status.select_chat_first");
            return;
        }
        self.run_block_action(auth, BlockAction::Block(self.selected_chat.clone()));
    }

    fn unblock_user(&mut self, user_id: String) {
        let Some(auth) = self.auth.clone() else {
            self.status = self.t("error.log_in_first");
            return;
        };
        self.run_block_action(auth, BlockAction::Unblock(user_id));
    }

    fn request_delete_selected_chat(&mut self) {
        if self.selected_chat.is_empty() {
            self.status = self.t("status.select_chat_first");
            return;
        }
        self.delete_chat_confirm = Some(self.selected_chat.clone());
    }

    fn delete_chat_locally(&mut self, chat_name: String) {
        self.history.chats.remove(&chat_name);
        self.history.unread_by_peer.remove(&chat_name);
        if self.selected_chat == chat_name {
            self.selected_chat.clear();
        }
        self.delete_chat_confirm = None;
        self.save_history();
        self.status = self.tf("status.chat_deleted", &[("user", &chat_name)]);
    }

    fn run_block_action(&mut self, auth: DeviceAuth, action: BlockAction) {
        if self.block_rx.is_some() {
            return;
        }
        let core = self.core.clone();
        let (tx, rx) = mpsc::channel();
        self.block_rx = Some(rx);
        thread::spawn(move || {
            let rt = runtime();
            let result = match &action {
                BlockAction::List => rt
                    .block_on(core.blocked_users(&auth))
                    .map_err(|err| format!("Blocked users refresh failed: {err}")),
                BlockAction::Block(user_id) => rt
                    .block_on(core.block_user(&auth, user_id.clone()))
                    .and_then(|_| rt.block_on(core.blocked_users(&auth)))
                    .map_err(|err| format!("Block failed: {err}")),
                BlockAction::Unblock(user_id) => rt
                    .block_on(core.unblock_user(&auth, user_id.clone()))
                    .and_then(|_| rt.block_on(core.blocked_users(&auth)))
                    .map_err(|err| format!("Unblock failed: {err}")),
            };
            let _ = tx.send(BlockResult { action, result });
        });
    }

    fn poll_block_result(&mut self) {
        let Some(rx) = self.block_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(result) => match result.result {
                Ok(users) => {
                    self.blocked_users = users;
                    match result.action {
                        BlockAction::List => {}
                        BlockAction::Block(user) => {
                            self.status = self.tf("status.user_blocked", &[("user", &user)]);
                        }
                        BlockAction::Unblock(user) => {
                            self.status = self.tf("status.user_unblocked", &[("user", &user)]);
                        }
                    }
                }
                Err(err) => self.status = self.localize_status_error(&err),
            },
            Err(mpsc::TryRecvError::Empty) => self.block_rx = Some(rx),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.status = self.t("status.block_worker_stopped");
            }
        }
    }

    fn send_current_message(&mut self) {
        if self.send_rx.is_some() {
            return;
        }
        let Some(auth) = self.auth.clone() else {
            self.status = self.t("error.log_in_first");
            return;
        };
        if self.selected_chat.is_empty() {
            self.status = self.t("status.select_chat_first");
            return;
        }
        if self.is_user_blocked(&self.selected_chat) {
            self.status = self.t("status.you_blocked_user");
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
            self.status = self.t("error.log_in_first");
            return;
        };
        if self.selected_chat.is_empty() {
            self.status = self.t("status.select_chat_first");
            return;
        }
        if self.is_user_blocked(&self.selected_chat) {
            self.status = self.t("status.you_blocked_user");
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
            self.status = self.t("status.file_too_large");
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

    fn start_call(&mut self, video: bool) {
        if self.call_rx.is_some() || self.active_call.is_some() {
            self.status = self.t("call.busy");
            return;
        }
        let Some(auth) = self.auth.clone() else {
            self.status = self.t("error.log_in_first");
            return;
        };
        if self.selected_chat.is_empty() {
            self.status = self.t("status.select_chat_first");
            return;
        }
        if self.is_user_blocked(&self.selected_chat) {
            self.status = self.t("status.you_blocked_user");
            return;
        }
        let Some(peer_device_uuid) = self.selected_chat_session() else {
            return;
        };
        let peer = self.selected_chat.clone();
        let core = self.core.clone();
        let call_id = Uuid::new_v4().to_string();
        let message_id = Uuid::new_v4().to_string();
        let (tx, rx) = mpsc::channel();
        self.call_rx = Some(rx);
        self.status = self.tf("call.calling", &[("user", &peer)]);
        thread::spawn(move || {
            let result = run_start_call_flow(
                core,
                auth,
                peer.clone(),
                peer_device_uuid,
                call_id,
                video,
                message_id,
            );
            let _ = tx.send(CallResult { result });
        });
    }

    fn poll_call_result(&mut self) {
        let Some(rx) = self.call_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(result) => match result.result {
                Ok(mut call) => {
                    self.status = self.tf("call.started", &[("user", &call.peer)]);
                    call.signaling_status = self.t("call.waiting_answer");
                    self.active_call = Some(call);
                }
                Err(err) => self.status = self.localize_status_error(&err),
            },
            Err(mpsc::TryRecvError::Empty) => self.call_rx = Some(rx),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.status = self.t("call.worker_stopped");
            }
        }
    }

    fn poll_call_signals(&mut self) {
        if let Some(rx) = self.call_signal_rx.take() {
            match rx.try_recv() {
                Ok(result) => match result.result {
                    Ok(signals) => {
                        let accepted_text = self.t("call.accepted");
                        let declined_text = self.t("call.declined");
                        let ended_text = self.t("call.ended");
                        let mut close_status = None;
                        let mut start_offer = false;
                        let mut remote_offer = None;
                        let mut remote_answer = None;
                        if let Some(call) = self.active_call.as_mut() {
                            for signal in signals {
                                match signal.kind.as_str() {
                                    "answer" => {
                                        call.accepted = true;
                                        call.signaling_status = accepted_text.clone();
                                        start_offer = true;
                                    }
                                    "webrtc-offer" => remote_offer = Some(signal.payload),
                                    "webrtc-answer" => remote_answer = Some(signal.payload),
                                    "decline" => {
                                        close_status = Some(declined_text.clone());
                                        break;
                                    }
                                    "busy" => {
                                        close_status = Some(self.t("call.busy"));
                                        break;
                                    }
                                    "hangup" => {
                                        close_status = Some(ended_text.clone());
                                        break;
                                    }
                                    _ => {
                                        call.signaling_status = format!(
                                            "signaling: {} at {}",
                                            signal.kind, signal.created_at_unix_ms
                                        );
                                    }
                                }
                            }
                        }
                        if start_offer {
                            self.start_webrtc_offer();
                        }
                        if let Some(payload) = remote_offer {
                            self.start_webrtc_answer(payload);
                        }
                        if let Some(payload) = remote_answer {
                            self.apply_webrtc_answer(payload);
                        }
                        if let Some(status) = close_status {
                            self.status = status;
                            self.active_call = None;
                            self.call_signal_rx = None;
                            self.stop_webrtc_session();
                        }
                    }
                    Err(err) => {
                        let localized = self.localize_status_error(&err);
                        if let Some(call) = self.active_call.as_mut() {
                            call.signaling_status = localized;
                        }
                    }
                },
                Err(mpsc::TryRecvError::Empty) => self.call_signal_rx = Some(rx),
                Err(mpsc::TryRecvError::Disconnected) => {}
            }
            return;
        }

        let Some(call) = self.active_call.as_mut() else {
            return;
        };
        if !call.incoming && !call.accepted && call.started_at.elapsed() >= CALL_ANSWER_TIMEOUT {
            let Some(auth) = self.auth.clone() else {
                return;
            };
            let core = self.core.clone();
            let call_id = call.call_id.clone();
            let peer_device_uuid = call.peer_device_uuid.clone();
            thread::spawn(move || {
                let rt = runtime();
                let _ = rt.block_on(core.send_call_signal(
                    &auth,
                    call_id,
                    peer_device_uuid,
                    "hangup".to_string(),
                    "{}".to_string(),
                ));
            });
            self.active_call = None;
            self.call_signal_rx = None;
            self.stop_webrtc_session();
            self.status = self.t("call.no_answer");
            return;
        }
        if call.last_signal_poll_at.elapsed() < Duration::from_millis(1200) {
            return;
        }
        let Some(auth) = self.auth.clone() else {
            return;
        };
        call.last_signal_poll_at = Instant::now();
        let core = self.core.clone();
        let call_id = call.call_id.clone();
        let (tx, rx) = mpsc::channel();
        self.call_signal_rx = Some(rx);
        thread::spawn(move || {
            let rt = runtime();
            let result = rt
                .block_on(core.fetch_call_signals(&auth, Some(call_id)))
                .map_err(|err| format!("Call signaling failed: {err}"));
            let _ = tx.send(CallSignalResult { result });
        });
    }

    fn start_webrtc_offer(&mut self) {
        if self.webrtc_session.is_some() || self.webrtc_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.webrtc_rx = Some(rx);
        let ice_config = self.ice_config();
        thread::spawn(move || {
            let rt = runtime();
            let result = rt
                .block_on(media::WebRtcSession::create_offer(ice_config))
                .map(|(session, offer_payload)| {
                    (session, WebRtcAction::LocalOffer { offer_payload })
                })
                .map_err(|err| format!("WebRTC offer failed: {err}"));
            let _ = tx.send(WebRtcResult { result });
        });
    }

    fn start_webrtc_answer(&mut self, offer_payload: String) {
        if self.webrtc_session.is_some() || self.webrtc_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.webrtc_rx = Some(rx);
        let ice_config = self.ice_config();
        thread::spawn(move || {
            let rt = runtime();
            let result = rt
                .block_on(media::WebRtcSession::create_answer(
                    &offer_payload,
                    ice_config,
                ))
                .map(|(session, answer_payload)| {
                    (session, WebRtcAction::LocalAnswer { answer_payload })
                })
                .map_err(|err| format!("WebRTC answer failed: {err}"));
            let _ = tx.send(WebRtcResult { result });
        });
    }

    fn apply_webrtc_answer(&mut self, answer_payload: String) {
        if self.webrtc_rx.is_some() {
            return;
        }
        let Some(session) = self.webrtc_session.take() else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        self.webrtc_rx = Some(rx);
        thread::spawn(move || {
            let rt = runtime();
            let result = rt
                .block_on(session.apply_answer(&answer_payload))
                .map(|_| (session, WebRtcAction::RemoteAnswerApplied))
                .map_err(|err| format!("WebRTC answer apply failed: {err}"));
            let _ = tx.send(WebRtcResult { result });
        });
    }

    fn poll_webrtc_result(&mut self) {
        let Some(rx) = self.webrtc_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(result) => match result.result {
                Ok((session, action)) => {
                    self.audio_playback = media::start_audio_playback(
                        &self.settings.speaker,
                        session.playback_queue(),
                    )
                    .ok();
                    self.webrtc_session = Some(session);
                    match action {
                        WebRtcAction::LocalOffer { offer_payload } => {
                            let status = self.t("call.webrtc_offer_sent");
                            self.send_webrtc_signal("webrtc-offer", offer_payload);
                            if let Some(call) = self.active_call.as_mut() {
                                call.signaling_status = status;
                            }
                        }
                        WebRtcAction::LocalAnswer { answer_payload } => {
                            let status = self.t("call.webrtc_answer_sent");
                            self.send_webrtc_signal("webrtc-answer", answer_payload);
                            if let Some(call) = self.active_call.as_mut() {
                                call.signaling_status = status;
                            }
                        }
                        WebRtcAction::RemoteAnswerApplied => {
                            let status = self.t("call.webrtc_connecting");
                            if let Some(call) = self.active_call.as_mut() {
                                call.signaling_status = status;
                            }
                        }
                    }
                }
                Err(err) => {
                    if let Some(call) = self.active_call.as_mut() {
                        call.signaling_status = err;
                    }
                }
            },
            Err(mpsc::TryRecvError::Empty) => self.webrtc_rx = Some(rx),
            Err(mpsc::TryRecvError::Disconnected) => {}
        }
    }

    fn send_webrtc_signal(&mut self, kind: &str, payload: String) {
        let Some(auth) = self.auth.clone() else {
            return;
        };
        let Some(call) = self.active_call.as_ref() else {
            return;
        };
        let core = self.core.clone();
        let call_id = call.call_id.clone();
        let peer_device_uuid = call.peer_device_uuid.clone();
        let kind = kind.to_string();
        thread::spawn(move || {
            let rt = runtime();
            let _ =
                rt.block_on(core.send_call_signal(&auth, call_id, peer_device_uuid, kind, payload));
        });
    }

    fn stop_webrtc_session(&mut self) {
        self.webrtc_rx = None;
        self.audio_playback = None;
        if let Some(session) = self.webrtc_session.take() {
            thread::spawn(move || {
                let rt = runtime();
                rt.block_on(session.close());
            });
        }
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
            self.status = self.tf("status.saving_file", &[("file_name", &file_name)]);
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
            self.status = self.t("status.file_data_unavailable");
            return;
        };
        let core = self.core.clone();
        let (tx, rx) = mpsc::channel();
        self.save_file_rx = Some(rx);
        self.status = self.tf("status.saving_file", &[("file_name", &file_name)]);
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
                        self.status = self.t("status.sent");
                    }
                    Err(err) => {
                        self.update_message_status(
                            &sent.chat_name,
                            sent.message_index,
                            MessageStatus::Failed,
                        );
                        self.status = self.localize_status_error(&err);
                    }
                }
                self.save_history();
            }
            Err(mpsc::TryRecvError::Empty) => self.send_rx = Some(rx),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.status = self.t("status.send_worker_stopped");
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
            self.status = self.t("status.select_chat_first");
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
        self.status = self.t("status.reopen_chat_before_sending");
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
        let peers_missing_safety = self.peers_missing_safety_numbers();
        let outgoing_message_ids = self.outgoing_message_ids();
        thread::spawn(move || {
            let result = run_sync_flow(
                core,
                local_keys,
                auth,
                peer_by_device_uuid,
                peers_missing_safety,
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
        let mut changed = false;
        for mapping in success.peer_mappings {
            if !self.verify_or_pin_peer_identity(&mapping.peer, &mapping.bundle) {
                continue;
            }
            changed |= self
                .history
                .peer_by_device_uuid
                .insert(mapping.device_uuid.clone(), mapping.peer.clone())
                .as_ref()
                != Some(&mapping.peer);
            changed |= self
                .history
                .device_uuid_by_peer
                .insert(mapping.peer.clone(), mapping.bundle.device_uuid.clone())
                .as_ref()
                != Some(&mapping.bundle.device_uuid);
            if !self.history.chats.contains_key(&mapping.peer) {
                self.history.chats.entry(mapping.peer).or_default();
                changed = true;
            }
        }
        for msg in success.decrypted {
            self.push_incoming(msg);
            changed = true;
        }
        changed |= self.apply_outgoing_statuses(success.statuses);
        if let Some(auth) = self.auth.clone() {
            changed |= self.mark_selected_chat_read(auth);
        }
        if changed {
            self.save_history();
        }
    }

    fn mark_selected_chat_read(&mut self, auth: DeviceAuth) -> bool {
        if self.read_ack_rx.is_some() {
            return false;
        }
        if self.selected_chat.is_empty() {
            return false;
        }
        let chat_name = self.selected_chat.clone();
        let Some(messages) = self.history.chats.get_mut(&self.selected_chat) else {
            return false;
        };
        let message_ids: Vec<String> = messages
            .iter()
            .filter(|message| !message.outgoing && message.status != MessageStatus::Read)
            .filter_map(|message| message.message_id.clone())
            .collect();
        if message_ids.is_empty() {
            self.history.unread_by_peer.remove(&self.selected_chat);
            return false;
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
        true
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

    fn peers_missing_safety_numbers(&self) -> Vec<String> {
        self.history
            .device_uuid_by_peer
            .keys()
            .filter(|peer| self.peer_safety_number(peer).is_none())
            .cloned()
            .collect()
    }

    fn apply_outgoing_statuses(&mut self, statuses: Vec<OutgoingMessageStatus>) -> bool {
        let mut changed = false;
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
                        if message.status != next {
                            message.status = next;
                            changed = true;
                        }
                    }
                }
            }
        }
        changed
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
            self.status = self.t("error.log_in_first");
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
        let (text, file_name, file_size, file_data_b64, blob_id, file_key_b64, incoming_call) =
            match payload {
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
                    None,
                ),
                Some(EncryptedMessagePayload::Call { call_id, video, .. }) => (
                    if video {
                        self.t("call.incoming_video_message")
                    } else {
                        self.t("call.incoming_audio_message")
                    },
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some((call_id, video)),
                ),
                None => (msg.plaintext, None, None, None, None, None, None),
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
        let title = if incoming_call.is_some() {
            self.tf("notification.incoming_call", &[("user", &nick)])
        } else {
            self.tf("notification.new_message", &[("user", &nick)])
        };
        let body = text.clone();
        notifications::notify(&title, &body);
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
            *self.history.unread_by_peer.entry(nick.clone()).or_default() += 1;
        }
        if let Some((call_id, video)) = incoming_call {
            if self.active_call.is_some() {
                if let Some(auth) = self.auth.clone() {
                    let core = self.core.clone();
                    let busy_call_id = call_id.clone();
                    let peer_device_uuid = msg.from_device_uuid.clone();
                    thread::spawn(move || {
                        let rt = runtime();
                        let _ = rt.block_on(core.send_call_signal(
                            &auth,
                            busy_call_id,
                            peer_device_uuid,
                            "busy".to_string(),
                            "{}".to_string(),
                        ));
                    });
                }
                self.status = self.t("call.busy");
                return;
            }
            media::play_call_tone(self.settings.speaker.clone());
            self.active_call = Some(CallState {
                peer: nick,
                peer_device_uuid: msg.from_device_uuid,
                call_id,
                video,
                microphone_muted: false,
                camera_disabled: false,
                incoming: true,
                accepted: false,
                signaling_status: self.t("call.incoming_waiting"),
                started_at: Instant::now(),
                last_signal_poll_at: Instant::now(),
            });
        }
    }
}

impl eframe::App for MessengerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_tray(ctx);
        if self.tray.is_some()
            && ctx.input(|input| input.viewport().close_requested())
            && !self.quit_requested
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.hide_to_tray(ctx);
            return;
        }
        self.apply_theme(ctx);
        self.poll_login_result();
        self.poll_open_chat_result();
        self.poll_search_result();
        self.poll_block_result();
        self.poll_send_result();
        self.poll_sync_result();
        self.poll_read_ack_result();
        self.poll_save_file_result();
        self.poll_trust_result();
        self.poll_call_result();
        self.poll_call_signals();
        self.poll_webrtc_result();
        self.sync_media_session();
        ctx.request_repaint_after(Duration::from_millis(800));
        if self.auth.is_some() && self.last_sync_at.elapsed() >= Duration::from_secs(2) {
            self.sync_incoming();
        }

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(self.t("app.title"));
                ui.separator();
                ui.label(&self.status);
                if let Some(peer) = &self.key_change_peer {
                    ui.separator();
                    ui.label(self.tf("security.key_changed", &[("peer", peer)]));
                    if ui.button(self.t("security.trust_new_key")).clicked() {
                        self.trust_new_peer_identity();
                    }
                }
            });
        });

        egui::SidePanel::left("left").show(ctx, |ui| {
            ui.heading(self.t("account.title"));
            if self.auth.is_some() {
                ui.label(format!("@{}", self.nickname));
                if ui.button(self.t("settings.open")).clicked() {
                    self.settings_open = true;
                }
                if ui.button(self.t("auth.logout")).clicked() {
                    self.logout();
                }
                ui.separator();
                ui.collapsing(self.t("security.title"), |ui| {
                    if let Some(local) = self.local_safety_number() {
                        ui.label(self.t("security.your_fingerprint"));
                        ui.monospace(local);
                    }
                    if !self.selected_chat.is_empty() {
                        ui.separator();
                        ui.label(self.tf(
                            "security.peer_fingerprint",
                            &[("peer", &self.selected_chat)],
                        ));
                        if let Some(peer) = self.peer_safety_number(&self.selected_chat) {
                            ui.monospace(peer);
                        } else {
                            ui.label(self.t("security.open_chat_to_pin"));
                        }
                    }
                });
            } else {
                ui.label(self.t("connection.server"));
                ui.text_edit_singleline(&mut self.server_input);
                ui.label(self.t("auth.nickname"));
                ui.text_edit_singleline(&mut self.nickname);
                ui.label(self.t("auth.password"));
                ui.add(egui::TextEdit::singleline(&mut self.password).password(true));
                let login_busy = self.login_rx.is_some();
                if ui
                    .add_enabled(!login_busy, egui::Button::new(self.t("auth.login")))
                    .clicked()
                {
                    self.register_or_login(false);
                }
                if ui
                    .add_enabled(
                        !login_busy,
                        egui::Button::new(self.t("auth.create_account")),
                    )
                    .clicked()
                {
                    self.register_nickname = self.nickname.clone();
                    self.create_account_open = true;
                }
                if ui.button(self.t("settings.open")).clicked() {
                    self.settings_open = true;
                }
            }

            ui.separator();
            ui.heading(self.t("new_chat.title"));
            ui.label(self.t("chat.peer_nickname"));
            let logged_in = self.auth.is_some();
            ui.add_sized(
                [160.0, 22.0],
                egui::TextEdit::singleline(&mut self.peer_nickname_input).interactive(logged_in),
            );
            let search_busy = self.search_rx.is_some();
            if ui
                .add_enabled(
                    logged_in && !search_busy,
                    egui::Button::new(self.t("chat.search_users")),
                )
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
                .add_enabled(logged_in, egui::Button::new(self.t("chat.open")))
                .clicked()
            {
                self.open_chat();
            }

            ui.separator();
            ui.heading(self.t("chats.title"));
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
                        if let Some(auth) = self.auth.clone()
                            && self.mark_selected_chat_read(auth)
                        {
                            self.save_history();
                        }
                    }
                }
            }
            if ui.button(self.t("chat.sync_incoming")).clicked() {
                self.sync_incoming();
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.auth.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading(self.t("welcome.title"));
                        ui.label(self.t("welcome.step_1"));
                        ui.label(self.t("welcome.step_2"));
                        ui.label(self.t("welcome.step_3"));
                    });
                });
                return;
            }

            if self.selected_chat.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.heading(self.t("placeholder.select_or_start"));
                });
                return;
            }

            let selected_chat = self.selected_chat.clone();
            let selected_blocked = self.is_user_blocked(&selected_chat);
            ui.horizontal(|ui| {
                ui.heading(self.tf("chat.title", &[("peer", &selected_chat)]));
                ui.separator();
                if selected_blocked {
                    if ui.button(self.t("block.unblock_user")).clicked() {
                        self.unblock_user(selected_chat.clone());
                    }
                } else if ui.button(self.t("block.block_user")).clicked() {
                    self.block_selected_chat_user();
                }
                if ui.button(self.t("chat.delete_chat")).clicked() {
                    self.request_delete_selected_chat();
                }
                ui.separator();
                if ui.button(self.t("call.audio_call")).clicked() {
                    self.start_call(false);
                }
                if ui.button(self.t("call.video_call")).clicked() {
                    self.start_call(true);
                }
            });
            if selected_blocked {
                ui.label(self.t("block.you_blocked_banner"));
            }
            ui.separator();

            let composer_height = 128.0;
            let history_height = (ui.available_height() - composer_height).max(120.0);
            let mut save_index = None;
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), history_height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let save_label = self.t("common.save");
                            let you_label = self.t("message.you");
                            if let Some(messages) = self.history.chats.get(&self.selected_chat) {
                                for (index, m) in messages.iter().enumerate() {
                                    if render_message_bubble(
                                        ui,
                                        m,
                                        &self.selected_chat,
                                        &save_label,
                                        &you_label,
                                    ) {
                                        save_index = Some(index);
                                    }
                                }
                            }
                        });
                },
            );
            if let Some(index) = save_index {
                self.save_file_message(index);
            }

            ui.separator();
            let message_hint = self.t("chat.message_hint");
            let response = egui::ScrollArea::vertical()
                .id_salt("message_composer_scroll")
                .max_height(64.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_sized(
                        [ui.available_width(), 64.0],
                        egui::TextEdit::multiline(&mut self.message_input)
                            .desired_rows(3)
                            .hint_text(message_hint),
                    )
                })
                .inner;
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
                    .add_enabled(
                        self.send_rx.is_none(),
                        egui::Button::new(self.t("chat.send")),
                    )
                    .clicked()
                {
                    self.send_current_message();
                }
                if ui
                    .add_enabled(
                        self.send_rx.is_none(),
                        egui::Button::new(self.t("chat.attach_file")),
                    )
                    .clicked()
                {
                    self.send_file();
                }
            });
        });

        if self.settings_open {
            self.render_settings_window(ctx);
        }
        if self.create_account_open {
            self.render_create_account_window(ctx);
        }
        if self.delete_chat_confirm.is_some() {
            self.render_delete_chat_window(ctx);
        }
        if self.active_call.is_some() {
            self.render_call_window(ctx);
        }
        if self.hidden_to_tray {
            ctx.request_repaint_after(Duration::from_millis(500));
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
        egui::Window::new(self.t("settings.title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(360.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(520.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.heading(self.t("settings.appearance"));
                        let theme_system = self.t("settings.theme_system");
                        let theme_light = self.t("settings.theme_light");
                        let theme_dark = self.t("settings.theme_dark");
                        let mut changed = false;
                        changed |= ui
                            .radio_value(&mut self.settings.theme, AppTheme::System, theme_system)
                            .changed();
                        changed |= ui
                            .radio_value(&mut self.settings.theme, AppTheme::Light, theme_light)
                            .changed();
                        changed |= ui
                            .radio_value(&mut self.settings.theme, AppTheme::Dark, theme_dark)
                            .changed();
                        if changed {
                            self.apply_theme(ctx);
                            self.settings.save();
                        }

                        ui.separator();
                        ui.heading(self.t("settings.language"));
                        let selected_language = self.language_label(self.settings.language);
                        let language_system = self.t("settings.language_system");
                        let language_en = self.t("settings.language_en");
                        let language_ru = self.t("settings.language_ru");
                        let language_changed = egui::ComboBox::from_id_salt("language_select")
                            .selected_text(selected_language)
                            .show_ui(ui, |ui| {
                                let mut changed = false;
                                changed |= ui
                                    .selectable_value(
                                        &mut self.settings.language,
                                        AppLanguage::System,
                                        language_system,
                                    )
                                    .changed();
                                changed |= ui
                                    .selectable_value(
                                        &mut self.settings.language,
                                        AppLanguage::English,
                                        language_en,
                                    )
                                    .changed();
                                changed |= ui
                                    .selectable_value(
                                        &mut self.settings.language,
                                        AppLanguage::Russian,
                                        language_ru,
                                    )
                                    .changed();
                                changed
                            })
                            .inner
                            .unwrap_or(false);
                        if language_changed {
                            self.localization = Localization::load(self.settings.language.code());
                            self.settings.save();
                            self.status = self.t("settings.language_updated");
                        }

                        ui.separator();
                        ui.heading(self.t("profile.title"));
                        ui.label(self.t("profile.default_username"));
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
                        if ui.button(self.t("profile.use_current_username")).clicked() {
                            self.settings.default_username = self.nickname.trim().to_string();
                            self.settings.save();
                        }

                        ui.separator();
                        ui.heading(self.t("connection.title"));
                        ui.label(self.t("connection.server"));
                        ui.text_edit_singleline(&mut self.server_input);
                        if self.auth.is_some() {
                            if ui.button(self.t("connection.apply_after_logout")).clicked() {
                                self.logout();
                                self.apply_server_config();
                            }
                        }

                        ui.separator();
                        ui.heading(self.t("privacy.title"));
                        ui.label(self.t("privacy.blocked_users"));
                        if self.blocked_users.is_empty() {
                            ui.label(self.t("privacy.no_blocked_users"));
                        } else {
                            let blocked_users = self.blocked_users.clone();
                            for user in blocked_users {
                                ui.horizontal(|ui| {
                                    ui.label(format!("@{user}"));
                                    if ui.button(self.t("block.unblock_user")).clicked() {
                                        self.unblock_user(user.clone());
                                    }
                                });
                            }
                        }
                        if ui.button(self.t("privacy.refresh_blocked_users")).clicked() {
                            self.refresh_blocked_users();
                        }

                        ui.separator();
                        ui.heading(self.t("call.media_devices"));
                        let system_default = self.t("call.system_default");
                        let no_devices = self.t("call.no_devices_found");
                        ui.label(self.t("call.microphone"));
                        egui::ComboBox::from_id_salt("microphone_select")
                            .selected_text(self.media_device_label(&self.settings.microphone))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.settings.microphone,
                                    media::SYSTEM_DEFAULT_DEVICE.to_string(),
                                    system_default.clone(),
                                );
                                for device in &self.microphone_devices {
                                    ui.selectable_value(
                                        &mut self.settings.microphone,
                                        device.clone(),
                                        device,
                                    );
                                }
                            });
                        if self.microphone_devices.is_empty() {
                            ui.label(&no_devices);
                        }
                        ui.label(self.t("call.speaker"));
                        egui::ComboBox::from_id_salt("speaker_select")
                            .selected_text(self.media_device_label(&self.settings.speaker))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.settings.speaker,
                                    media::SYSTEM_DEFAULT_DEVICE.to_string(),
                                    system_default.clone(),
                                );
                                for device in &self.speaker_devices {
                                    ui.selectable_value(
                                        &mut self.settings.speaker,
                                        device.clone(),
                                        device,
                                    );
                                }
                            });
                        if self.speaker_devices.is_empty() {
                            ui.label(&no_devices);
                        }
                        ui.label(self.t("call.camera"));
                        egui::ComboBox::from_id_salt("camera_select")
                            .selected_text(self.media_device_label(&self.settings.camera))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.settings.camera,
                                    media::SYSTEM_DEFAULT_DEVICE.to_string(),
                                    system_default.clone(),
                                );
                                for device in &self.camera_devices {
                                    ui.selectable_value(
                                        &mut self.settings.camera,
                                        device.clone(),
                                        device,
                                    );
                                }
                            });
                        if self.camera_devices.is_empty() {
                            ui.label(self.t("call.camera_stack_pending"));
                        }
                        if ui.button(self.t("call.refresh_devices")).clicked() {
                            self.refresh_media_devices();
                        }

                        ui.separator();
                        ui.heading(self.t("call.network"));
                        ui.label(self.t("call.ice_servers"));
                        ui.add(
                            egui::TextEdit::multiline(&mut self.settings.ice_servers)
                                .desired_rows(3)
                                .hint_text("stun:stun.l.google.com:19302"),
                        );
                        ui.label(self.t("call.turn_username"));
                        ui.text_edit_singleline(&mut self.settings.turn_username);
                        ui.label(self.t("call.turn_password"));
                        ui.add(
                            egui::TextEdit::singleline(&mut self.settings.turn_password)
                                .password(true),
                        );
                        self.settings.save();

                        ui.separator();
                        ui.heading(self.t("about.title"));
                        ui.label(
                            self.tf("about.version", &[("version", env!("CARGO_PKG_VERSION"))]),
                        );
                        ui.label(self.t("about.creator"));

                        ui.separator();
                        if ui.button(self.t("common.close")).clicked() {
                            self.settings_open = false;
                        }
                    });
            });
        self.settings_open = open && self.settings_open;
    }

    fn render_create_account_window(&mut self, ctx: &egui::Context) {
        let mut open = self.create_account_open;
        egui::Window::new(self.t("auth.create_account"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(380.0)
            .show(ctx, |ui| {
                ui.label(self.t("auth.invite_required"));
                ui.separator();
                ui.label(self.t("auth.nickname"));
                ui.text_edit_singleline(&mut self.register_nickname);
                ui.label(self.t("auth.password"));
                ui.add(egui::TextEdit::singleline(&mut self.register_password).password(true));
                ui.label(self.t("auth.invite_code"));
                ui.text_edit_singleline(&mut self.register_invite_code);
                ui.separator();
                let busy = self.login_rx.is_some();
                if ui
                    .add_enabled(!busy, egui::Button::new(self.t("auth.create_account")))
                    .clicked()
                {
                    self.create_account();
                }
                if ui.button(self.t("common.cancel")).clicked() {
                    self.create_account_open = false;
                }
            });
        self.create_account_open = open && self.create_account_open;
    }

    fn render_delete_chat_window(&mut self, ctx: &egui::Context) {
        let Some(chat_name) = self.delete_chat_confirm.clone() else {
            return;
        };
        let mut open = true;
        egui::Window::new(self.t("chat.delete_chat"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(360.0)
            .show(ctx, |ui| {
                ui.label(self.tf("chat.delete_chat_confirm", &[("user", &chat_name)]));
                ui.separator();
                ui.horizontal(|ui| {
                    if ui
                        .button(self.t("chat.delete_chat_confirm_button"))
                        .clicked()
                    {
                        self.delete_chat_locally(chat_name.clone());
                    }
                    if ui.button(self.t("common.cancel")).clicked() {
                        self.delete_chat_confirm = None;
                    }
                });
            });
        if !open {
            self.delete_chat_confirm = None;
        }
    }

    fn render_call_window(&mut self, ctx: &egui::Context) {
        let mut end_call = false;
        let mut accept_call = false;
        let mut decline_call = false;
        let title = self.t("call.window_title");
        let Some(call) = self.active_call.as_ref() else {
            return;
        };
        let peer = call.peer.clone();
        let peer_device_uuid = call.peer_device_uuid.clone();
        let call_id = call.call_id.clone();
        let video = call.video;
        let incoming = call.incoming;
        let accepted = call.accepted;
        let signaling_status = call.signaling_status.clone();
        let microphone_level = self
            .media_session
            .as_ref()
            .map(media::MediaSession::microphone_level)
            .unwrap_or_default();
        let webrtc_status = self
            .webrtc_session
            .as_ref()
            .map(media::WebRtcSession::status);
        let mut microphone_muted = call.microphone_muted;
        let mut camera_disabled = call.camera_disabled;
        ctx.input(|input| {
            if input.key_pressed(egui::Key::Escape) {
                end_call = true;
            }
            if accepted
                && input.key_pressed(egui::Key::M)
                && (input.modifiers.command || input.modifiers.ctrl)
            {
                microphone_muted = !microphone_muted;
            }
        });
        let active_label = if video {
            self.t("call.video_active")
        } else {
            self.t("call.audio_active")
        };
        let id_label = self.tf("call.id", &[("id", &call_id)]);
        let note_label = if incoming {
            self.t("call.incoming_note")
        } else {
            self.t("call.outgoing_note")
        };
        let unmute_label = self.t("call.unmute_microphone");
        let mute_label = self.t("call.mute_microphone");
        let enable_camera_label = self.t("call.enable_camera");
        let disable_camera_label = self.t("call.disable_camera");
        let accept_label = self.t("call.accept");
        let decline_label = self.t("call.decline");
        let media_active_label = self.t("call.media_active");
        let hang_up_label = self.t("call.hang_up");
        egui::Window::new(title)
            .collapsible(false)
            .resizable(true)
            .default_width(420.0)
            .default_height(260.0)
            .show(ctx, |ui| {
                ui.heading(format!("@{peer}"));
                ui.label(active_label);
                ui.label(id_label);
                ui.label(note_label);
                ui.label(signaling_status);
                if let Some(status) = webrtc_status {
                    ui.label(status);
                }
                if accepted && !microphone_muted {
                    ui.label(media_active_label);
                    ui.add(egui::ProgressBar::new(microphone_level).show_percentage());
                }
                ui.separator();
                if incoming && !accepted {
                    ui.horizontal(|ui| {
                        if ui.button(accept_label).clicked() {
                            accept_call = true;
                        }
                        if ui.button(decline_label).clicked() {
                            decline_call = true;
                        }
                    });
                    ui.separator();
                }
                ui.horizontal(|ui| {
                    let mic_label = if microphone_muted {
                        unmute_label
                    } else {
                        mute_label
                    };
                    if ui
                        .add_enabled(accepted, egui::Button::new(mic_label))
                        .clicked()
                    {
                        microphone_muted = !microphone_muted;
                    }
                    let camera_label = if camera_disabled {
                        enable_camera_label
                    } else {
                        disable_camera_label
                    };
                    if ui
                        .add_enabled(video && accepted, egui::Button::new(camera_label))
                        .clicked()
                    {
                        camera_disabled = !camera_disabled;
                    }
                    if ui.button(hang_up_label).clicked() {
                        end_call = true;
                    }
                });
            });
        if accept_call {
            if let Some(auth) = self.auth.clone() {
                let core = self.core.clone();
                thread::spawn(move || {
                    let rt = runtime();
                    let _ = rt.block_on(core.send_call_signal(
                        &auth,
                        call_id,
                        peer_device_uuid,
                        "answer".to_string(),
                        "{}".to_string(),
                    ));
                });
            }
            let accepted_text = self.t("call.accepted");
            if let Some(call) = self.active_call.as_mut() {
                call.accepted = true;
                call.signaling_status = accepted_text;
            }
        } else if decline_call {
            if let Some(auth) = self.auth.clone() {
                let core = self.core.clone();
                thread::spawn(move || {
                    let rt = runtime();
                    let _ = rt.block_on(core.send_call_signal(
                        &auth,
                        call_id,
                        peer_device_uuid,
                        "decline".to_string(),
                        "{}".to_string(),
                    ));
                });
            }
            self.active_call = None;
            self.call_signal_rx = None;
            self.stop_webrtc_session();
            self.status = self.t("call.declined");
        } else if end_call {
            if let Some(auth) = self.auth.clone() {
                let core = self.core.clone();
                thread::spawn(move || {
                    let rt = runtime();
                    let _ = rt.block_on(core.send_call_signal(
                        &auth,
                        call_id,
                        peer_device_uuid,
                        "hangup".to_string(),
                        "{}".to_string(),
                    ));
                });
            }
            self.active_call = None;
            self.call_signal_rx = None;
            self.stop_webrtc_session();
            self.status = self.t("call.ended");
        } else if let Some(call) = self.active_call.as_mut() {
            if call.microphone_muted != microphone_muted {
                self.media_session = None;
                self.media_failed_call_id = None;
            }
            call.microphone_muted = microphone_muted;
            call.camera_disabled = camera_disabled;
        }
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
    invite_code: Option<String>,
    language: String,
) -> Result<LoginSuccess, String> {
    let config = ClientConfig::for_server(&server_input);
    let normalized_server = config.http_base.clone();
    let core = ClientCore::new(config);
    let rt = runtime();

    if create {
        let invite_code = invite_code.ok_or("Invite code is required".to_string())?;
        match rt.block_on(core.register_user(nickname.clone(), password.clone(), invite_code)) {
            Ok(true) => {}
            Ok(false) => return Err("Account already exists".to_string()),
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

    let localization = Localization::load(&language);
    let status = localization
        .text("status.logged_in_as")
        .replace("{nickname}", &nickname);
    Ok(LoginSuccess {
        core,
        local_keys,
        history,
        auth,
        nickname: nickname.clone(),
        password,
        server_input: normalized_server,
        status,
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

fn run_start_call_flow(
    core: ClientCore,
    auth: DeviceAuth,
    peer: String,
    peer_device_uuid: String,
    call_id: String,
    video: bool,
    message_id: String,
) -> Result<CallState, String> {
    let rt = runtime();
    let online = rt
        .block_on(core.is_user_online(peer.clone(), DEFAULT_DEVICE_ID.to_string()))
        .map_err(|err| format!("Online check failed: {err}"))?;
    if !online {
        return Err("user is not online".to_string());
    }
    let sent = rt.block_on(core.send_call_invite_to_peer_with_id(
        &auth,
        peer_device_uuid.clone(),
        call_id.clone(),
        video,
        message_id,
    ));
    match sent {
        Ok(true) => {
            let _ = rt.block_on(core.send_call_signal(
                &auth,
                call_id.clone(),
                peer_device_uuid.clone(),
                "invite".to_string(),
                format!("{{\"video\":{video}}}"),
            ));
            Ok(CallState {
                peer,
                peer_device_uuid,
                call_id,
                video,
                microphone_muted: false,
                camera_disabled: false,
                incoming: false,
                accepted: false,
                signaling_status: "signaling ready".to_string(),
                started_at: Instant::now(),
                last_signal_poll_at: Instant::now(),
            })
        }
        Ok(false) => Err("Peer session missing. Re-open chat.".to_string()),
        Err(err) => Err(format!("Call failed: {err}")),
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
    peers_missing_safety: Vec<String>,
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
            if !core.has_peer_session(&item.from_device_uuid)
                || peers_missing_safety.iter().any(|missing| missing == &peer)
            {
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
