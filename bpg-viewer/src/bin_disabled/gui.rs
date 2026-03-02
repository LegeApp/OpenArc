// BPG GUI Viewer with zoom and pan support
use eframe::egui;
use egui::{ColorImage, TextureHandle, Vec2, Pos2, Rect};
use std::path::PathBuf;
use bpg_viewer::decode_file;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0])
            .with_title("BPG Viewer"),
        ..Default::default()
    };

    eframe::run_native(
        "BPG Viewer",
        options,
        Box::new(|cc| {
            // Set up custom fonts if needed
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Box::new(BpgViewerApp::new(cc))
        }),
    )
}

#[derive(Default)]
struct ImageState {
    texture: Option<TextureHandle>,
    original_size: Vec2,
    zoom: f32,
    pan_offset: Vec2,
    dragging: bool,
    last_mouse_pos: Option<Pos2>,
    file_path: Option<PathBuf>,
}

impl ImageState {
    fn reset_view(&mut self) {
        self.zoom = 1.0;
        self.pan_offset = Vec2::ZERO;
    }

    fn fit_to_window(&mut self, available_size: Vec2) {
        if self.original_size.x > 0.0 && self.original_size.y > 0.0 {
            let scale_x = available_size.x / self.original_size.x;
            let scale_y = available_size.y / self.original_size.y;
            self.zoom = scale_x.min(scale_y) * 0.95; // 95% to leave some margin
            self.pan_offset = Vec2::ZERO;
        }
    }

    fn actual_size(&mut self) {
        self.zoom = 1.0;
        self.pan_offset = Vec2::ZERO;
    }

    fn get_display_size(&self) -> Vec2 {
        self.original_size * self.zoom
    }

    fn get_display_rect(&self, center: Pos2) -> Rect {
        let size = self.get_display_size();
        let top_left = center - size / 2.0 + self.pan_offset;
        Rect::from_min_size(top_left.to_pos2(), size)
    }
}

struct BpgViewerApp {
    image: ImageState,
    show_info: bool,
    status_message: String,
    view_mode: ViewMode,
}

#[derive(PartialEq)]
enum ViewMode {
    SingleImage,
    Catalog, // For future implementation
}

