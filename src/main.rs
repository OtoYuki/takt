use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use relm4::prelude::*;

#[derive(serde::Deserialize)]
struct HyprWindow {
    title: Option<String>,
    class: Option<String>,
}

#[derive(serde::Deserialize)]
struct HyprWorkspace {
    id: i32,
}

#[derive(serde::Deserialize)]
struct HyprMonActiveWs {
    id: i32,
}

#[derive(serde::Deserialize)]
struct HyprMonitor {
    name: String,
    focused: bool,
    #[serde(rename = "activeWorkspace")]
    active_workspace: HyprMonActiveWs,
}

struct AppModel {
    _target_monitor: String,
    ws_base: u32,

    time_str: String,
    date_str: String,
    sec_str: String,
    sec_tick: bool,

    active_workspace: u32,
    occupied_workspaces: Vec<u32>,
    window_title: String,

    cpu_usage: f32,
    mem_usage: f32,
    vol_level: u32,
    vol_muted: bool,
    bat_capacity: u32,
    bat_status: String,
    net_quality: Option<u32>,
    net_type: String,

    media_status: Option<String>,
    media_title: Option<String>,
}

#[derive(Debug)]
enum AppMsg {
    Tick,
    UpdateTelemetry {
        cpu: f32,
        mem: f32,
        vol: u32,
        vol_muted: bool,
        bat_capacity: u32,
        bat_status: String,
        net_quality: Option<u32>,
        net_type: String,
        media_status: Option<String>,
        media_title: Option<String>,
    },
    UpdateHyprland {
        active_ws: u32,
        occupied_ws: Vec<u32>,
        title: String,
    },
    SwitchWorkspaceSlot(u32),
    ToggleMute,
    ToggleMedia,
    TriggerPower,
}

fn meter_dots_4(pct: f32, active_color: &str, inactive_color: &str) -> String {
    let level = if pct <= 0.0 {
        0
    } else if pct < 25.0 {
        1
    } else if pct < 50.0 {
        2
    } else if pct < 75.0 {
        3
    } else {
        4
    };
    let mut out = String::new();
    for i in 0..4 {
        if i < level {
            out.push_str(&format!("<span foreground='{}'>▪</span>", active_color));
        } else {
            out.push_str(&format!("<span foreground='{}'>▫</span>", inactive_color));
        }
    }
    out
}

fn cpu_markup(cpu: f32) -> String {
    let color = if cpu >= 85.0 {
        "#BE5D3C"
    } else if cpu >= 50.0 {
        "#D8A664"
    } else {
        "#999208"
    };
    let dots = meter_dots_4(cpu, color, "#353e26");
    format!(
        "<span font_family='Goga' font_size='9pt' font_weight='bold' foreground='#5A6042'>CPU</span> <span font_family='Geist Mono' font_weight='bold' foreground='{}'>{:>2.0}%</span> <span font_family='Geist Mono' font_size='9.5pt'>{}</span>",
        color, cpu, dots
    )
}

fn mem_markup(mem: f32) -> String {
    let color = if mem >= 85.0 {
        "#BE5D3C"
    } else if mem >= 60.0 {
        "#D8A664"
    } else {
        "#D1CF8B"
    };
    let dots = meter_dots_4(mem, color, "#353e26");
    format!(
        "<span font_family='Goga' font_size='9pt' font_weight='bold' foreground='#5A6042'>RAM</span> <span font_family='Geist Mono' font_weight='bold' foreground='{}'>{:>2.0}%</span> <span font_family='Geist Mono' font_size='9.5pt'>{}</span>",
        color, mem, dots
    )
}

