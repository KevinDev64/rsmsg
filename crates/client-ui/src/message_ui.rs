use eframe::egui;

use crate::history::{ChatMessage, MessageStatus};

pub fn render_message_bubble(ui: &mut egui::Ui, message: &ChatMessage, peer: &str) {
    let bubble_color = if message.outgoing {
        egui::Color32::from_rgb(56, 120, 255)
    } else {
        egui::Color32::from_rgb(44, 48, 58)
    };
    let text_color = egui::Color32::WHITE;
    let meta = if message.outgoing {
        format!(
            "You · {} · {}",
            format_message_time(message.ts),
            message_status_label(message.status)
        )
    } else {
        format!(
            "@{peer} · {} · {}",
            format_message_time(message.ts),
            message_status_label(message.status)
        )
    };
    let max_width = ui.available_width() * 0.72;
    let display_text = hard_wrap_long_words(&message.text, 48);
    let bubble_width = estimate_bubble_width(&display_text, &meta, max_width);
    let frame = egui::Frame::new()
        .fill(bubble_color)
        .corner_radius(egui::CornerRadius::same(14))
        .inner_margin(egui::Margin::symmetric(12, 8));

    if message.outgoing {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
            frame.show(ui, |ui| {
                render_bubble_content(ui, message, &meta, &display_text, text_color, bubble_width)
            });
        });
    } else {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
            frame.show(ui, |ui| {
                render_bubble_content(ui, message, &meta, &display_text, text_color, bubble_width)
            });
        });
    }
    ui.add_space(6.0);
}

fn render_bubble_content(
    ui: &mut egui::Ui,
    message: &ChatMessage,
    meta: &str,
    text: &str,
    text_color: egui::Color32,
    width: f32,
) {
    ui.set_width(width);
    ui.add(egui::Label::new(egui::RichText::new(meta).small().color(text_color)).wrap());
    ui.add(egui::Label::new(egui::RichText::new(text).color(text_color)).wrap());
    if let (Some(file_name), Some(file_size), Some(data_b64)) = (
        &message.file_name,
        message.file_size,
        &message.file_data_b64,
    ) {
        ui.label(
            egui::RichText::new(format_file_size(file_size))
                .small()
                .color(text_color),
        );
        if ui.button("Save").clicked() {
            if let Some(path) = rfd::FileDialog::new().set_file_name(file_name).save_file() {
                if let Ok(data) =
                    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data_b64)
                {
                    let _ = std::fs::write(path, data);
                }
            }
        }
    }
}

fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn estimate_bubble_width(text: &str, meta: &str, max_width: f32) -> f32 {
    let longest = text
        .lines()
        .chain(meta.lines())
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(1);
    ((longest as f32 * 7.5) + 8.0).clamp(48.0, max_width)
}

fn hard_wrap_long_words(text: &str, max_run: usize) -> String {
    let mut out = String::with_capacity(text.len());
    let mut run = 0_usize;
    for ch in text.chars() {
        if ch.is_whitespace() {
            run = 0;
            out.push(ch);
        } else {
            if run >= max_run {
                out.push('\n');
                run = 0;
            }
            out.push(ch);
            run += 1;
        }
    }
    out
}

fn format_message_time(ts_ms: i64) -> String {
    let seconds = (ts_ms / 1000).rem_euclid(86_400);
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    format!("{hours:02}:{minutes:02}")
}

fn message_status_label(status: MessageStatus) -> &'static str {
    match status {
        MessageStatus::Sending => "sending",
        MessageStatus::Sent => "sent",
        MessageStatus::Delivered => "delivered",
        MessageStatus::Read => "read",
        MessageStatus::Failed => "failed",
    }
}
