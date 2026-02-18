// src/gui_main.rs
// myshell GUI - a standalone windowed terminal emulator

use eframe::egui::{self, Color32, FontId, Key, Modifiers, RichText, ScrollArea, TextEdit, Vec2};
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;

/// Shared terminal output buffer
type OutputBuffer = Arc<Mutex<String>>;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("myshell")
            .with_inner_size([900.0, 600.0])
            .with_min_inner_size([400.0, 300.0])
            .with_icon(load_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "myshell",
        options,
        Box::new(|cc| {
            // Set up dark theme
            setup_theme(&cc.egui_ctx);
            Ok(Box::new(TerminalApp::new()))
        }),
    )
}

fn load_icon() -> egui::IconData {
    // Placeholder 1x1 pixel icon - replace with real icon data
    egui::IconData {
        rgba: vec![0, 0, 0, 255],
        width: 1,
        height: 1,
    }
}

fn setup_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    // Dark terminal colors
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = Color32::from_rgb(15, 15, 20);
    style.visuals.window_fill = Color32::from_rgb(15, 15, 20);
    style.visuals.extreme_bg_color = Color32::from_rgb(10, 10, 15);
    style.visuals.override_text_color = Some(Color32::from_rgb(220, 220, 210));

    ctx.set_style(style);

    // Load a monospace font
    let mut fonts = egui::FontDefinitions::default();
    ctx.set_fonts(fonts);
}

struct TerminalApp {
    /// The text currently typed in the input bar
    input: String,
    /// All terminal output lines
    output: OutputBuffer,
    /// Write end to send data to the PTY (shell stdin)
    pty_writer: Arc<Mutex<Box<dyn Write + Send>>>,
    /// Whether we should scroll to bottom
    scroll_to_bottom: bool,
    /// Command history
    history: Vec<String>,
    history_index: Option<usize>,
    /// Cursor position tracking
    cursor_line: usize,
}