fn vol_markup(vol: u32, muted: bool) -> String {
    if muted {
        "<span font_family='Goga' font_size='9pt' font_weight='bold' foreground='#5A6042'>VOL</span> <span font_family='Geist Mono' font_weight='bold' foreground='#BE5D3C'>MUTE</span> <span font_family='Geist Mono' font_size='9.5pt' foreground='#353e26'>▫▫▫▫</span>".to_string()
    } else {
        let dots = meter_dots_4(vol as f32, "#D8A664", "#353e26");
        format!(
            "<span font_family='Goga' font_size='9pt' font_weight='bold' foreground='#5A6042'>VOL</span> <span font_family='Geist Mono' font_weight='bold' foreground='#D8A664'>{:>2}%</span> <span font_family='Geist Mono' font_size='9.5pt'>{}</span>",
            vol, dots
        )
    }
}

fn bat_markup(cap: u32, status: &str) -> String {
    let is_charging = status == "Charging" || status == "Full";
    let color = if cap <= 20 && !is_charging {
        "#BE5D3C"
    } else if is_charging {
        "#999208"
    } else {
        "#FBFFE1"
    };
    let dots = meter_dots_4(cap as f32, color, "#353e26");
    let tag = if status == "Full" {
        "FULL".to_string()
    } else if status == "Charging" {
        format!("+{}%", cap)
    } else {
        format!("{}%", cap)
    };
    format!(
        "<span font_family='Goga' font_size='9pt' font_weight='bold' foreground='#5A6042'>BAT</span> <span font_family='Geist Mono' font_weight='bold' foreground='{}'>{}</span> <span font_family='Geist Mono' font_size='9.5pt'>{}</span>",
        color, tag, dots
    )
}

fn net_markup(quality: Option<u32>, net_type: &str) -> String {
    match quality {
        Some(q) => {
            let dots = meter_dots_4(q as f32, "#999208", "#353e26");
            format!(
                "<span font_family='Goga' font_size='9pt' font_weight='bold' foreground='#5A6042'>NET</span> <span font_family='Geist Mono' font_weight='bold' foreground='#999208'>{:>2}%</span> <span font_family='Geist Mono' font_size='9.5pt'>{}</span>",
                q, dots
            )
        }
        None if net_type == "ETH" => {
            "<span font_family='Goga' font_size='9pt' font_weight='bold' foreground='#5A6042'>NET</span> <span font_family='Geist Mono' font_weight='bold' foreground='#999208'>ETH</span> <span font_family='Geist Mono' font_size='9.5pt' foreground='#999208'>▪▪▪▪</span>".to_string()
        }
        _ => {
            "<span font_family='Goga' font_size='9pt' font_weight='bold' foreground='#5A6042'>NET</span> <span font_family='Geist Mono' font_weight='bold' foreground='#BE5D3C'>OFF</span> <span font_family='Geist Mono' font_size='9.5pt' foreground='#353e26'>▫▫▫▫</span>".to_string()
        }
    }
}

fn clock_markup(time: &str, date: &str, sec: &str, tick: bool) -> String {
    let delim = if tick {
        "<span font_family='Geist Mono' font_weight='bold' font_size='11pt' foreground='#999208'>::</span>"
    } else {
        "<span font_family='Geist Mono' font_weight='bold' font_size='11pt' foreground='#5A6042'>::</span>"
    };
    format!(
        "<span font_family='Freigeist' font_weight='800' font_size='12pt' foreground='#FBFFE1'>{}</span> {} <span font_family='Goga' font_weight='bold' font_size='10pt' foreground='#D1CF8B'>{}</span> <span font_family='Geist Mono' font_size='8.5pt' font_weight='bold' foreground='#5A6042'>:{}</span>",
        time, delim, date, sec
    )
}

fn window_title_markup(title: &str) -> String {
    let escaped = gtk4::glib::markup_escape_text(title);
    let char_count = escaped.chars().count();
    let display = if char_count > 34 {
        let truncated: String = escaped.chars().take(34).collect();
        format!("{}…", truncated)
    } else {
        escaped.to_string()
    };
    format!(
        "<span font_family='Goga' font_size='9pt' font_weight='bold' foreground='#999208'>◈ ACT //</span> <span font_family='Geist Mono' font_weight='bold' font_size='10pt' foreground='#FBFFE1'>{}</span>",
        display
    )
}

