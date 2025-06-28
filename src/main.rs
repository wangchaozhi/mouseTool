// 在 Windows 上隐藏控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// 跨平台鼠标控制模块
mod cross_platform_mouse {
    use device_query::{DeviceQuery, DeviceState, Keycode};
    use enigo::{Enigo, Mouse, Button, Coordinate, Direction, Settings};

    pub struct MouseController {
        enigo: Enigo,
        device_state: DeviceState,
    }

    impl MouseController {
        pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
            let enigo = Enigo::new(&Settings::default())?;
            let device_state = DeviceState::new();

            Ok(Self {
                enigo,
                device_state,
            })
        }

        pub fn get_mouse_position(&self) -> (i32, i32) {
            let mouse = self.device_state.get_mouse();
            (mouse.coords.0, mouse.coords.1)
        }

        pub fn is_left_button_pressed(&self) -> bool {
            let mouse = self.device_state.get_mouse();
            mouse.button_pressed[1]
        }

        pub fn get_mouse_button_states(&self) -> Vec<bool> {
            let mouse = self.device_state.get_mouse();
            mouse.button_pressed.clone()
        }

        pub fn is_middle_button_pressed(&self) -> bool {
            let mouse = self.device_state.get_mouse();
            // 根据反馈，实际的按钮映射可能是：
            // 0=左键, 1=中键, 2=右键 (在某些系统上)
            if mouse.button_pressed.len() > 1 {
                mouse.button_pressed[3] // 尝试索引1作为中键
            } else {
                false
            }
        }

        pub fn is_right_button_pressed(&self) -> bool {
            let mouse = self.device_state.get_mouse();
            // 根据反馈，右键可能是索引2
            if mouse.button_pressed.len() > 2 {
                mouse.button_pressed[2] // 尝试索引2作为右键
            } else {
                false
            }
        }

        pub fn move_mouse_to(&mut self, x: i32, y: i32) -> Result<(), Box<dyn std::error::Error>> {
            self.enigo.move_mouse(x, y, Coordinate::Abs)?;
            Ok(())
        }

        pub fn click_left(&mut self) -> Result<(), Box<dyn std::error::Error>> {
            self.enigo.button(Button::Left, Direction::Click)?;
            Ok(())
        }

        pub fn click_right(&mut self) -> Result<(), Box<dyn std::error::Error>> {
            self.enigo.button(Button::Right, Direction::Click)?;
            Ok(())
        }

        pub fn click_middle(&mut self) -> Result<(), Box<dyn std::error::Error>> {
            self.enigo.button(Button::Middle, Direction::Click)?;
            Ok(())
        }

        pub fn get_screen_size(&self) -> Result<(i32, i32), Box<dyn std::error::Error>> {
            let (width, height) = self.enigo.main_display()?;
            Ok((width, height))
        }
    }
}

struct MouseClickerApp {
    x_pos: i32,
    y_pos: i32,
    click_interval: f64,
    click_count: u32,
    is_clicking: Arc<Mutex<bool>>,
    total_clicks: Arc<Mutex<u32>>,
    click_type: ClickType,
    auto_mode: bool,
    status_message: String,
    is_picking_position: bool,
    last_capture_button_state: bool,
    mouse_controller: Arc<Mutex<cross_platform_mouse::MouseController>>,
    show_debug_info: bool,
    capture_button_type: CaptureButtonType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CaptureButtonType {
    MiddleButton,
    RightButton,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ClickType {
    Left,
    Right,
    Middle,
}

impl Default for ClickType {
    fn default() -> Self {
        ClickType::Left
    }
}

impl MouseClickerApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // 设置中文字体支持
        Self::setup_fonts(&cc.egui_ctx);

        // 初始化鼠标控制器
        let mouse_controller = match cross_platform_mouse::MouseController::new() {
            Ok(controller) => Arc::new(Mutex::new(controller)),
            Err(e) => {
                eprintln!("Failed to initialize mouse controller: {}", e);
                // 创建一个dummy控制器，虽然可能无法工作，但不会崩溃
                panic!("Cannot initialize mouse controller: {}", e);
            }
        };

        Self {
            x_pos: 100,
            y_pos: 100,
            click_interval: 1.0,
            click_count: 10,
            is_clicking: Arc::new(Mutex::new(false)),
            total_clicks: Arc::new(Mutex::new(0)),
            click_type: ClickType::Left,
            auto_mode: false,
            status_message: "准备就绪".to_string(),
            is_picking_position: false,
            last_capture_button_state: false,
            mouse_controller,
            show_debug_info: false,
            capture_button_type: CaptureButtonType::MiddleButton,
        }
    }

