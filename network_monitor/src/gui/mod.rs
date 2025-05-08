use crate::config::{Config, NetworkTarget};
use crate::network;
use eframe::{egui, CreationContext};
use egui::{Color32, RichText, Ui, FontId, FontFamily, TextStyle};
use poll_promise::Promise;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

// Network target status information
#[derive(Clone, Debug)]
pub struct TargetStatus {
    pub name: String,
    pub address: String,
    pub port: Option<u16>,
    pub last_check: Instant,
    pub ping_result: Option<Result<Duration, String>>,
    pub port_result: Option<Result<(), String>>,
}

impl TargetStatus {
    pub fn new(target: &NetworkTarget) -> Self {
        Self {
            name: target.name.clone(),
            address: target.address.clone(),
            port: target.port,
            last_check: Instant::now(),
            ping_result: None,
            port_result: None,
        }
    }

    pub fn is_ok(&self) -> bool {
        self.ping_result.as_ref().map_or(false, |r| r.is_ok())
            && (self.port.is_none() || self.port_result.as_ref().map_or(false, |r| r.is_ok()))
    }
}

// GUI application state
pub struct NetworkMonitorApp {
    config: Arc<Mutex<Config>>,
    config_path: String,
    target_statuses: Arc<Mutex<HashMap<String, TargetStatus>>>,
    logs: Vec<(String, Color32)>,
    selected_tab: Tab,
    monitoring_active: bool,
    monitoring_handle: Option<std::thread::JoinHandle<()>>,
    runtime: Arc<Runtime>,
    recovery_in_progress: bool,
    recovery_promise: Option<Promise<Result<(), String>>>,
    show_config_editor: bool,
    config_editor_text: String,
    config_save_error: Option<String>,
}

#[derive(PartialEq)]
enum Tab {
    Status,
    Settings,
    Logs,
}