fn media_markup(status: Option<&str>, title: Option<&str>) -> String {
    if let (Some(s), Some(t)) = (status, title) {
        let (icon, col) = if s == "Playing" {
            ("⏵ PLAY", "#D8A664")
        } else {
            ("⏸ PAUS", "#5A6042")
        };
        let escaped = gtk4::glib::markup_escape_text(t);
        let char_count = escaped.chars().count();
        let display = if char_count > 24 {
            let truncated: String = escaped.chars().take(24).collect();
            format!("{}…", truncated)
        } else {
            escaped.to_string()
        };
        format!(
            "<span font_family='Goga' font_size='9pt' font_weight='bold' foreground='{}'>{} //</span> <span font_family='Geist Mono' font_weight='bold' font_size='10pt' foreground='#FBFFE1'>{}</span>",
            col, icon, display
        )
    } else {
        String::new()
    }
}

fn get_ws_classes(active_ws: u32, ws_id: u32, occupied: &[u32]) -> &'static [&'static str] {
    if active_ws == ws_id {
        &["ws-btn", "active"]
    } else if occupied.contains(&ws_id) {
        &["ws-btn", "occupied"]
    } else {
        &["ws-btn", "empty"]
    }
}

#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = String;
    type Input = AppMsg;
    type Output = ();

    view! {
        #[root]
        gtk4::ApplicationWindow {
            set_title: Some("takt"),
            set_default_size: (1600, 44),
            add_css_class: "takt-bar",

            gtk4::CenterBox {
                set_orientation: gtk4::Orientation::Horizontal,
                set_hexpand: true,
                add_css_class: "takt-rail",

                #[wrap(Some)]
                set_start_widget = &gtk4::Box {
                    set_orientation: gtk4::Orientation::Horizontal,
                    set_spacing: 8,
                    set_valign: gtk4::Align::Center,

                    // Workspaces Pod
                    gtk4::Box {
                        set_orientation: gtk4::Orientation::Horizontal,
                        set_valign: gtk4::Align::Center,
                        add_css_class: "tactile-pod",
                        add_css_class: "pod-workspaces",

                        gtk4::Button {
                            set_label: if model.ws_base == 6 { "06" } else { "01" },
                            #[watch]
                            set_label: &format!("{:02}", model.ws_base),
                            #[watch]
                            set_css_classes: get_ws_classes(model.active_workspace, model.ws_base, &model.occupied_workspaces),
                            connect_clicked => AppMsg::SwitchWorkspaceSlot(0),
                        },
                        gtk4::Button {
                            set_label: if model.ws_base == 6 { "07" } else { "02" },
                            #[watch]
                            set_label: &format!("{:02}", model.ws_base + 1),
                            #[watch]
                            set_css_classes: get_ws_classes(model.active_workspace, model.ws_base + 1, &model.occupied_workspaces),
                            connect_clicked => AppMsg::SwitchWorkspaceSlot(1),
                        },
                        gtk4::Button {
                            set_label: if model.ws_base == 6 { "08" } else { "03" },
                            #[watch]
                            set_label: &format!("{:02}", model.ws_base + 2),
                            #[watch]
                            set_css_classes: get_ws_classes(model.active_workspace, model.ws_base + 2, &model.occupied_workspaces),
                            connect_clicked => AppMsg::SwitchWorkspaceSlot(2),
                        },
                        gtk4::Button {
                            set_label: if model.ws_base == 6 { "09" } else { "04" },
                            #[watch]
                            set_label: &format!("{:02}", model.ws_base + 3),
                            #[watch]
                            set_css_classes: get_ws_classes(model.active_workspace, model.ws_base + 3, &model.occupied_workspaces),
                            connect_clicked => AppMsg::SwitchWorkspaceSlot(3),
                        },
                        gtk4::Button {
                            set_label: if model.ws_base == 6 { "10" } else { "05" },
                            #[watch]
                            set_label: &format!("{:02}", model.ws_base + 4),
                            #[watch]
                            set_css_classes: get_ws_classes(model.active_workspace, model.ws_base + 4, &model.occupied_workspaces),
                            connect_clicked => AppMsg::SwitchWorkspaceSlot(4),
                        },
                    },

                    // Active Window Pod
                    gtk4::Box {
                        set_orientation: gtk4::Orientation::Horizontal,
                        set_valign: gtk4::Align::Center,
                        add_css_class: "tactile-pod",
                        add_css_class: "pod-window",

                        gtk4::Label {
                            set_markup: &window_title_markup(&model.window_title),
                            #[watch]
                            set_markup: &window_title_markup(&model.window_title),
                        },
                    },
                },

                #[wrap(Some)]
                set_center_widget = &gtk4::Box {
                    set_orientation: gtk4::Orientation::Horizontal,
                    set_valign: gtk4::Align::Center,
                    add_css_class: "tactile-pod",
                    add_css_class: "pod-clock",

                    gtk4::Label {
                        set_markup: &clock_markup(&model.time_str, &model.date_str, &model.sec_str, model.sec_tick),
                        #[watch]
                        set_markup: &clock_markup(&model.time_str, &model.date_str, &model.sec_str, model.sec_tick),
                    },
                },

                #[wrap(Some)]
                set_end_widget = &gtk4::Box {
                    set_orientation: gtk4::Orientation::Horizontal,
                    set_spacing: 8,
                    set_valign: gtk4::Align::Center,

                    // Media Pod (visible when media exists)
                    gtk4::Box {
                        set_orientation: gtk4::Orientation::Horizontal,
                        set_valign: gtk4::Align::Center,
                        add_css_class: "tactile-pod",
                        add_css_class: "pod-media",
                        set_visible: model.media_status.is_some(),
                        #[watch]
                        set_visible: model.media_status.is_some(),

                        gtk4::Button {
                            add_css_class: "media-btn",
                            connect_clicked => AppMsg::ToggleMedia,
                            gtk4::Label {
                                set_markup: &media_markup(model.media_status.as_deref(), model.media_title.as_deref()),
                                #[watch]
                                set_markup: &media_markup(model.media_status.as_deref(), model.media_title.as_deref()),
                            }
                        }
                    },

                    // Unified Hardware Instrument Deck
                    gtk4::Box {
                        set_orientation: gtk4::Orientation::Horizontal,
                        set_valign: gtk4::Align::Center,
                        add_css_class: "tactile-pod",
                        add_css_class: "pod-hardware",

                        // CPU Cell
                        gtk4::Label {
                            add_css_class: "instrument-cell",
                            set_markup: &cpu_markup(model.cpu_usage),
                            #[watch]
                            set_markup: &cpu_markup(model.cpu_usage),
                        },
                        // RAM Cell
                        gtk4::Label {
                            add_css_class: "instrument-cell",
                            set_markup: &mem_markup(model.mem_usage),
                            #[watch]
                            set_markup: &mem_markup(model.mem_usage),
                        },
                        // Volume Cell
                        gtk4::Button {
                            add_css_class: "instrument-btn",
                            connect_clicked => AppMsg::ToggleMute,
                            gtk4::Label {
                                set_markup: &vol_markup(model.vol_level, model.vol_muted),
                                #[watch]
                                set_markup: &vol_markup(model.vol_level, model.vol_muted),
                            }
                        },
                        // Battery Cell
                        gtk4::Label {
                            add_css_class: "instrument-cell",
                            set_markup: &bat_markup(model.bat_capacity, &model.bat_status),
                            #[watch]
                            set_markup: &bat_markup(model.bat_capacity, &model.bat_status),
                        },
                        // Network Cell
                        gtk4::Label {
                            add_css_class: "instrument-cell",
                            set_markup: &net_markup(model.net_quality, &model.net_type),
                            #[watch]
                            set_markup: &net_markup(model.net_quality, &model.net_type),
                        },
                        // Tactical Power Cell
                        gtk4::Button {
                            add_css_class: "instrument-btn",
                            add_css_class: "power-cell",
                            connect_clicked => AppMsg::TriggerPower,
                            gtk4::Label {
                                set_markup: "<span font_family='Geist Mono' font_weight='bold' font_size='11pt' foreground='#BE5D3C'>⏻</span>",
                            }
                        },
                    },
                },
            },
        }
    }

    fn init(
        target_monitor: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let ws_base = if target_monitor.starts_with("eDP") {
            6
        } else {
            1
        };

        // Initialize Wayland Layer Shell
        root.init_layer_shell();
        root.set_layer(Layer::Top);
        root.set_anchor(Edge::Top, true);
        root.set_anchor(Edge::Left, true);
        root.set_anchor(Edge::Right, true);
        root.auto_exclusive_zone_enable();

        // Bind explicitly to the target monitor
        if let Some(display) = gtk4::gdk::Display::default() {
            let monitors = display.monitors();
            for i in 0..monitors.n_items() {
                if let Some(obj) = monitors.item(i) {
                    if let Ok(m) = obj.downcast::<gtk4::gdk::Monitor>() {
                        if m.connector().as_deref() == Some(&target_monitor) {
                            root.set_monitor(&m);
                            break;
                        }
                    }
                }
            }
        }

        // Initial system telemetry
        let mut sys = sysinfo::System::new_all();
        sys.refresh_memory();
        sys.refresh_cpu_all();
        let mem = (sys.used_memory() as f32 / sys.total_memory() as f32) * 100.0;

        let (vol_level, vol_muted) = read_volume();
        let (bat_capacity, bat_status) = read_battery();
        let (net_quality, net_type) = read_network();
        let (media_status, media_title) = read_media()
            .map(|(s, t)| (Some(s), Some(t)))
            .unwrap_or((None, None));
        let (active_ws, occupied_ws, window_title) = read_hyprland_state(&target_monitor);

        let now = chrono::Local::now();
        let model = AppModel {
            _target_monitor: target_monitor.clone(),
            ws_base,

            time_str: now.format("%H:%M").to_string(),
            date_str: now.format("%a %d %b").to_string().to_uppercase(),
            sec_str: now.format("%S").to_string(),
            sec_tick: true,

            active_workspace: active_ws,
            occupied_workspaces: occupied_ws,
            window_title,

            cpu_usage: sys.global_cpu_usage(),
            mem_usage: mem,
            vol_level,
            vol_muted,
            bat_capacity,
            bat_status,
            net_quality,
            net_type,

            media_status,
            media_title,
        };

        let widgets = view_output!();

        // Background Telemetry Worker (every 1 sec)
        let sender_telemetry = sender.clone();
        std::thread::spawn(move || {
            let mut sys = sysinfo::System::new_all();
            loop {
                std::thread::sleep(std::time::Duration::from_millis(1000));
                sys.refresh_cpu_all();
                sys.refresh_memory();

                let cpu = sys.global_cpu_usage();
                let mem = (sys.used_memory() as f32 / sys.total_memory() as f32) * 100.0;
                let (vol, vol_muted) = read_volume();
                let (bat_capacity, bat_status) = read_battery();
                let (net_quality, net_type) = read_network();
                let (media_status, media_title) = read_media()
                    .map(|(s, t)| (Some(s), Some(t)))
                    .unwrap_or((None, None));

                sender_telemetry.input(AppMsg::UpdateTelemetry {
                    cpu,
                    mem,
                    vol,
                    vol_muted,
                    bat_capacity,
                    bat_status,
                    net_quality,
                    net_type,
                    media_status,
                    media_title,
                });
                sender_telemetry.input(AppMsg::Tick);
            }
        });

        // Background Hyprland Worker (every 300ms)
        let sender_hypr = sender.clone();
        let mon_for_hypr = target_monitor;
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(300));
            let (active_ws, occupied_ws, title) = read_hyprland_state(&mon_for_hypr);
            sender_hypr.input(AppMsg::UpdateHyprland {
                active_ws,
                occupied_ws,
                title,
            });
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            AppMsg::Tick => {
                let now = chrono::Local::now();
                self.time_str = now.format("%H:%M").to_string();
                self.date_str = now.format("%a %d %b").to_string().to_uppercase();
                self.sec_str = now.format("%S").to_string();
                self.sec_tick = !self.sec_tick;
            }
            AppMsg::UpdateTelemetry {
                cpu,
                mem,
                vol,
                vol_muted,
                bat_capacity,
                bat_status,
                net_quality,
                net_type,
                media_status,
                media_title,
            } => {
                self.cpu_usage = cpu;
                self.mem_usage = mem;
                self.vol_level = vol;
                self.vol_muted = vol_muted;
                self.bat_capacity = bat_capacity;
                self.bat_status = bat_status;
                self.net_quality = net_quality;
                self.net_type = net_type;
                self.media_status = media_status;
                self.media_title = media_title;
            }
            AppMsg::UpdateHyprland {
                active_ws,
                occupied_ws,
                title,
            } => {
                self.active_workspace = active_ws;
                self.occupied_workspaces = occupied_ws;
                self.window_title = title;
            }
            AppMsg::SwitchWorkspaceSlot(slot) => {
                let ws_id = self.ws_base + slot;
                let _ = std::process::Command::new("hyprctl")
                    .args(["dispatch", "workspace", &ws_id.to_string()])
                    .spawn();
            }
            AppMsg::ToggleMute => {
                let _ = std::process::Command::new("wpctl")
                    .args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"])
                    .spawn();
                let (vol, muted) = read_volume();
                self.vol_level = vol;
                self.vol_muted = muted;
            }
            AppMsg::ToggleMedia => {
                let _ = std::process::Command::new("playerctl")
                    .arg("play-pause")
                    .spawn();
            }
            AppMsg::TriggerPower => {
                let _ = std::process::Command::new("wlogout").spawn();
            }
        }
    }
}

