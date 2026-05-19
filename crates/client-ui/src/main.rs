use client_core::{ClientConfig, ClientCore, DecryptedMessage, DeviceAuth, PendingEnvelope};
use eframe::egui;
use shared::RegisterDeviceRequest;
use tokio::runtime::Runtime;
use uuid::Uuid;

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
    user_id: String,
    device_id: String,
    identity_key_b64: String,
    signed_prekey_b64: String,
    auth: Option<DeviceAuth>,
    status: String,
    to_device_uuid: String,
    plaintext_message: String,
    shared_key_b64: String,
    inbox: Vec<PendingEnvelope>,
    decrypted_inbox: Vec<DecryptedMessage>,
}

impl MessengerApp {
    fn new() -> Self {
        let runtime = Runtime::new().expect("tokio runtime");
        let core = ClientCore::new(ClientConfig::local_default());
        let _ = core.healthcheck();
        let shared_key_b64 = core.generate_shared_key_b64();
        Self {
            runtime,
            core,
            user_id: String::new(),
            device_id: String::new(),
            identity_key_b64: "AQ==".to_string(),
            signed_prekey_b64: "AQ==".to_string(),
            auth: None,
            status: "Disconnected".to_string(),
            to_device_uuid: String::new(),
            plaintext_message: String::new(),
            shared_key_b64,
            inbox: Vec::new(),
            decrypted_inbox: Vec::new(),
        }
    }

    fn register_device(&mut self) {
        let req = RegisterDeviceRequest {
            user_id: self.user_id.clone(),
            device_id: self.device_id.clone(),
            identity_key_b64: self.identity_key_b64.clone(),
            signed_prekey_b64: self.signed_prekey_b64.clone(),
        };
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

    fn send_message(&mut self) {
        let Some(auth) = self.auth.clone() else {
            self.status = "Login required".to_string();
            return;
        };
        let Ok(_) = Uuid::parse_str(&self.to_device_uuid) else {
            self.status = "Invalid recipient uuid".to_string();
            return;
        };
        match self.runtime.block_on(self.core.send_text_message(
            &auth,
            self.to_device_uuid.clone(),
            self.plaintext_message.clone(),
            &self.shared_key_b64,
        )) {
            Ok(true) => {
                self.status = "Message accepted".to_string();
                self.plaintext_message.clear();
            }
            Ok(false) => self.status = "Duplicate message rejected".to_string(),
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
                self.status = format!("Fetched {} messages", messages.len());
                self.decrypted_inbox = self
                    .core
                    .decrypt_pending(messages.clone(), &self.shared_key_b64);
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
                self.status = format!("WS pulled {} messages", messages.len());
                self.decrypted_inbox = self
                    .core
                    .decrypt_pending(messages.clone(), &self.shared_key_b64);
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
                ui.label("Identity key b64");
                ui.text_edit_singleline(&mut self.identity_key_b64);
                ui.label("Signed prekey b64");
                ui.text_edit_singleline(&mut self.signed_prekey_b64);

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
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Chat");
            ui.label("Recipient device uuid");
            ui.text_edit_singleline(&mut self.to_device_uuid);
            ui.label("Shared key b64");
            ui.text_edit_singleline(&mut self.shared_key_b64);
            ui.horizontal(|ui| {
                if ui.button("Generate key").clicked() {
                    self.shared_key_b64 = self.core.generate_shared_key_b64();
                }
            });
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