// Font configuration function
fn configure_fonts(ctx: &egui::Context) {
    // Set font sizes
    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (TextStyle::Heading, FontId::new(22.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(16.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(14.0, FontFamily::Monospace)),
        (TextStyle::Button, FontId::new(16.0, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
    ].into();
    
    ctx.set_style(style);
}

impl NetworkMonitorApp {
    pub fn new(cc: &CreationContext, config_path: String) -> Self {
        // Set default style
        let mut style = (*cc.egui_ctx.style()).clone();
        style.visuals.dark_mode = true;
        cc.egui_ctx.set_style(style);
        
        // Configure fonts
        configure_fonts(&cc.egui_ctx);

        // Load configuration
        let config = match crate::config::load_config(&config_path) {
            Ok(config) => config,
            Err(e) => {
                eprintln!("Failed to load config: {}", e);
                Config::default()
            }
        };

        // Initialize target statuses
        let target_statuses = Arc::new(Mutex::new(HashMap::new()));
        
        // Initialize targets from configuration
        let targets = config.targets.clone();
        for target in targets {
            let status = TargetStatus::new(&target);
            if let Ok(mut statuses) = target_statuses.lock() {
                statuses.insert(target.name.clone(), status);
            }
        }

        // Create tokio runtime
        let runtime = Arc::new(
            Runtime::new().expect("Failed to create Tokio runtime")
        );

        Self {
            config: Arc::new(Mutex::new(config)),
            config_path,
            target_statuses,
            logs: Vec::new(),
            selected_tab: Tab::Status,
            monitoring_active: false,
            monitoring_handle: None,
            runtime,
            recovery_in_progress: false,
            recovery_promise: None,
            show_config_editor: false,
            config_editor_text: String::new(),
            config_save_error: None,
        }
    }

    // Add log message
    fn add_log(&mut self, message: &str, color: Color32) {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        self.logs.push((format!("[{}] {}", timestamp, message), color));
    }

    // Start monitoring
    fn start_monitoring(&mut self) {
        if self.monitoring_active {
            return;
        }

        self.monitoring_active = true;
        self.add_log("Monitoring started", Color32::GREEN);

        let config = self.config.clone();
        let target_statuses = self.target_statuses.clone();
        let runtime = self.runtime.clone();

        let handle = std::thread::spawn(move || {
            while let Ok(config_guard) = config.lock() {
                let check_interval = config_guard.check_interval_sec;
                let ping_timeout = config_guard.ping_timeout_ms;
                let targets = config_guard.targets.clone();
                drop(config_guard); // Release lock before async operations

                for target in targets {
                    let target_name = target.name.clone();
                    let target_address = target.address.clone();
                    let target_port = target.port;

                    // Get or create status
                    let mut status = {
                        if let Ok(mut statuses) = target_statuses.lock() {
                            if let Some(status) = statuses.get_mut(&target_name) {
                                status.clone()
                            } else {
                                let new_status = TargetStatus::new(&target);
                                statuses.insert(target_name.clone(), new_status.clone());
                                new_status
                            }
                        } else {
                            continue; // Skip if can't lock
                        }
                    };

                    // Update last check time
                    status.last_check = Instant::now();

                    // Ping check
                    let ping_result = runtime.block_on(
                        network::ping_host(&target_address, Duration::from_millis(ping_timeout))
                    );
                    // anyhow::Error를 String으로 변환
                    status.ping_result = Some(ping_result.map_err(|e| e.to_string()));

                    // Port check if specified
                    if let Some(port) = target_port {
                        let port_result = runtime.block_on(
                            network::check_port(&target_address, port, Duration::from_millis(ping_timeout))
                        );
                        // anyhow::Error를 String으로 변환
                        status.port_result = Some(port_result.map_err(|e| e.to_string()));
                    }

                    // Update status in shared state
                    if let Ok(mut statuses) = target_statuses.lock() {
                        statuses.insert(target_name, status);
                    }
                }

                // Sleep for check interval
                std::thread::sleep(Duration::from_secs(check_interval));
            }
        });

        self.monitoring_handle = Some(handle);
    }

    // Stop monitoring
    fn stop_monitoring(&mut self) {
        if !self.monitoring_active {
            return;
        }

        self.monitoring_active = false;
        self.add_log("Monitoring stopped", Color32::YELLOW);
        
        // Current thread cannot be stopped, but we use a flag to prevent starting new monitoring
    }

    // Execute recovery actions
    fn perform_recovery(&mut self) {
        if self.recovery_in_progress {
            return;
        }

        self.recovery_in_progress = true;
        self.add_log("Starting recovery actions", Color32::YELLOW);

        let config = self.config.clone();
        let logs = Arc::new(Mutex::new(Vec::new()));
        let runtime = self.runtime.clone();

        self.recovery_promise = Some(Promise::spawn_thread("recovery", move || {
            let config_guard = config.lock().unwrap();
            let config_ref = &*config_guard;

            for action in &config_ref.recovery_actions {
                let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                if let Ok(mut log_vec) = logs.lock() {
                    log_vec.push((
                        format!("[{}] Executing recovery action '{}'", timestamp, action.name),
                        Color32::YELLOW
                    ));
                }

                match runtime.block_on(network::execute_command(&action.command)) {
                    Ok(output) => {
                        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                        if let Ok(mut log_vec) = logs.lock() {
                            log_vec.push((
                                format!("[{}] Recovery action '{}' succeeded: {}", timestamp, action.name, output),
                                Color32::GREEN
                            ));
                        }

                        // Wait if specified
                        if let Some(wait_ms) = action.wait_after_ms {
                            std::thread::sleep(Duration::from_millis(wait_ms));
                        }
                    }
                    Err(e) => {
                        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                        if let Ok(mut log_vec) = logs.lock() {
                            log_vec.push((
                                format!("[{}] Recovery action '{}' failed: {}", timestamp, action.name, e),
                                Color32::RED
                            ));
                        }
                        return Err(format!("Recovery action '{}' failed: {}", action.name, e));
                    }
                }
            }

            Ok(())
        }));
    }

    // Save configuration
    fn save_config(&mut self) {
        // 먼저 설정을 파싱합니다
        let parse_result = toml::from_str::<Config>(&self.config_editor_text);
        
        match parse_result {
            Ok(new_config) => {
                // 설정 파일 저장 시도
                let save_result = crate::config::save_config(&new_config, &self.config_path);
                
                match save_result {
                    Ok(_) => {
                        // 설정 업데이트
                        {
                            if let Ok(mut config) = self.config.lock() {
                                *config = new_config.clone(); // 복사본 사용
                            }
                        }
                        
                        self.show_config_editor = false;
                        self.config_save_error = None;
                        self.add_log("Settings saved successfully", Color32::GREEN);
                        
                        // 대상 상태 업데이트
                        // 설정의 복사본을 사용하여 불변 참조 문제 해결
                        let targets = new_config.targets.clone();
                        
                        {
                            if let Ok(mut statuses) = self.target_statuses.lock() {
                                // 존재하지 않는 대상 제거
                                statuses.retain(|name, _| {
                                    targets.iter().any(|t| t.name == *name)
                                });
                                
                                // 새 대상 추가
                                for target in &targets {
                                    if !statuses.contains_key(&target.name) {
                                        statuses.insert(target.name.clone(), TargetStatus::new(target));
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        self.config_save_error = Some(format!("Failed to save settings: {}", e));
                        self.add_log(&format!("Failed to save settings: {}", e), Color32::RED);
                    }
                }
            }
            Err(e) => {
                self.config_save_error = Some(format!("Failed to parse settings: {}", e));
                self.add_log(&format!("Failed to parse settings: {}", e), Color32::RED);
            }
        }
    }

    // Open settings editor
    fn open_config_editor(&mut self) {
        // 설정을 직렬화하기 전에 먼저 config의 복사본을 만듭니다
        let config_clone = {
            if let Ok(config) = self.config.lock() {
                Some(config.clone())
            } else {
                None
            }
        };
        
        // 복사본이 있으면 직렬화 시도
        if let Some(config) = config_clone {
            match toml::to_string_pretty(&config) {
                Ok(config_str) => {
                    self.config_editor_text = config_str;
                    self.show_config_editor = true;
                    self.config_save_error = None;
                }
                Err(e) => {
                    self.add_log(&format!("Failed to serialize config: {}", e), Color32::RED);
                }
            }
        } else {
            self.add_log("Failed to lock config for editing", Color32::RED);
        }
    }
}

impl eframe::App for NetworkMonitorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check recovery status
        if let Some(promise) = &self.recovery_promise {
            if let Some(result) = promise.ready() {
                match result {
                    Ok(_) => self.add_log("All recovery actions completed", Color32::GREEN),
                    Err(e) => self.add_log(&format!("Recovery action failed: {}", e), Color32::RED),
                }
                self.recovery_in_progress = false;
                self.recovery_promise = None;
            }
        }

        // Top menu bar
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Edit Settings").clicked() {
                        self.open_config_editor();
                        ui.close_menu();
                    }
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                
                ui.menu_button("Monitoring", |ui| {
                    if self.monitoring_active {
                        if ui.button("Stop").clicked() {
                            self.stop_monitoring();
                            ui.close_menu();
                        }
                    } else {
                        if ui.button("Start").clicked() {
                            self.start_monitoring();
                            ui.close_menu();
                        }
                    }
                    
                    ui.separator();
                    
                    if ui.button("Run Recovery Actions").clicked() {
                        self.perform_recovery();
                        ui.close_menu();
                    }
                });
                
                ui.menu_button("Help", |ui| {
                    if ui.button("About").clicked() {
                        self.add_log("Network Monitor v0.1.0", Color32::LIGHT_BLUE);
                        ui.close_menu();
                    }
                });
            });
        });
        
        // Tab bar
        egui::TopBottomPanel::top("tab_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.selected_tab, Tab::Status, "Status");
                ui.selectable_value(&mut self.selected_tab, Tab::Settings, "Settings");
                ui.selectable_value(&mut self.selected_tab, Tab::Logs, "Logs");
            });
        });
        
        // Main content
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.selected_tab {
                Tab::Status => self.render_status_tab(ui),
                Tab::Settings => self.render_settings_tab(ui),
                Tab::Logs => self.render_logs_tab(ui),
            }
        });
        
        // Config editor modal
        if self.show_config_editor {
            egui::Window::new("Settings Editor")
                .fixed_size([600.0, 400.0])
                .show(ctx, |ui| {
                    ui.vertical(|ui| {
                        if let Some(error) = &self.config_save_error {
                            ui.colored_label(Color32::RED, error);
                            ui.separator();
                        }
                        
                        let text_edit = egui::TextEdit::multiline(&mut self.config_editor_text)
                            .font(egui::TextStyle::Monospace)
                            .code_editor()
                            .desired_width(f32::INFINITY);
                        
                        ui.add_sized([ui.available_width(), 320.0], text_edit);
                        
                        ui.horizontal(|ui| {
                            if ui.button("Save").clicked() {
                                self.save_config();
                            }
                            if ui.button("Cancel").clicked() {
                                self.show_config_editor = false;
                                self.config_save_error = None;
                            }
                        });
                    });
                });
        }
    }
}