fn read_volume() -> (u32, bool) {
    if let Ok(output) = std::process::Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
        .output()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        let muted = text.contains("[MUTED]");
        if let Some(vol_str) = text.split_whitespace().nth(1) {
            if let Ok(vol_f) = vol_str.parse::<f32>() {
                return ((vol_f * 100.0).round() as u32, muted);
            }
        }
    }
    (100, false)
}

fn read_battery() -> (u32, String) {
    let cap = std::fs::read_to_string("/sys/class/power_supply/BAT0/capacity")
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(100);
    let status = std::fs::read_to_string("/sys/class/power_supply/BAT0/status")
        .unwrap_or_else(|_| "Full".into())
        .trim()
        .to_string();
    (cap, status)
}

fn read_network() -> (Option<u32>, String) {
    if let Ok(content) = std::fs::read_to_string("/proc/net/wireless") {
        for line in content.lines() {
            if line.contains("wlan0:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    let qual_str = parts[2].trim_end_matches('.');
                    if let Ok(qual) = qual_str.parse::<f32>() {
                        let pct = ((qual / 70.0) * 100.0).min(100.0) as u32;
                        return (Some(pct), "WIFI".to_string());
                    }
                }
            }
        }
    }
    if let Ok(state) = std::fs::read_to_string("/sys/class/net/eno1/operstate") {
        if state.trim() == "up" {
            return (Some(100), "ETH".to_string());
        }
    }
    if let Ok(state) = std::fs::read_to_string("/sys/class/net/wlan0/operstate") {
        if state.trim() == "up" {
            return (Some(100), "WIFI".to_string());
        }
    }
    (None, "OFF".to_string())
}

