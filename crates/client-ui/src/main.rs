use client_core::{
    ClientConfig, ClientCore, DecryptedMessage, DeviceAuth, LocalDeviceKeys, PendingEnvelope,
};
use eframe::egui;
use tokio::runtime::Runtime;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "rsmsg",
        options,
        Box::new(|_cc| Ok(Box::new(MessengerApp::new()))),
    )
}

struct MessengerApp {
    runtime: Runtime,
    core: ClientCore,
    local_keys: LocalDeviceKeys,
    user_id: String,
    device_id: String,
    auth: Option<DeviceAuth>,
    status: String,
    peer_user_id: String,
    peer_device_id: String,
    peer_device_uuid: String,
    peer_shared_key_b64: String,
    plaintext_message: String,
    inbox: Vec<PendingEnvelope>,
    decrypted_inbox: Vec<DecryptedMessage>,
}

impl MessengerApp {
    fn new() -> Self {
        let runtime = Runtime::new().expect("tokio runtime");
        let core = ClientCore::new(ClientConfig::local_default());
        let _ = core.healthcheck();
        let local_keys = core.generate_local_device_keys();
        Self {
            runtime,
            core,
            local_keys,
            user_id: String::new(),
            device_id: String::new(),
            auth: None,
            status: "Disconnected".to_string(),
            peer_user_id: String::new(),
            peer_device_id: String::new(),
            peer_device_uuid: String::new(),
            peer_shared_key_b64: String::new(),
            plaintext_message: String::new(),
            inbox: Vec::new(),
            decrypted_inbox: Vec::new(),
        }
    }

    fn register_device(&mut self) {
        let req = self.core.build_register_request(
            self.user_id.clone(),
            self.device_id.clone(),
            &self.local_keys,
        );
        match self.runtime.block_on(self.core.register_device(req)) {
            Ok(uuid) => self.status = format!("Registered device: {uuid}"),
            Err(err) => self.status = format!("Register failed: {err}"),
        }
    }

    fn login_device(&mut self) {
        match self.runtime.block_on(
            self.core
                .login_device(self.user_id.clone(), self.device_id.clone()),
        ) {
            Ok(auth) => {
                self.status = format!("Logged in: {}", auth.device_uuid);
                self.auth = Some(auth);
            }
            Err(err) => self.status = format!("Login failed: {err}"),
        }
    }

    fn derive_peer_key(&mut self) {
        match self.runtime.block_on(self.core.derive_peer_shared_key(
            &self.local_keys,
            self.peer_user_id.clone(),
            self.peer_device_id.clone(),
        )) {
            Ok((key, bundle)) => {
                self.peer_shared_key_b64 = key;
                self.peer_device_uuid = bundle.device_uuid;
                self.status = "Peer session key derived".to_string();
            }
            Err(err) => self.status = format!("Derive key failed: {err}"),
        }
    }

    fn send_message(&mut self) {
        let Some(auth) = self.auth.clone() else {
            self.status = "Login required".to_string();
            return;
        };
        if self.peer_device_uuid.is_empty() {
            self.status = "Derive peer key first".to_string();
            return;
        }

        match self.runtime.block_on(self.core.send_text_to_peer(
            &auth,
            self.peer_device_uuid.clone(),
            self.plaintext_message.clone(),
        )) {
            Ok(true) => {
                self.status = "Message accepted".to_string();
                self.plaintext_message.clear();
            }
            Ok(false) => self.status = "No peer session key; derive first".to_string(),
            Err(err) => self.status = format!("Send failed: {err}"),
        }
    }

    fn pull_pending(&mut self) {
        let Some(auth) = self.auth.clone() else {
            self.status = "Login required".to_string();
            return;
        };
        match self
            .runtime
            .block_on(self.core.fetch_pending(&auth, Some(100)))
        {
            Ok(messages) => {
                let (decrypted, ack_ids) =
                    self.core.decrypt_pending_with_sessions(messages.clone());
                if !ack_ids.is_empty() {
                    let _ = self
                        .runtime
                        .block_on(self.core.ack_messages(&auth, ack_ids));
                }
                self.status = format!("Fetched {} messages", messages.len());
                self.decrypted_inbox = decrypted;
                self.inbox = messages;
            }
            Err(err) => self.status = format!("Fetch failed: {err}"),
        }
    }

    fn ws_drain_once(&mut self) {
        let Some(auth) = self.auth.clone() else {
            self.status = "Login required".to_string();
            return;
        };
        match self.runtime.block_on(self.core.ws_drain_once(&auth)) {
            Ok(messages) => {
                let (decrypted, ack_ids) =
                    self.core.decrypt_pending_with_sessions(messages.clone());
                if !ack_ids.is_empty() {
                    let _ = self
                        .runtime
                        .block_on(self.core.ack_messages(&auth, ack_ids));
                }
                self.status = format!("WS pulled {} messages", messages.len());
                self.decrypted_inbox = decrypted;
                self.inbox = messages;
            }
            Err(err) => self.status = format!("WS failed: {err}"),
        }
    }
}

impl eframe::App for MessengerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("rsmsg client");
                ui.separator();
                ui.label(&self.status);
            });
        });

        egui::SidePanel::left("auth_panel")
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Auth");
                ui.label("User ID");
                ui.text_edit_singleline(&mut self.user_id);
                ui.label("Device ID");
                ui.text_edit_singleline(&mut self.device_id);
                if ui.button("Register").clicked() {
                    self.register_device();
                }
                if ui.button("Login").clicked() {
                    self.login_device();
                }

                if let Some(auth) = &self.auth {
                    ui.separator();
                    ui.label(format!("Device UUID: {}", auth.device_uuid));
                }

                ui.separator();
                ui.heading("Peer Session");
                ui.label("Peer user id");
                ui.text_edit_singleline(&mut self.peer_user_id);
                ui.label("Peer device id");
                ui.text_edit_singleline(&mut self.peer_device_id);
                if ui.button("Derive peer key").clicked() {
                    self.derive_peer_key();
                }
                ui.label(format!("Peer device uuid: {}", self.peer_device_uuid));
                ui.label(format!("Session key: {}", self.peer_shared_key_b64));
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Chat");
            ui.label("Plaintext message");
            ui.text_edit_multiline(&mut self.plaintext_message);

            ui.horizontal(|ui| {
                if ui.button("Send").clicked() {
                    self.send_message();
                }
                if ui.button("Fetch pending").clicked() {
                    self.pull_pending();
                }
                if ui.button("WS drain once").clicked() {
                    self.ws_drain_once();
                }
            });

            ui.separator();
            ui.heading("Inbox (decrypted)");
            egui::ScrollArea::vertical().show(ui, |ui| {
                for msg in &self.decrypted_inbox {
                    ui.group(|ui| {
                        ui.label(format!("id: {}", msg.message_id));
                        ui.label(format!("from: {}", msg.from_device_uuid));
                        ui.label(format!("ts: {}", msg.created_at_unix_ms));
                        ui.label(format!("text: {}", msg.plaintext));
                    });
                }
            });

            ui.separator();
            ui.heading("Inbox (raw envelope)");
            egui::ScrollArea::vertical().show(ui, |ui| {
                for msg in &self.inbox {
                    ui.group(|ui| {
                        ui.label(format!("id: {}", msg.message_id));
                        ui.label(format!("from: {}", msg.from_device_uuid));
                        ui.label(format!("ts: {}", msg.created_at_unix_ms));
                        ui.label(format!("envelope: {}", msg.envelope_b64));
                    });
                }
            });
        });
    }
}