impl NetworkMonitorApp {
    // Status tab rendering
    fn render_status_tab(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.heading("Network Status");
            
            ui.horizontal(|ui| {
                if self.monitoring_active {
                    if ui.button("Stop Monitoring").clicked() {
                        self.stop_monitoring();
                    }
                } else {
                    if ui.button("Start Monitoring").clicked() {
                        self.start_monitoring();
                    }
                }
                
                if ui.button("Run Recovery Actions").clicked() && !self.recovery_in_progress {
                    self.perform_recovery();
                }
                
                if self.recovery_in_progress {
                    ui.spinner();
                    ui.label("Recovery in progress...");
                }
            });
            
            ui.separator();
            
            // Status grid
            egui::Grid::new("status_grid")
                .num_columns(4)
                .striped(true)
                .spacing([10.0, 5.0])
                .show(ui, |ui| {
                    ui.strong("Target");
                    ui.strong("Address");
                    ui.strong("Status");
                    ui.strong("Response Time");
                    ui.end_row();
                    
                    if let Ok(statuses) = self.target_statuses.lock() {
                        for (_, status) in statuses.iter() {
                            ui.label(&status.name);
                            
                            let address_text = if let Some(port) = status.port {
                                format!("{}:{}", status.address, port)
                            } else {
                                status.address.clone()
                            };
                            ui.label(address_text);
                            
                            // Status indicator
                            if status.is_ok() {
                                ui.colored_label(Color32::GREEN, "Online");
                            } else if status.ping_result.is_some() {
                                ui.colored_label(Color32::RED, "Offline");
                            } else {
                                ui.colored_label(Color32::GRAY, "Unknown");
                            }
                            
                            // Response time
                            if let Some(Ok(duration)) = &status.ping_result {
                                ui.label(format!("{:.2} ms", duration.as_millis()));
                            } else {
                                ui.label("-");
                            }
                            
                            ui.end_row();
                        }
                    }
                });
        });
    }
    
    // Settings tab rendering
    fn render_settings_tab(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.heading("Settings");
            
            if ui.button("Edit Settings").clicked() {
                self.open_config_editor();
            }
            
            ui.separator();
            
            if let Ok(config) = self.config.lock() {
                ui.heading("General Settings");
                
                egui::Grid::new("general_settings_grid")
                    .num_columns(2)
                    .striped(true)
                    .spacing([10.0, 5.0])
                    .show(ui, |ui| {
                        ui.label("Default Target:");
                        ui.label(&config.default_target);
                        ui.end_row();
                        
                        ui.label("Check Interval:");
                        ui.label(format!("{} sec", config.check_interval_sec));
                        ui.end_row();
                        
                        ui.label("Ping Timeout:");
                        ui.label(format!("{} ms", config.ping_timeout_ms));
                        ui.end_row();
                        
                        ui.label("Retry Count:");
                        ui.label(format!("{}", config.retry_count));
                        ui.end_row();
                        
                        ui.label("Log File:");
                        ui.label(config.log_file.as_deref().unwrap_or("None"));
                        ui.end_row();
                        
                        ui.label("Notifications:");
                        ui.label(if config.notification_enabled { "Yes" } else { "No" });
                        ui.end_row();
                    });
                
                ui.separator();
                ui.heading("Network Targets");
                
                egui::Grid::new("targets_grid")
                    .num_columns(3)
                    .striped(true)
                    .spacing([10.0, 5.0])
                    .show(ui, |ui| {
                        ui.strong("Name");
                        ui.strong("Address");
                        ui.strong("Port");
                        ui.end_row();
                        
                        for target in &config.targets {
                            ui.label(&target.name);
                            ui.label(&target.address);
                            ui.label(target.port.map_or("None".to_string(), |p| p.to_string()));
                            ui.end_row();
                        }
                    });
                
                ui.separator();
                ui.heading("Recovery Actions");
                
                egui::Grid::new("recovery_grid")
                    .num_columns(3)
                    .striped(true)
                    .spacing([10.0, 5.0])
                    .show(ui, |ui| {
                        ui.strong("Name");
                        ui.strong("Command");
                        ui.strong("Wait Time");
                        ui.end_row();
                        
                        for action in &config.recovery_actions {
                            ui.label(&action.name);
                            ui.label(&action.command);
                            ui.label(action.wait_after_ms.map_or("None".to_string(), |w| format!("{} ms", w)));
                            ui.end_row();
                        }
                    });
            }
        });
    }
    
    // Logs tab rendering
    fn render_logs_tab(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.heading("Logs");
            
            if ui.button("Clear Logs").clicked() {
                self.logs.clear();
            }
            
            ui.separator();
            
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for (message, color) in &self.logs {
                        ui.colored_label(*color, message);
                    }
                });
        });
    }
}

// Run GUI application
pub fn run_gui(config_path: String) -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Network Monitor",
        options,
        Box::new(|cc| Box::new(NetworkMonitorApp::new(cc, config_path)))
    )
}