fn read_media() -> Option<(String, String)> {
    let status_out = std::process::Command::new("playerctl")
        .arg("status")
        .output()
        .ok()?;
    if !status_out.status.success() {
        return None;
    }
    let status = String::from_utf8_lossy(&status_out.stdout)
        .trim()
        .to_string();
    if status != "Playing" && status != "Paused" {
        return None;
    }
    let title_out = std::process::Command::new("playerctl")
        .args(["metadata", "--format", "{{artist}} - {{title}}"])
        .output()
        .ok()?;
    let mut title = String::from_utf8_lossy(&title_out.stdout)
        .trim()
        .to_string();
    if title.is_empty() || title == " - " {
        if let Ok(meta_title) = std::process::Command::new("playerctl")
            .args(["metadata", "title"])
            .output()
        {
            title = String::from_utf8_lossy(&meta_title.stdout)
                .trim()
                .to_string();
        }
    }
    if title.is_empty() {
        return None;
    }
    Some((status, title))
}

fn read_hyprland_state(target_monitor: &str) -> (u32, Vec<u32>, String) {
    let mut active_ws = 1u32;
    let mut occupied = Vec::new();
    let mut title = "Desktop".to_string();

    if let Ok(output) = std::process::Command::new("hyprctl")
        .args(["monitors", "-j"])
        .output()
    {
        if let Ok(monitors) = serde_json::from_slice::<Vec<HyprMonitor>>(&output.stdout) {
            for m in monitors {
                if m.name == target_monitor || (m.focused && target_monitor.is_empty()) {
                    if m.active_workspace.id > 0 {
                        active_ws = m.active_workspace.id as u32;
                    }
                    break;
                }
            }
        }
    }

    if let Ok(output) = std::process::Command::new("hyprctl")
        .args(["workspaces", "-j"])
        .output()
    {
        if let Ok(workspaces) = serde_json::from_slice::<Vec<HyprWorkspace>>(&output.stdout) {
            for ws in workspaces {
                if ws.id > 0 {
                    occupied.push(ws.id as u32);
                }
            }
        }
    }

    if let Ok(output) = std::process::Command::new("hyprctl")
        .args(["activewindow", "-j"])
        .output()
    {
        if let Ok(win) = serde_json::from_slice::<HyprWindow>(&output.stdout) {
            if let Some(t) = win.title {
                if !t.trim().is_empty() {
                    title = t.trim().to_string();
                }
            } else if let Some(c) = win.class {
                if !c.trim().is_empty() {
                    title = c.trim().to_string();
                }
            }
        }
    }

    (active_ws, occupied, title)
}