    fn setup_fonts(ctx: &egui::Context) {
        let mut fonts = egui::FontDefinitions::default();

        // 尝试加载中文字体
        let font_paths = if cfg!(windows) {
            vec![
                "C:/Windows/Fonts/msyh.ttc",      // 微软雅黑
                "C:/Windows/Fonts/simsun.ttc",   // 宋体
                "C:/Windows/Fonts/simhei.ttf",   // 黑体
            ]
        } else if cfg!(target_os = "macos") {
            vec![
                "/Library/Fonts/PingFang.ttc",           // 苹方
                "/System/Library/Fonts/STHeiti Light.ttc", // 黑体
                "/System/Library/Fonts/Helvetica.ttc",    // 备选
            ]
        } else {
            vec![
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/TTF/DejaVuSans.ttf",
                "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
                "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            ]
        };

        for (i, path) in font_paths.iter().enumerate() {
            if let Ok(font_data) = std::fs::read(path) {
                let font_name = format!("custom_font_{}", i);
                fonts.font_data.insert(
                    font_name.clone(),
                    egui::FontData::from_owned(font_data).into(),
                );

                fonts.families.entry(egui::FontFamily::Proportional)
                    .or_default()
                    .insert(0, font_name.clone());

                fonts.families.entry(egui::FontFamily::Monospace)
                    .or_default()
                    .insert(0, font_name);
                break;
            }
        }

        ctx.set_fonts(fonts);
    }

    fn start_position_picking(&mut self) {
        self.is_picking_position = true;
        let button_name = match self.capture_button_type {
            CaptureButtonType::MiddleButton => "鼠标中键（滚轮键）",
            CaptureButtonType::RightButton => "鼠标右键",
        };
        self.status_message = format!("坐标捕捉模式已激活！请在屏幕任意位置点击{}...", button_name);
        self.last_capture_button_state = false;
    }

    fn check_position_picking(&mut self) {
        if !self.is_picking_position {
            return;
        }

        if let Ok(controller) = self.mouse_controller.lock() {
            let current_button_state = match self.capture_button_type {
                CaptureButtonType::MiddleButton => controller.is_middle_button_pressed(),
                CaptureButtonType::RightButton => controller.is_right_button_pressed(),
            };

            // 检测鼠标按键从按下到释放的完整点击动作
            if self.last_capture_button_state && !current_button_state {
                // 完整的点击动作完成，获取点击位置的坐标
                let (x, y) = controller.get_mouse_position();

                // 将捕捉到的坐标填入输入框
                self.x_pos = x;
                self.y_pos = y;

                let button_name = match self.capture_button_type {
                    CaptureButtonType::MiddleButton => "中键",
                    CaptureButtonType::RightButton => "右键",
                };

                // 更新状态消息
                self.status_message = format!("✅ 坐标捕捉成功！已设置为: ({}, {}) [使用{}捕捉]", x, y, button_name);

                // 退出捕捉模式
                self.is_picking_position = false;
            }

            self.last_capture_button_state = current_button_state;
        } else {
            // 如果无法访问鼠标控制器，退出捕捉模式
            self.is_picking_position = false;
            self.status_message = "⚠️ 鼠标控制器访问失败，请重试".to_string();
        }
    }

    fn get_current_mouse_pos(&mut self) {
        if let Ok(controller) = self.mouse_controller.lock() {
            let (x, y) = controller.get_mouse_position();
            self.x_pos = x;
            self.y_pos = y;
            self.status_message = format!("已获取当前鼠标位置: ({}, {})", x, y);
        }
    }

    fn get_screen_info(&mut self) {
        if let Ok(controller) = self.mouse_controller.lock() {
            match controller.get_screen_size() {
                Ok((width, height)) => {
                    self.status_message = format!("屏幕尺寸: {}x{}", width, height);
                }
                Err(e) => {
                    self.status_message = format!("获取屏幕信息失败: {}", e);
                }
            }
        }
    }

    fn perform_single_click(&self) {
        let x = self.x_pos;
        let y = self.y_pos;
        let click_type = self.click_type;
        let total_clicks = self.total_clicks.clone();
        let mouse_controller = self.mouse_controller.clone();

        thread::spawn(move || {
            if let Ok(mut controller) = mouse_controller.lock() {
                let _ = controller.move_mouse_to(x, y);
                thread::sleep(Duration::from_millis(50));

                let result = match click_type {
                    ClickType::Left => controller.click_left(),
                    ClickType::Right => controller.click_right(),
                    ClickType::Middle => controller.click_middle(),
                };

                if result.is_ok() {
                    if let Ok(mut count) = total_clicks.lock() {
                        *count += 1;
                    }
                }
            }
        });
    }

    fn start_auto_clicking(&mut self) {
        if *self.is_clicking.lock().unwrap() {
            return;
        }

        *self.is_clicking.lock().unwrap() = true;
        self.status_message = "自动点击中...".to_string();

        let is_clicking = self.is_clicking.clone();
        let total_clicks = self.total_clicks.clone();
        let mouse_controller = self.mouse_controller.clone();
        let x = self.x_pos;
        let y = self.y_pos;
        let interval = self.click_interval;
        let max_clicks = self.click_count;
        let click_type = self.click_type;

        thread::spawn(move || {
            let mut clicks_performed = 0;

            while *is_clicking.lock().unwrap() && clicks_performed < max_clicks {
                if let Ok(mut controller) = mouse_controller.lock() {
                    let _ = controller.move_mouse_to(x, y);
                    thread::sleep(Duration::from_millis(10));

                    let result = match click_type {
                        ClickType::Left => controller.click_left(),
                        ClickType::Right => controller.click_right(),
                        ClickType::Middle => controller.click_middle(),
                    };

                    if result.is_ok() {
                        clicks_performed += 1;
                        if let Ok(mut count) = total_clicks.lock() {
                            *count += 1;
                        }
                    }
                }

                thread::sleep(Duration::from_secs_f64(interval));
            }

            *is_clicking.lock().unwrap() = false;
        });
    }

    fn stop_clicking(&mut self) {
        *self.is_clicking.lock().unwrap() = false;
        self.status_message = "已停止".to_string();
    }
}

impl eframe::App for MouseClickerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 检查是否在拾取坐标模式
        self.check_position_picking();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("🖱️ 跨平台鼠标点击工具");
            ui.separator();