impl TerminalApp {
    fn new() -> Self {
        let output: OutputBuffer = Arc::new(Mutex::new(String::new()));

        // --- Spawn a PTY with myshell (or bash as fallback) ---
        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty(PtySize {
                rows: 40,
                cols: 200,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("failed to open PTY");

        // Build command - try to use myshell itself, fallback to bash
        let mut cmd = CommandBuilder::new("bash");
        // You'd replace "bash" with the path to your compiled myshell binary later:
        // let mut cmd = CommandBuilder::new("/path/to/myshell");

        let mut child = pair.slave
            .spawn_command(cmd)
            .expect("failed to spawn shell");

        // PTY writer (we send keypresses/commands here)
        let pty_writer = Arc::new(Mutex::new(
            pair.master.take_writer().expect("failed to get pty writer")
        ));

        // PTY reader thread - reads shell output and appends to buffer
        let output_clone = output.clone();
        let mut reader = pair.master.try_clone_reader().expect("failed to get pty reader");

        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let text = String::from_utf8_lossy(&buf[..n]);
                        // Strip basic ANSI escape codes for simple rendering
                        let clean = strip_ansi(&text);
                        if let Ok(mut out) = output_clone.lock() {
                            out.push_str(&clean);
                            // Keep buffer from growing forever (keep last 100KB)
                            if out.len() > 100_000 {
                                let trim_at = out.len() - 80_000;
                                *out = out[trim_at..].to_string();
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // Watch for child exit
        thread::spawn(move || {
            child.wait().ok();
        });

        TerminalApp {
            input: String::new(),
            output,
            pty_writer,
            scroll_to_bottom: true,
            history: Vec::new(),
            history_index: None,
            cursor_line: 0,
        }
    }

    fn send_input(&mut self) {
        let line = self.input.trim_end_matches('\n').to_string();
        if !line.is_empty() {
            self.history.push(line.clone());
            self.history_index = None;
        }
        let to_send = format!("{}\n", line);
        self.input.clear();
        self.scroll_to_bottom = true;

        if let Ok(mut writer) = self.pty_writer.lock() {
            writer.write_all(to_send.as_bytes()).ok();
            writer.flush().ok();
        }
    }

    fn send_raw(&mut self, bytes: &[u8]) {
        if let Ok(mut writer) = self.pty_writer.lock() {
            writer.write_all(bytes).ok();
            writer.flush().ok();
        }
    }

    fn history_up(&mut self) {
        if self.history.is_empty() { return; }
        let idx = match self.history_index {
            None => self.history.len() - 1,
            Some(i) if i > 0 => i - 1,
            Some(i) => i,
        };
        self.history_index = Some(idx);
        self.input = self.history[idx].clone();
    }

    fn history_down(&mut self) {
        match self.history_index {
            None => {}
            Some(i) if i + 1 < self.history.len() => {
                self.history_index = Some(i + 1);
                self.input = self.history[i + 1].clone();
            }
            _ => {
                self.history_index = None;
                self.input.clear();
            }
        }
    }
}

impl eframe::App for TerminalApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Repaint frequently to catch new PTY output
        ctx.request_repaint_after(std::time::Duration::from_millis(16));

        // Top menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("Terminal", |ui| {
                    if ui.button("New Tab (coming soon)").clicked() { ui.close_menu(); }
                    if ui.button("Clear").clicked() {
                        if let Ok(mut out) = self.output.lock() { out.clear(); }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() { std::process::exit(0); }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.button("Copy").clicked() { ui.close_menu(); }
                    if ui.button("Paste").clicked() { ui.close_menu(); }
                });
            });
        });

        // Bottom input bar
        egui::TopBottomPanel::bottom("input_bar")
            .min_height(36.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("❯")
                            .color(Color32::from_rgb(80, 200, 120))
                            .font(FontId::monospace(14.0))
                    );

                    let input_id = egui::Id::new("terminal_input");
                    let response = ui.add(
                        TextEdit::singleline(&mut self.input)
                            .id(input_id)
                            .font(FontId::monospace(14.0))
                            .text_color(Color32::from_rgb(220, 220, 210))
                            .frame(false)
                            .desired_width(f32::INFINITY)
                    );

                    // Auto-focus the input
                    if !response.has_focus() {
                        ctx.memory_mut(|m| m.request_focus(input_id));
                    }

                    // Handle key events on the input
                    if response.has_focus() {
                        ctx.input(|i| {
                            for event in &i.events {
                                match event {
                                    egui::Event::Key { key: Key::Enter, pressed: true, .. } => {
                                        self.send_input();
                                    }
                                    egui::Event::Key { key: Key::ArrowUp, pressed: true, .. } => {
                                        self.history_up();
                                    }
                                    egui::Event::Key { key: Key::ArrowDown, pressed: true, .. } => {
                                        self.history_down();
                                    }
                                    egui::Event::Key { key: Key::C, pressed: true, modifiers, .. }
                                        if modifiers.ctrl => {
                                        self.send_raw(b"\x03"); // Ctrl+C
                                    }
                                    egui::Event::Key { key: Key::D, pressed: true, modifiers, .. }
                                        if modifiers.ctrl => {
                                        self.send_raw(b"\x04"); // Ctrl+D
                                    }
                                    egui::Event::Key { key: Key::L, pressed: true, modifiers, .. }
                                        if modifiers.ctrl => {
                                        if let Ok(mut out) = self.output.lock() { out.clear(); }
                                    }
                                    _ => {}
                                }
                            }
                        });
                    }
                });
            });

        // Main terminal output area
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Color32::from_rgb(15, 15, 20)).inner_margin(8.0))
            .show(ctx, |ui| {
                let scroll = ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(self.scroll_to_bottom);

                scroll.show(ui, |ui| {
                    let output = self.output.lock().unwrap().clone();
                    ui.add(
                        egui::Label::new(
                            RichText::new(&output)
                                .font(FontId::monospace(13.0))
                                .color(Color32::from_rgb(204, 204, 178))
                        ).wrap()
                    );
                });

                self.scroll_to_bottom = false;
            });
    }
}

/// Very basic ANSI escape code stripper
/// A real terminal would parse and render these as colors
fn strip_ansi(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip until end of escape sequence
            if chars.peek() == Some(&'[') {
                chars.next();
                // Skip until we hit a letter (end of CSI sequence)
                while let Some(&ch) = chars.peek() {
                    chars.next();
                    if ch.is_ascii_alphabetic() { break; }
                }
            } else {
                // Skip other escape types
                chars.next();
            }
        } else {
            result.push(c);
        }
    }
    result
}