fn detect_monitors() -> Vec<String> {
    if let Ok(output) = std::process::Command::new("hyprctl")
        .args(["monitors", "-j"])
        .output()
    {
        if let Ok(monitors) = serde_json::from_slice::<Vec<HyprMonitor>>(&output.stdout) {
            let names: Vec<String> = monitors.into_iter().map(|m| m.name).collect();
            if !names.is_empty() {
                return names;
            }
        }
    }
    vec!["HDMI-A-1".to_string()]
}

fn run_monitor(mon_name: String) {
    let clean_mon = mon_name.replace(['-', '.'], "_");
    let app_id = format!("dev.s1re.takt.{}", clean_mon);
    let app = RelmApp::new(&app_id);
    relm4::set_global_css(include_str!("style.css"));
    app.run::<AppModel>(mon_name);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut target_monitor = None;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--monitor" && i + 1 < args.len() {
            target_monitor = Some(args[i + 1].clone());
            i += 1;
        }
        i += 1;
    }

    if let Some(mon_name) = target_monitor {
        run_monitor(mon_name);
    } else {
        let monitors = detect_monitors();
        if monitors.len() <= 1 {
            let mon_name = monitors
                .into_iter()
                .next()
                .unwrap_or_else(|| "HDMI-A-1".to_string());
            run_monitor(mon_name);
        } else {
            let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("takt"));
            for mon in monitors {
                let _ = std::process::Command::new(&exe)
                    .args(["--monitor", &mon])
                    .spawn();
            }
        }
    }
}