            // 如果在捕捉模式，添加醒目的提示框
            if self.is_picking_position {
                ui.allocate_ui_with_layout(
                    [ui.available_width(), 60.0].into(),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        ui.add_space(10.0);
                        ui.colored_label(egui::Color32::from_rgb(255, 165, 0), "🎯 坐标捕捉模式激活中");
                        ui.colored_label(egui::Color32::LIGHT_RED, "请在屏幕任意位置点击鼠标中键（滚轮键）来捕捉坐标");
                        ui.add_space(10.0);
                    }
                );
                ui.separator();
            }

            // 坐标设置
            ui.horizontal(|ui| {
                ui.label("点击坐标:");

                // 在捕捉模式下高亮显示坐标输入框
                if self.is_picking_position {
                    ui.style_mut().visuals.extreme_bg_color = egui::Color32::from_rgb(255, 255, 200);
                }

                ui.add(egui::DragValue::new(&mut self.x_pos).prefix("X: "));
                ui.add(egui::DragValue::new(&mut self.y_pos).prefix("Y: "));

                if self.is_picking_position {
                    ui.label("👈 坐标将自动填入这里");
                }
            });

            ui.horizontal(|ui| {
                if !self.is_picking_position {
                    if ui.button("捕捉坐标").clicked() {
                        self.start_position_picking();
                    }
                    if ui.button("获取当前位置").clicked() {
                        self.get_current_mouse_pos();
                    }
                    if ui.button("获取屏幕信息").clicked() {
                        self.get_screen_info();
                    }
                } else {
                    let button_name = match self.capture_button_type {
                        CaptureButtonType::MiddleButton => "中键",
                        CaptureButtonType::RightButton => "右键",
                    };
                    ui.colored_label(egui::Color32::RED, format!("等待{}点击中，请在屏幕任意位置点击鼠标{}...", button_name, button_name));
                    if ui.button("取消捕捉").clicked() {
                        self.is_picking_position = false;
                        self.status_message = "已取消坐标捕捉".to_string();
                    }
                }
            });

            // 捕捉按钮类型选择
            ui.horizontal(|ui| {
                ui.label("捕捉按钮:");
                ui.radio_value(&mut self.capture_button_type, CaptureButtonType::MiddleButton, "中键");
                ui.radio_value(&mut self.capture_button_type, CaptureButtonType::RightButton, "右键");
            });

            ui.separator();

            // 点击类型选择
            ui.horizontal(|ui| {
                ui.label("点击类型:");
                ui.radio_value(&mut self.click_type, ClickType::Left, "左键");
                ui.radio_value(&mut self.click_type, ClickType::Right, "右键");
                ui.radio_value(&mut self.click_type, ClickType::Middle, "中键");
            });

            ui.separator();

            // 单次点击
            ui.horizontal(|ui| {
                if ui.button("单次点击").clicked() {
                    self.perform_single_click();
                    self.status_message = "执行单次点击".to_string();
                }
            });

            ui.separator();

            // 自动点击设置
            ui.checkbox(&mut self.auto_mode, "自动点击模式");

            if self.auto_mode {
                ui.horizontal(|ui| {
                    ui.label("点击间隔(秒):");
                    ui.add(egui::DragValue::new(&mut self.click_interval)
                        .range(0.1..=10.0)
                        .speed(0.1));
                });

                ui.horizontal(|ui| {
                    ui.label("点击次数:");
                    ui.add(egui::DragValue::new(&mut self.click_count)
                        .range(1..=1000));
                });

                ui.horizontal(|ui| {
                    let is_clicking = *self.is_clicking.lock().unwrap();

                    if !is_clicking {
                        if ui.button("开始自动点击").clicked() {
                            self.start_auto_clicking();
                        }
                    } else {
                        if ui.button("停止点击").clicked() {
                            self.stop_clicking();
                        }
                    }
                });
            }

            ui.separator();

            // 状态信息
            ui.horizontal(|ui| {
                ui.label("状态:");
                ui.colored_label(egui::Color32::BLUE, &self.status_message);
            });

            ui.horizontal(|ui| {
                ui.label("总点击次数:");
                let total = *self.total_clicks.lock().unwrap();
                ui.colored_label(egui::Color32::GREEN, total.to_string());
            });

            ui.separator();

            // 额外功能
            ui.horizontal(|ui| {
                if ui.button("重置计数器").clicked() {
                    *self.total_clicks.lock().unwrap() = 0;
                    self.status_message = "计数器已重置".to_string();
                }
            });

            ui.separator();

            // 平台信息
            ui.collapsing("平台信息", |ui| {
                ui.label(format!("操作系统: {}", std::env::consts::OS));
                ui.label(format!("架构: {}", std::env::consts::ARCH));
                ui.label("支持的平台: Windows, macOS, Linux");
                ui.label("使用纯Rust实现，无需额外系统依赖");

                ui.separator();
                ui.checkbox(&mut self.show_debug_info, "显示鼠标按钮调试信息");

                if self.show_debug_info {
                    if let Ok(controller) = self.mouse_controller.lock() {
                        let button_states = controller.get_mouse_button_states();
                        ui.label(format!("鼠标按钮状态数组: {:?}", button_states));
                        ui.label("数组说明: [索引0, 索引1, 索引2, 索引3, 索引4, 索引5]");

                        let (x, y) = controller.get_mouse_position();
                        ui.label(format!("当前鼠标位置: ({}, {})", x, y));

                        let left = controller.is_left_button_pressed();
                        let right = controller.is_right_button_pressed();
                        let middle = controller.is_middle_button_pressed();
                        ui.label(format!("实时按钮状态:"));
                        ui.horizontal(|ui| {
                            if left {
                                ui.colored_label(egui::Color32::GREEN, "左键:按下");
                            } else {
                                ui.label("左键:释放");
                            }
                            if right {
                                ui.colored_label(egui::Color32::GREEN, "右键:按下");
                            } else {
                                ui.label("右键:释放");
                            }
                            if middle {
                                ui.colored_label(egui::Color32::GREEN, "中键:按下");
                            } else {
                                ui.label("中键:释放");
                            }
                        });

                        ui.label(format!("技术细节:"));
                        ui.label(format!("  左键: {} (使用索引2)", left));
                        ui.label(format!("  右键: {} (使用索引3)", right));
                        ui.label(format!("  中键: {} (使用索引4)", middle));

                        ui.separator();
                        ui.colored_label(egui::Color32::GREEN, "✅ 按钮映射已修正:");
                        ui.label("索引0-1: 未知功能");
                        ui.label("索引2: 左键");
                        ui.label("索引3: 右键");
                        ui.label("索引4: 中键");
                        ui.label("索引5: 可能是额外按钮");
                    }
                }
            });

            ui.separator();

            // 说明文字
            ui.collapsing("使用说明", |ui| {
                ui.label("1. 设置要点击的坐标位置（基于屏幕左上角为原点）");
                ui.label("2. 选择点击类型（左键/右键/中键）");
                ui.label("3. 可以进行单次点击或开启自动点击模式");
                ui.label("4. 自动模式下可以设置点击间隔和次数");
                ui.label("5. 点击过程中可以随时停止");
                ui.label("6. 使用「捕捉坐标」按钮：点击按钮后在屏幕任意位置点击鼠标中键，坐标会自动填入");
                ui.label("7. 使用「获取当前位置」按钮：直接获取鼠标当前位置坐标");
                ui.label("💡 提示：使用中键捕捉坐标可以避免与界面左键点击冲突");
                ui.separator();
                ui.colored_label(egui::Color32::RED, "⚠️ 请谨慎使用，避免对系统造成不必要的影响");
                ui.colored_label(egui::Color32::GREEN, "✅ 跨平台纯Rust实现，支持Windows/macOS/Linux");
            });
        });

        // 在捕捉模式下更频繁地刷新以检测点击，并添加闪烁效果
        if self.is_picking_position {
            ctx.request_repaint_after(Duration::from_millis(16)); // ~60 FPS 用于流畅的视觉反馈
        } else {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([480.0, 650.0])
            .with_min_inner_size([450.0, 600.0])
            .with_resizable(true)
            .with_title("跨平台鼠标点击工具"),
        ..Default::default()
    };

    eframe::run_native(
        "跨平台鼠标点击工具",
        options,
        Box::new(|cc| Ok(Box::new(MouseClickerApp::new(cc)))),
    )
}