impl BpgViewerApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            image: ImageState::default(),
            show_info: true,
            status_message: "No image loaded. Press 'O' to open a file.".to_string(),
            view_mode: ViewMode::SingleImage,
        }
    }

    fn load_image(&mut self, ctx: &egui::Context, path: PathBuf) {
        self.status_message = format!("Loading: {}...", path.display());

        match decode_file(path.to_str().unwrap()) {
            Ok(decoded) => {
                let width = decoded.width as usize;
                let height = decoded.height as usize;

                match decoded.to_rgba32() {
                    Ok(rgba_data) => {
                        let color_image = ColorImage::from_rgba_unmultiplied(
                            [width, height],
                            &rgba_data,
                        );

                        let texture = ctx.load_texture(
                            "loaded_image",
                            color_image,
                            egui::TextureOptions::LINEAR,
                        );

                        self.image.original_size = Vec2::new(width as f32, height as f32);
                        self.image.texture = Some(texture);
                        self.image.file_path = Some(path.clone());
                        self.image.reset_view();

                        self.status_message = format!(
                            "Loaded: {} ({}x{}, {:?})",
                            path.file_name().unwrap().to_string_lossy(),
                            width,
                            height,
                            decoded.format
                        );
                    }
                    Err(e) => {
                        self.status_message = format!("Failed to convert image: {}", e);
                    }
                }
            }
            Err(e) => {
                self.status_message = format!("Failed to load image: {}", e);
            }
        }
    }

    fn open_file_dialog(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("BPG Images", &["bpg"])
            .add_filter("All Files", &["*"])
            .pick_file()
        {
            self.load_image(ctx, path);
        }
    }

    fn render_menu_bar(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open... (O)").clicked() {
                    self.open_file_dialog(ctx);
                    ui.close_menu();
                }

                ui.separator();

                if ui.button("Quit (Q)").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

            ui.menu_button("View", |ui| {
                if ui.button("Fit to Window (F)").clicked() {
                    if let Some(texture) = &self.image.texture {
                        let available_size = ui.available_size();
                        self.image.fit_to_window(available_size);
                    }
                    ui.close_menu();
                }

                if ui.button("Actual Size (1)").clicked() {
                    self.image.actual_size();
                    ui.close_menu();
                }

                if ui.button("Zoom In (+)").clicked() {
                    self.image.zoom *= 1.2;
                    ui.close_menu();
                }

                if ui.button("Zoom Out (-)").clicked() {
                    self.image.zoom /= 1.2;
                    ui.close_menu();
                }

                ui.separator();

                ui.checkbox(&mut self.show_info, "Show Info Panel (I)");
            });

            ui.menu_button("Mode", |ui| {
                if ui.selectable_label(self.view_mode == ViewMode::SingleImage, "Single Image").clicked() {
                    self.view_mode = ViewMode::SingleImage;
                    ui.close_menu();
                }

                if ui.selectable_label(self.view_mode == ViewMode::Catalog, "Catalog View (Coming Soon)").clicked() {
                    // Will be implemented in next step
                    self.status_message = "Catalog view coming soon!".to_string();
                    ui.close_menu();
                }
            });

            ui.menu_button("Help", |ui| {
                if ui.button("Keyboard Shortcuts").clicked() {
                    self.status_message = "O=Open, F=Fit, 1=Actual Size, +/- or Scroll=Zoom, Drag=Pan, I=Info, Q=Quit".to_string();
                    ui.close_menu();
                }

                ui.separator();

                if ui.button("About").clicked() {
                    self.status_message = format!("BPG Viewer v{} - Built with egui", env!("CARGO_PKG_VERSION"));
                    ui.close_menu();
                }
            });
        });
    }

    fn render_info_panel(&self, ui: &mut egui::Ui) {
        egui::SidePanel::right("info_panel")
            .resizable(true)
            .default_width(250.0)
            .show_inside(ui, |ui| {
                ui.heading("Image Info");
                ui.separator();

                if let Some(path) = &self.image.file_path {
                    ui.label(format!("File: {}", path.file_name().unwrap().to_string_lossy()));
                    ui.label(format!("Path: {}", path.parent().unwrap_or(path.as_path()).display()));
                    ui.separator();
                }

                if self.image.texture.is_some() {
                    ui.label(format!("Size: {}x{}",
                        self.image.original_size.x as u32,
                        self.image.original_size.y as u32));
                    ui.label(format!("Zoom: {:.1}%", self.image.zoom * 100.0));
                    ui.label(format!("Display: {:.0}x{:.0}",
                        self.image.get_display_size().x,
                        self.image.get_display_size().y));
                    ui.separator();

                    ui.label("Controls:");
                    ui.label("• Scroll: Zoom");
                    ui.label("• Drag: Pan");
                    ui.label("• F: Fit to window");
                    ui.label("• 1: Actual size");
                    ui.label("• +/-: Zoom in/out");
                } else {
                    ui.colored_label(egui::Color32::GRAY, "No image loaded");
                }

                ui.separator();
                ui.label("Status:");
                ui.colored_label(egui::Color32::LIGHT_BLUE, &self.status_message);
            });
    }

    fn render_image_view(&mut self, ui: &mut egui::Ui) {
        let available_rect = ui.available_rect_before_wrap();
        let center = available_rect.center();

        // Handle keyboard shortcuts
        if ui.input(|i| i.key_pressed(egui::Key::O)) {
            self.open_file_dialog(ui.ctx());
        }
        if ui.input(|i| i.key_pressed(egui::Key::F)) {
            self.image.fit_to_window(available_rect.size());
        }
        if ui.input(|i| i.key_pressed(egui::Key::Num1)) {
            self.image.actual_size();
        }
        if ui.input(|i| i.key_pressed(egui::Key::I)) {
            self.show_info = !self.show_info;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Q)) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ui.input(|i| i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals)) {
            self.image.zoom *= 1.2;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Minus)) {
            self.image.zoom /= 1.2;
        }

        if let Some(texture) = &self.image.texture {
            // Handle mouse scroll for zoom
            let scroll_delta = ui.input(|i| i.scroll_delta.y);
            if scroll_delta != 0.0 {
                let zoom_factor = 1.0 + scroll_delta * 0.001;
                self.image.zoom *= zoom_factor;
                self.image.zoom = self.image.zoom.max(0.1).min(10.0); // Clamp zoom
            }

            // Handle mouse drag for panning
            let response = ui.allocate_rect(available_rect, egui::Sense::click_and_drag());

            if response.dragged() {
                if let Some(last_pos) = self.image.last_mouse_pos {
                    if let Some(current_pos) = response.interact_pointer_pos() {
                        let delta = current_pos - last_pos;
                        self.image.pan_offset += delta;
                    }
                }
                self.image.dragging = true;
            } else {
                self.image.dragging = false;
            }

            self.image.last_mouse_pos = response.interact_pointer_pos();

            // Draw the image
            let display_rect = self.image.get_display_rect(center);

            // Draw background checkerboard for transparency
            let checker_size = 16.0;
            let mut checker_rect = display_rect;
            checker_rect.min.x = (checker_rect.min.x / checker_size).floor() * checker_size;
            checker_rect.min.y = (checker_rect.min.y / checker_size).floor() * checker_size;

            ui.painter().rect_filled(
                display_rect,
                0.0,
                egui::Color32::from_rgb(220, 220, 220),
            );

            // Draw image
            ui.painter().image(
                texture.id(),
                display_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                egui::Color32::WHITE,
            );

            // Draw border around image
            ui.painter().rect_stroke(
                display_rect,
                0.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 100, 100)),
            );

        } else {
            // No image loaded - show drop zone
            let drop_zone = ui.allocate_rect(available_rect, egui::Sense::click());

            ui.painter().rect_filled(
                available_rect,
                0.0,
                egui::Color32::from_rgb(40, 40, 40),
            );

            let text = "Click 'O' to open a BPG file\nor drag and drop a file here";
            let font_id = egui::FontId::proportional(24.0);
            let text_color = egui::Color32::from_rgb(150, 150, 150);

            ui.painter().text(
                center,
                egui::Align2::CENTER_CENTER,
                text,
                font_id,
                text_color,
            );

            if drop_zone.clicked() {
                self.open_file_dialog(ui.ctx());
            }
        }

        // Handle drag and drop files
        ui.ctx().input(|i| {
            if !i.raw.dropped_files.is_empty() {
                if let Some(file) = i.raw.dropped_files.first() {
                    if let Some(path) = &file.path {
                        self.load_image(ui.ctx(), path.clone());
                    }
                }
            }
        });
    }

    fn render_status_bar(&self, ui: &mut egui::Ui) {
        egui::TopBottomPanel::bottom("status_bar")
            .resizable(false)
            .min_height(24.0)
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(&self.status_message);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.image.texture.is_some() {
                            ui.label(format!("Zoom: {:.0}%", self.image.zoom * 100.0));
                            ui.separator();
                            ui.label(format!("{}x{}",
                                self.image.original_size.x as u32,
                                self.image.original_size.y as u32));
                        }
                    });
                });
            });
    }
}

impl eframe::App for BpgViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            self.render_menu_bar(ctx, ui);
        });

        // Status bar
        egui::TopBottomPanel::bottom("status_bar_container").show(ctx, |ui| {
            self.render_status_bar(ui);
        });

        // Main content area
        egui::CentralPanel::default().show(ctx, |ui| {
            // Info panel (if enabled)
            if self.show_info {
                self.render_info_panel(ui);
            }

            // Image viewing area
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(egui::Color32::from_rgb(30, 30, 30)))
                .show_inside(ui, |ui| {
                    self.render_image_view(ui);
                });
        });
    }
}
