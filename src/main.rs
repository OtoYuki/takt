use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use relm4::prelude::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeckScreen {
    Sys = 1,
    Perf = 2,
    Cal = 3,
    Media = 4,
}

impl DeckScreen {
    pub fn next(&self) -> Self {
        match self {
            DeckScreen::Sys => DeckScreen::Perf,
            DeckScreen::Perf => DeckScreen::Cal,
            DeckScreen::Cal => DeckScreen::Media,
            DeckScreen::Media => DeckScreen::Sys,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            DeckScreen::Sys => DeckScreen::Media,
            DeckScreen::Perf => DeckScreen::Sys,
            DeckScreen::Cal => DeckScreen::Perf,
            DeckScreen::Media => DeckScreen::Cal,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            DeckScreen::Sys => "sys",
            DeckScreen::Perf => "perf",
            DeckScreen::Cal => "cal",
            DeckScreen::Media => "media",
        }
    }

    pub fn from_str_name(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "1" | "sys" | "system" => Some(DeckScreen::Sys),
            "2" | "perf" | "performance" => Some(DeckScreen::Perf),
            "3" | "cal" | "calendar" | "chrono" => Some(DeckScreen::Cal),
            "4" | "media" | "mpris" => Some(DeckScreen::Media),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtDeviceInfo {
    pub mac: String,
    pub name: String,
    pub connected: bool,
    pub paired: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDeviceInfo {
    pub name: String,
    pub description: String,
    pub glyph: &'static str,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiNetworkInfo {
    pub in_use: bool,
    pub ssid: String,
    pub signal: u32,
    pub security: String,
    pub is_known: bool,
}

struct AppModel {
    target_monitor: String,
    ws_base: u32,
    is_compact: bool,
    current_screen: DeckScreen,

    time_str: String,
    date_str: String,
    week_ribbon: String,
    week_stats: String,

    active_workspace: u32,
    occupied_workspaces: Vec<u32>,
    window_title: String,

    cpu_usage: f32,
    cpu_temp: Option<u32>,
    mem_usage: f32,
    mem_used_gb: f32,
    mem_total_gb: f32,

    gpu_usage: Option<u32>,
    gpu_vram_used_mb: Option<u32>,
    gpu_vram_total_mb: Option<u32>,
    gpu_temp: Option<u32>,

    disk_used_pct: u32,
    disk_free_gb: u32,

    vol_level: u32,
    vol_muted: bool,
    audio_sinks: Vec<AudioDeviceInfo>,
    audio_sources: Vec<AudioDeviceInfo>,
    audio_dirty: AtomicBool,
    bat_capacity: u32,
    bat_status: String,
    net_quality: Option<u32>,
    net_type: String,
    net_ssid: Option<String>,

    bluetooth_powered: bool,
    bluetooth_device: Option<String>,
    bt_devices: Vec<BtDeviceInfo>,
    bt_scanning: bool,
    bt_dirty: AtomicBool,

    wifi_radio_on: bool,
    wifi_networks: Vec<WifiNetworkInfo>,
    wifi_scanning: bool,
    wifi_selected_ssid: Option<String>,
    wifi_status_msg: Option<String>,
    wifi_dirty: AtomicBool,

    media_status: Option<String>,
    media_title: Option<String>,
    media_artist: Option<String>,
    media_player: Option<String>,
}

#[derive(Debug)]
pub struct TelemetryData {
    cpu: f32,
    cpu_temp: Option<u32>,
    mem: f32,
    mem_used_gb: f32,
    mem_total_gb: f32,
    gpu_usage: Option<u32>,
    gpu_vram_used_mb: Option<u32>,
    gpu_vram_total_mb: Option<u32>,
    gpu_temp: Option<u32>,
    disk_used_pct: u32,
    disk_free_gb: u32,
    vol: u32,
    vol_muted: bool,
    bat_capacity: u32,
    bat_status: String,
    net_quality: Option<u32>,
    net_type: String,
    net_ssid: Option<String>,
    bluetooth_powered: bool,
    bluetooth_device: Option<String>,
    media_status: Option<String>,
    media_title: Option<String>,
    media_artist: Option<String>,
    media_player: Option<String>,
}

#[derive(Debug)]
enum AppMsg {
    Tick,
    UpdateTelemetry(Box<TelemetryData>),
    UpdateHyprland {
        active_ws: u32,
        occupied_ws: Vec<u32>,
        title: String,
        ws_base: u32,
    },
    SwitchWorkspaceSlot(u32),
    SetDeckScreen(DeckScreen),
    SetDeckScreenRemote(DeckScreen),
    CycleDeckScreen(i32),
    ToggleMute,
    AdjustVolume(i32),
    RefreshAudioDevices,
    SetDefaultSink(String),
    SetDefaultSource(String),
    UpdateAudioDevices(Vec<AudioDeviceInfo>, Vec<AudioDeviceInfo>),

    // Embedded Bluetooth Deck
    ToggleBluetooth,
    ScanBluetooth,
    ConnectBluetooth(String),
    DisconnectBluetooth(String),
    UpdateBtDevices(Vec<BtDeviceInfo>),

    // Embedded Wi-Fi Deck
    ToggleWifiRadio,
    RescanWifi,
    ConnectWifi(String),
    SubmitWifiPassword(String),
    DisconnectWifi,
    SelectWifi(Option<String>),
    UpdateWifiNetworks(Vec<WifiNetworkInfo>),
    WifiActionStatus(Option<String>),

    // Media & System
    ToggleMedia,
    PrevMedia,
    NextMedia,
    ExecLock,
    ExecSuspend,
    ExecReboot,
    ExecPoweroff,
    ExecLogout,
    TriggerLauncher,
}

fn get_asset_path(rel: &str) -> Option<String> {
    let dev = std::path::Path::new("/home/s1re/dev/takt/assets").join(rel);
    if dev.exists() {
        return Some(dev.to_string_lossy().to_string());
    }
    let share = std::path::Path::new("/home/s1re/.local/share/takt/assets").join(rel);
    if share.exists() {
        return Some(share.to_string_lossy().to_string());
    }
    None
}

#[allow(dead_code)]
fn meter_bars_4(pct: f32, active_color: &str, inactive_color: &str) -> String {
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
            out.push_str(&format!("<span foreground='{}'>▮</span>", active_color));
        } else {
            out.push_str(&format!("<span foreground='{}'>▯</span>", inactive_color));
        }
    }
    out
}

fn cpu_perf_markup(cpu: f32, temp: Option<u32>) -> String {
    let color = if cpu >= 85.0 {
        "#BE5D3C"
    } else if cpu >= 50.0 {
        "#D8A664"
    } else {
        "#999208"
    };
    let bars = meter_bars_4(cpu, color, "#28341f");
    let temp_str = temp
        .map(|t| format!(" <span font_family='Geist Mono' font_weight='bold' foreground='#FBFFE1'>{}°C</span>", t))
        .unwrap_or_default();
    format!(
        "<span font_family='Goga' font_weight='bold' font_size='8.5pt' foreground='#999208'>CPU //</span> <span font_family='Geist Mono' font_weight='bold' font_size='9pt' foreground='{}'>{:>2.0}%</span> <span font_family='Geist Mono' font_size='6.5pt'>{}</span>{}",
        color, cpu, bars, temp_str
    )
}

fn mem_perf_markup(mem: f32, used_gb: f32, total_gb: f32) -> String {
    let color = if mem >= 85.0 {
        "#BE5D3C"
    } else if mem >= 60.0 {
        "#D8A664"
    } else {
        "#D1CF8B"
    };
    let bars = meter_bars_4(mem, color, "#28341f");
    format!(
        "<span font_family='Goga' font_weight='bold' font_size='8.5pt' foreground='#999208'>RAM //</span> <span font_family='Geist Mono' font_weight='bold' font_size='9pt' foreground='#FBFFE1'>{:.1}/{:.0}G</span> <span font_family='Geist Mono' font_size='6.5pt'>{}</span>",
        used_gb, total_gb, bars
    )
}

fn gpu_perf_markup(
    usage: Option<u32>,
    used_mb: Option<u32>,
    total_mb: Option<u32>,
    temp: Option<u32>,
) -> String {
    if let (Some(u), Some(used), Some(tot), Some(t)) = (usage, used_mb, total_mb, temp) {
        let color = if u >= 85 {
            "#BE5D3C"
        } else if u >= 50 {
            "#D8A664"
        } else {
            "#999208"
        };
        let bars = meter_bars_4(u as f32, color, "#28341f");
        format!(
            "<span font_family='Goga' font_weight='bold' font_size='8.5pt' foreground='#D8A664'>GPU //</span> <span font_family='Geist Mono' font_weight='bold' font_size='9pt' foreground='{}'>{:>2}%</span> <span font_family='Geist Mono' font_size='6.5pt'>{}</span> <span font_family='Geist Mono' font_size='8.5pt' foreground='#D1CF8B'>{:.1}/{:.0}G</span> <span font_family='Geist Mono' font_size='8.5pt' foreground='#FBFFE1'>{}°C</span>",
            color,
            u,
            bars,
            used as f32 / 1024.0,
            tot as f32 / 1024.0,
            t
        )
    } else {
        "<span font_family='Goga' font_weight='bold' font_size='8.5pt' foreground='#5A6042'>GPU // OFF</span>".to_string()
    }
}

fn disk_perf_markup(used_pct: u32, free_gb: u32) -> String {
    let color = if used_pct >= 90 {
        "#BE5D3C"
    } else if used_pct >= 75 {
        "#D8A664"
    } else {
        "#D1CF8B"
    };
    format!(
        "<span font_family='Goga' font_weight='bold' font_size='8.5pt' foreground='#999208'>NVME //</span> <span font_family='Geist Mono' font_weight='bold' font_size='9pt' foreground='{}'>{}%</span> <span font_family='Geist Mono' font_size='8.5pt' foreground='#5A6042'>({}G FREE)</span>",
        color, used_pct, free_gb
    )
}

fn media_studio_markup(
    _status: Option<&str>,
    artist: Option<&str>,
    title: Option<&str>,
    player: Option<&str>,
) -> String {
    let p_name = player.unwrap_or("MPRIS");
    let track_info = match (artist, title) {
        (Some(a), Some(t)) => format!("{} — {}", a, t),
        (None, Some(t)) => t.to_string(),
        _ => "No active playback".to_string(),
    };
    let escaped = gtk4::glib::markup_escape_text(&track_info);
    format!(
        "<span font_family='Goga' font_weight='bold' font_size='8.5pt' letter_spacing='1000' foreground='#999208'>{} //</span> <span font_family='Geist Mono' font_weight='bold' font_size='9.5pt' foreground='#FBFFE1'>{}</span>",
        p_name.to_uppercase(),
        escaped
    )
}

fn format_calendar() -> (String, String) {
    use chrono::Datelike;
    let now = chrono::Local::now();
    let weekday = now.weekday().num_days_from_monday(); // 0..=6
    let monday = now.date_naive() - chrono::Duration::days(weekday as i64);

    let mut ribbon = String::new();
    let day_names = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"];

    for (i, name) in day_names.iter().enumerate() {
        let d = monday + chrono::Duration::days(i as i64);
        let day_num = d.day();
        if i as u32 == weekday {
            ribbon.push_str(&format!(
                "<span background='#D8A664' foreground='#141C10' font_weight='bold'> {} {:02} </span>",
                name, day_num
            ));
        } else if (i as u32) < weekday {
            ribbon.push_str(&format!(
                "<span foreground='#5A6042'> {} {:02} </span>",
                name, day_num
            ));
        } else {
            ribbon.push_str(&format!(
                "<span foreground='#D1CF8B'> {} {:02} </span>",
                name, day_num
            ));
        }
        if i < 6 {
            ribbon.push_str("<span foreground='#28351f'>│</span>");
        }
    }

    let week_num = now.format("%V");
    let day_of_year = now.format("%j");
    let date_full = now.format("%d %b %Y").to_string().to_uppercase();
    let stats = format!(
        "<span font_family='Freigeist' font_weight='800' font_size='11pt' foreground='#FBFFE1'>{}</span> <span font_family='Geist Mono' font_size='8.5pt' foreground='#38462b'>│</span> <span font_family='Goga' font_weight='bold' font_size='8.5pt' letter_spacing='1200' foreground='#999208'>WEEK {} · DAY {}</span>",
        date_full, week_num, day_of_year
    );

    (ribbon, stats)
}

#[allow(dead_code)]
fn bat_markup(cap: u32, status: &str) -> String {
    let is_charging = status == "Charging" || status == "Full";
    let color = if cap <= 20 && !is_charging {
        "#BE5D3C"
    } else if is_charging {
        "#999208"
    } else {
        "#FBFFE1"
    };
    let bars = meter_bars_4(cap as f32, color, "#28341f");
    let tag = if status == "Full" {
        "100%".to_string()
    } else if status == "Charging" {
        format!("+{}%", cap)
    } else {
        format!("{}%", cap)
    };
    format!(
        "<span font_family='Geist Mono' font_weight='bold' font_size='9pt' foreground='{}'>{}</span> <span font_family='Geist Mono' font_size='6.5pt'>{}</span>",
        color, tag, bars
    )
}

fn network_markup(
    quality: Option<u32>,
    net_type: &str,
    ssid: Option<&str>,
    is_compact: bool,
) -> String {
    match net_type {
        "WIFI" => {
            if let Some(name) = ssid {
                let max_len = if is_compact { 7 } else { 11 };
                let display: String = name.chars().take(max_len).collect();
                format!(
                    "<span font_family='Geist Mono' font_weight='bold' font_size='9pt' foreground='#FBFFE1'>{}</span>",
                    display
                )
            } else if let Some(q) = quality {
                format!(
                    "<span font_family='Geist Mono' font_weight='bold' font_size='9pt' foreground='#999208'>{:>2}%</span>",
                    q
                )
            } else {
                "<span font_family='Geist Mono' font_weight='bold' font_size='9pt' foreground='#999208'>WIFI</span>".to_string()
            }
        }
        "ETH" => {
            "<span font_family='Geist Mono' font_weight='bold' font_size='9pt' foreground='#999208'>ETH</span>".to_string()
        }
        _ => {
            "<span font_family='Geist Mono' font_weight='bold' font_size='9pt' foreground='#BE5D3C'>OFF</span>".to_string()
        }
    }
}

fn bluetooth_markup(powered: bool, device: Option<&str>, is_compact: bool) -> String {
    if !powered {
        "<span font_family='Geist Mono' font_weight='bold' font_size='9pt' foreground='#3e4832'>BT OFF</span>".to_string()
    } else if let Some(dev) = device {
        let max_len = if is_compact { 7 } else { 11 };
        let name: String = dev.chars().take(max_len).collect();
        format!(
            "<span font_family='Geist Mono' font_weight='bold' font_size='9pt' foreground='#FBFFE1'>{}</span>",
            name
        )
    } else {
        "<span font_family='Geist Mono' font_weight='bold' font_size='9pt' foreground='#999208'>BT ON</span>".to_string()
    }
}

fn volume_markup(vol: u32, muted: bool) -> String {
    if muted {
        "<span font_family='Geist Mono' font_weight='bold' font_size='9pt' foreground='#BE5D3C'>MUTE</span>".to_string()
    } else {
        format!(
            "<span font_family='Geist Mono' font_weight='bold' font_size='9pt' foreground='#D8A664'>{:>2}%</span>",
            vol
        )
    }
}

fn power_markup() -> &'static str {
    "<span font_family='Geist Mono' font_weight='bold' font_size='11pt' foreground='#BE5D3C'>⏻</span>"
}

fn clock_markup(time: &str, date: &str) -> String {
    format!(
        "<span font_family='Freigeist' font_weight='800' font_size='11.5pt' foreground='#FBFFE1'>{}</span> <span font_family='Geist Mono' font_size='8.5pt' foreground='#38462b'>│</span> <span font_family='Goga' font_weight='bold' font_size='8.5pt' letter_spacing='1500' foreground='#D1CF8B'>{}</span>",
        time, date
    )
}

fn format_ws_label(ws_id: u32, active_ws: u32, occupied: &[u32]) -> String {
    if active_ws == ws_id {
        format!("{:02}", ws_id)
    } else if occupied.contains(&ws_id) {
        format!("{:02} •", ws_id)
    } else {
        format!("{:02}", ws_id)
    }
}

fn window_title_markup(title: &str, is_compact: bool) -> String {
    let escaped = gtk4::glib::markup_escape_text(title);
    let max_len = if is_compact { 32 } else { 55 };
    let char_count = escaped.chars().count();
    let display = if char_count == 0 {
        "ready".to_string()
    } else if char_count > max_len {
        let truncated: String = escaped.chars().take(max_len).collect();
        format!("{}…", truncated)
    } else {
        escaped.to_string()
    };
    format!(
        "<span font_family='Goga' font_size='8.5pt' font_weight='bold' letter_spacing='1000' foreground='#999208'>WIN //</span> <span font_family='Geist Mono' font_weight='bold' font_size='9pt' foreground='#FBFFE1'>{}</span> <span font_family='Geist Mono' font_size='9pt' foreground='#D8A664'>█</span>",
        display
    )
}

fn media_title_markup(title: Option<&str>, is_compact: bool) -> String {
    if let Some(t) = title {
        let max_len = if is_compact { 16 } else { 22 };
        let char_count = t.chars().count();
        let display = if char_count > max_len {
            let truncated: String = t.chars().take(max_len).collect();
            format!("{}…", truncated)
        } else {
            t.to_string()
        };
        let escaped = gtk4::glib::markup_escape_text(&display);
        format!(
            "<span font_family='Geist Mono' font_weight='bold' font_size='8.5pt' foreground='#FBFFE1'>{}</span>",
            escaped
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

fn markup_label(markup: &str) -> gtk4::Label {
    let lbl = gtk4::Label::new(None);
    lbl.set_markup(markup);
    lbl
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
                    set_spacing: 6,
                    set_valign: gtk4::Align::Center,

                    // Brand Anchor Pod ("Floaty blob thing")
                    gtk4::Button {
                        add_css_class: "tactile-pod",
                        add_css_class: "pod-brand",
                        connect_clicked => AppMsg::TriggerLauncher,

                        gtk4::Box {
                            set_orientation: gtk4::Orientation::Horizontal,
                            set_spacing: 7,
                            set_valign: gtk4::Align::Center,

                            gtk4::Image {
                                set_from_file: get_asset_path("s1re-mark-filled-gold.svg").as_deref(),
                                set_pixel_size: 17,
                            },
                            gtk4::Label {
                                add_css_class: "brand-text",
                                set_markup: "<span font_family='Freigeist' font_weight='800' font_size='10pt' foreground='#FBFFE1'>s1re</span><span font_family='Geist Mono' font_weight='bold' font_size='8.5pt' foreground='#999208'> //</span>",
                            }
                        }
                    },

                    // Workspaces Pod (5 at a time, super easy to tell active vs occupied vs empty)
                    gtk4::Box {
                        set_orientation: gtk4::Orientation::Horizontal,
                        set_valign: gtk4::Align::Center,
                        add_css_class: "tactile-pod",
                        add_css_class: "pod-workspaces",

                        gtk4::Button {
                            #[watch]
                            set_label: &format_ws_label(model.ws_base, model.active_workspace, &model.occupied_workspaces),
                            #[watch]
                            set_css_classes: get_ws_classes(model.active_workspace, model.ws_base, &model.occupied_workspaces),
                            connect_clicked => AppMsg::SwitchWorkspaceSlot(0),
                        },
                        gtk4::Button {
                            #[watch]
                            set_label: &format_ws_label(model.ws_base + 1, model.active_workspace, &model.occupied_workspaces),
                            #[watch]
                            set_css_classes: get_ws_classes(model.active_workspace, model.ws_base + 1, &model.occupied_workspaces),
                            connect_clicked => AppMsg::SwitchWorkspaceSlot(1),
                        },
                        gtk4::Button {
                            #[watch]
                            set_label: &format_ws_label(model.ws_base + 2, model.active_workspace, &model.occupied_workspaces),
                            #[watch]
                            set_css_classes: get_ws_classes(model.active_workspace, model.ws_base + 2, &model.occupied_workspaces),
                            connect_clicked => AppMsg::SwitchWorkspaceSlot(2),
                        },
                        gtk4::Button {
                            #[watch]
                            set_label: &format_ws_label(model.ws_base + 3, model.active_workspace, &model.occupied_workspaces),
                            #[watch]
                            set_css_classes: get_ws_classes(model.active_workspace, model.ws_base + 3, &model.occupied_workspaces),
                            connect_clicked => AppMsg::SwitchWorkspaceSlot(3),
                        },
                        gtk4::Button {
                            #[watch]
                            set_label: &format_ws_label(model.ws_base + 4, model.active_workspace, &model.occupied_workspaces),
                            #[watch]
                            set_css_classes: get_ws_classes(model.active_workspace, model.ws_base + 4, &model.occupied_workspaces),
                            connect_clicked => AppMsg::SwitchWorkspaceSlot(4),
                        },
                    },

                    // Active Window Pod ("win // thing")
                    gtk4::Box {
                        set_orientation: gtk4::Orientation::Horizontal,
                        set_spacing: 6,
                        set_valign: gtk4::Align::Center,
                        add_css_class: "tactile-pod",
                        add_css_class: "pod-window",
                        set_visible: model.current_screen == DeckScreen::Sys,
                        #[watch]
                        set_visible: model.current_screen == DeckScreen::Sys,

                        gtk4::Image {
                            set_from_file: get_asset_path("icons/window.svg").as_deref(),
                            set_pixel_size: 14,
                        },
                        gtk4::Label {
                            set_markup: &window_title_markup(&model.window_title, model.is_compact),
                            #[watch]
                            set_markup: &window_title_markup(&model.window_title, model.is_compact),
                        },
                    },
                },

                #[wrap(Some)]
                set_center_widget = &gtk4::Box {
                    set_orientation: gtk4::Orientation::Horizontal,
                    set_spacing: 6,
                    set_valign: gtk4::Align::Center,

                    // SCREEN 01: SYS (Master Chrono HUD)
                    #[name = "screen_sys_box"]
                    gtk4::Box {
                        set_orientation: gtk4::Orientation::Horizontal,
                        set_spacing: 8,
                        set_valign: gtk4::Align::Center,
                        add_css_class: "tactile-pod",
                        add_css_class: "pod-clock",
                        set_visible: model.current_screen == DeckScreen::Sys,
                        #[watch]
                        set_visible: model.current_screen == DeckScreen::Sys,

                        gtk4::Image {
                            set_from_file: get_asset_path("icons/starburst.svg").as_deref(),
                            set_pixel_size: 14,
                        },
                        gtk4::Label {
                            #[watch]
                            set_markup: &clock_markup(&model.time_str, &model.date_str),
                        },
                    },

                    // SCREEN 02: PERF (Hardware Telemetry Cockpit)
                    #[name = "screen_perf_box"]
                    gtk4::Box {
                        set_orientation: gtk4::Orientation::Horizontal,
                        set_spacing: 6,
                        set_valign: gtk4::Align::Center,
                        set_visible: model.current_screen == DeckScreen::Perf,
                        #[watch]
                        set_visible: model.current_screen == DeckScreen::Perf,

                        // CPU Pod
                        gtk4::Box {
                            set_orientation: gtk4::Orientation::Horizontal,
                            set_spacing: 6,
                            set_valign: gtk4::Align::Center,
                            add_css_class: "tactile-pod",
                            add_css_class: "pod-perf-cell",

                            gtk4::Image {
                                set_from_file: get_asset_path("icons/cpu.svg").as_deref(),
                                set_pixel_size: 14,
                            },
                            gtk4::Label {
                                #[watch]
                                set_markup: &cpu_perf_markup(model.cpu_usage, model.cpu_temp),
                            },
                        },

                        // Memory Pod
                        gtk4::Box {
                            set_orientation: gtk4::Orientation::Horizontal,
                            set_spacing: 6,
                            set_valign: gtk4::Align::Center,
                            add_css_class: "tactile-pod",
                            add_css_class: "pod-perf-cell",

                            gtk4::Image {
                                set_from_file: get_asset_path("icons/memory.svg").as_deref(),
                                set_pixel_size: 14,
                            },
                            gtk4::Label {
                                #[watch]
                                set_markup: &mem_perf_markup(model.mem_usage, model.mem_used_gb, model.mem_total_gb),
                            },
                        },

                        // NVIDIA GPU Pod
                        gtk4::Box {
                            set_orientation: gtk4::Orientation::Horizontal,
                            set_spacing: 6,
                            set_valign: gtk4::Align::Center,
                            add_css_class: "tactile-pod",
                            add_css_class: "pod-perf-cell",

                            gtk4::Label {
                                #[watch]
                                set_markup: &gpu_perf_markup(model.gpu_usage, model.gpu_vram_used_mb, model.gpu_vram_total_mb, model.gpu_temp),
                            },
                        },

                        // Storage NVMe Pod
                        gtk4::Box {
                            set_orientation: gtk4::Orientation::Horizontal,
                            set_spacing: 6,
                            set_valign: gtk4::Align::Center,
                            add_css_class: "tactile-pod",
                            add_css_class: "pod-perf-cell",

                            gtk4::Label {
                                #[watch]
                                set_markup: &disk_perf_markup(model.disk_used_pct, model.disk_free_gb),
                            },
                        },

                        // Battery Pod (when on laptop)
                        gtk4::Box {
                            set_orientation: gtk4::Orientation::Horizontal,
                            set_spacing: 6,
                            set_valign: gtk4::Align::Center,
                            add_css_class: "tactile-pod",
                            add_css_class: "pod-perf-cell",

                            gtk4::Image {
                                set_pixel_size: 14,
                                #[watch]
                                set_from_file: get_asset_path(if model.bat_status == "Charging" { "icons/battery-charging.svg" } else { "icons/battery.svg" }).as_deref(),
                            },
                            gtk4::Label {
                                #[watch]
                                set_markup: &bat_markup(model.bat_capacity, &model.bat_status),
                            },
                        },
                    },

                    // SCREEN 03: CAL (Chrono & Calendar Ribbon)
                    #[name = "screen_cal_box"]
                    gtk4::Box {
                        set_orientation: gtk4::Orientation::Horizontal,
                        set_spacing: 6,
                        set_valign: gtk4::Align::Center,
                        set_visible: model.current_screen == DeckScreen::Cal,
                        #[watch]
                        set_visible: model.current_screen == DeckScreen::Cal,

                        // Date & Week Summary Pod
                        gtk4::Box {
                            set_orientation: gtk4::Orientation::Horizontal,
                            set_spacing: 8,
                            set_valign: gtk4::Align::Center,
                            add_css_class: "tactile-pod",
                            add_css_class: "pod-cal-summary",

                            gtk4::Image {
                                set_from_file: get_asset_path("icons/starburst.svg").as_deref(),
                                set_pixel_size: 14,
                            },
                            gtk4::Label {
                                #[watch]
                                set_markup: &model.week_stats,
                            },
                        },

                        // 7-Day Current Week Ribbon
                        gtk4::Box {
                            set_orientation: gtk4::Orientation::Horizontal,
                            set_spacing: 2,
                            set_valign: gtk4::Align::Center,
                            add_css_class: "tactile-pod",
                            add_css_class: "pod-week-ribbon",

                            gtk4::Label {
                                #[watch]
                                set_markup: &model.week_ribbon,
                            },
                        },
                    },

                    // SCREEN 04: MEDIA (Expanded Studio Media Deck)
                    #[name = "screen_media_box"]
                    gtk4::Box {
                        set_orientation: gtk4::Orientation::Horizontal,
                        set_spacing: 6,
                        set_valign: gtk4::Align::Center,
                        set_visible: model.current_screen == DeckScreen::Media,
                        #[watch]
                        set_visible: model.current_screen == DeckScreen::Media,

                        // Expanded Studio Track Display
                        gtk4::Box {
                            set_orientation: gtk4::Orientation::Horizontal,
                            set_spacing: 8,
                            set_valign: gtk4::Align::Center,
                            add_css_class: "tactile-pod",
                            add_css_class: "pod-studio-track",

                            gtk4::Image {
                                set_from_file: get_asset_path("icons/mpris.svg").as_deref(),
                                set_pixel_size: 14,
                            },
                            gtk4::Label {
                                #[watch]
                                set_markup: &media_studio_markup(
                                    model.media_status.as_deref(),
                                    model.media_artist.as_deref(),
                                    model.media_title.as_deref(),
                                    model.media_player.as_deref()
                                ),
                            },
                        },

                        // Full Studio Transport Controls
                        gtk4::Box {
                            set_orientation: gtk4::Orientation::Horizontal,
                            set_spacing: 4,
                            set_valign: gtk4::Align::Center,
                            add_css_class: "tactile-pod",
                            add_css_class: "pod-studio-transport",

                            gtk4::Button {
                                add_css_class: "media-ctrl-btn",
                                connect_clicked => AppMsg::PrevMedia,
                                gtk4::Label {
                                    set_markup: "<span font_family='Geist Mono' font_weight='bold' font_size='9pt' foreground='#D1CF8B'>⏮ PREV</span>",
                                },
                            },
                            gtk4::Button {
                                add_css_class: "media-ctrl-btn",
                                connect_clicked => AppMsg::ToggleMedia,
                                gtk4::Label {
                                    #[watch]
                                    set_markup: if model.media_status.as_deref() == Some("Playing") {
                                        "<span font_family='Geist Mono' font_weight='bold' font_size='9pt' foreground='#D8A664'>⏸ PAUSE</span>"
                                    } else {
                                        "<span font_family='Geist Mono' font_weight='bold' font_size='9pt' foreground='#999208'>▶ PLAY</span>"
                                    },
                                },
                            },
                            gtk4::Button {
                                add_css_class: "media-ctrl-btn",
                                connect_clicked => AppMsg::NextMedia,
                                gtk4::Label {
                                    set_markup: "<span font_family='Geist Mono' font_weight='bold' font_size='9pt' foreground='#D1CF8B'>NEXT ⏭</span>",
                                },
                            },
                        },
                    },
                },

                #[wrap(Some)]
                set_end_widget = &gtk4::Box {
                    set_orientation: gtk4::Orientation::Horizontal,
                    set_spacing: 6,
                    set_valign: gtk4::Align::Center,

                    // Media Pod (visible when media is playing and NOT on Media screen)
                    gtk4::Box {
                        set_orientation: gtk4::Orientation::Horizontal,
                        set_spacing: 4,
                        set_valign: gtk4::Align::Center,
                        add_css_class: "tactile-pod",
                        add_css_class: "pod-media",
                        set_visible: model.media_status.is_some() && model.current_screen == DeckScreen::Sys,
                        #[watch]
                        set_visible: model.media_status.is_some() && model.current_screen == DeckScreen::Sys,

                        // Play/Pause toggle
                        gtk4::Button {
                            add_css_class: "media-ctrl-btn",
                            connect_clicked => AppMsg::ToggleMedia,
                            gtk4::Label {
                                #[watch]
                                set_markup: if model.media_status.as_deref() == Some("Playing") {
                                    "<span font_family='Geist Mono' font_weight='bold' font_size='9pt' foreground='#D8A664'>⏸</span>"
                                } else {
                                    "<span font_family='Geist Mono' font_weight='bold' font_size='9pt' foreground='#999208'>▶</span>"
                                },
                            }
                        },
                        // Track title
                        gtk4::Label {
                            add_css_class: "media-title-label",
                            #[watch]
                            set_markup: &media_title_markup(model.media_title.as_deref(), model.is_compact),
                        },
                        // Next track
                        gtk4::Button {
                            add_css_class: "media-ctrl-btn",
                            connect_clicked => AppMsg::NextMedia,
                            gtk4::Label {
                                set_markup: "<span font_family='Geist Mono' font_weight='bold' font_size='9pt' foreground='#5A6042'>⏭</span>",
                            }
                        }
                    },

                    // 4 Screens Deck Selector Pod
                    #[name = "pod_deck_selector"]
                    gtk4::Box {
                        set_orientation: gtk4::Orientation::Horizontal,
                        set_valign: gtk4::Align::Center,
                        add_css_class: "tactile-pod",
                        add_css_class: "pod-deck-selector",

                        gtk4::Button {
                            #[watch]
                            set_css_classes: if model.current_screen == DeckScreen::Sys { &["deck-chip", "active"] } else { &["deck-chip", "idle"] },
                            connect_clicked => AppMsg::SetDeckScreen(DeckScreen::Sys),
                            gtk4::Label {
                                set_markup: "<span font_family='Goga' font_weight='bold' font_size='8.5pt' letter_spacing='800'>01 SYS</span>",
                            }
                        },
                        gtk4::Button {
                            #[watch]
                            set_css_classes: if model.current_screen == DeckScreen::Perf { &["deck-chip", "active"] } else { &["deck-chip", "idle"] },
                            connect_clicked => AppMsg::SetDeckScreen(DeckScreen::Perf),
                            gtk4::Label {
                                set_markup: "<span font_family='Goga' font_weight='bold' font_size='8.5pt' letter_spacing='800'>02 PERF</span>",
                            }
                        },
                        gtk4::Button {
                            #[watch]
                            set_css_classes: if model.current_screen == DeckScreen::Cal { &["deck-chip", "active"] } else { &["deck-chip", "idle"] },
                            connect_clicked => AppMsg::SetDeckScreen(DeckScreen::Cal),
                            gtk4::Label {
                                set_markup: "<span font_family='Goga' font_weight='bold' font_size='8.5pt' letter_spacing='800'>03 CAL</span>",
                            }
                        },
                        gtk4::Button {
                            #[watch]
                            set_css_classes: if model.current_screen == DeckScreen::Media { &["deck-chip", "active"] } else { &["deck-chip", "idle"] },
                            connect_clicked => AppMsg::SetDeckScreen(DeckScreen::Media),
                            gtk4::Label {
                                set_markup: "<span font_family='Goga' font_weight='bold' font_size='8.5pt' letter_spacing='800'>04 MEDIA</span>",
                            }
                        },
                    },

                    // Bluetooth Actionable Cell (MenuButton with native GTK4 Popover)
                    gtk4::MenuButton {
                        add_css_class: "tactile-pod",
                        add_css_class: "pod-bt",
                        set_direction: gtk4::ArrowType::Down,

                        #[wrap(Some)]
                        set_child = &gtk4::Box {
                            set_orientation: gtk4::Orientation::Horizontal,
                            set_spacing: 5,
                            set_valign: gtk4::Align::Center,

                            gtk4::Image {
                                set_from_file: get_asset_path("icons/bluetooth.svg").as_deref(),
                                set_pixel_size: 14,
                            },
                            gtk4::Label {
                                #[watch]
                                set_markup: &bluetooth_markup(model.bluetooth_powered, model.bluetooth_device.as_deref(), model.is_compact),
                            }
                        },

                        #[wrap(Some)]
                        set_popover = &gtk4::Popover {
                            add_css_class: "tactile-popover",

                            gtk4::Box {
                                set_orientation: gtk4::Orientation::Vertical,
                                set_spacing: 6,
                                add_css_class: "popover-content",
                                add_css_class: "deck-bt-popover",

                                // Popover Header Bar
                                gtk4::Box {
                                    set_orientation: gtk4::Orientation::Horizontal,
                                    set_spacing: 6,
                                    set_valign: gtk4::Align::Center,

                                    gtk4::Label {
                                        set_hexpand: true,
                                        set_halign: gtk4::Align::Start,
                                        set_markup: "<span font_family='Goga' font_weight='bold' font_size='8.5pt' letter_spacing='1200' foreground='#999208'>BLUETOOTH // DECK</span>",
                                    },

                                    gtk4::Button {
                                        add_css_class: "deck-ctrl-btn",
                                        connect_clicked => AppMsg::ScanBluetooth,
                                        gtk4::Label {
                                            #[watch]
                                            set_markup: if model.bt_scanning {
                                                "<span font_family='Geist Mono' font_weight='bold' font_size='7.5pt' foreground='#D8A664'>SCANNING…</span>"
                                            } else {
                                                "<span font_family='Geist Mono' font_weight='bold' font_size='7.5pt' foreground='#D1CF8B'>SCAN</span>"
                                            },
                                        },
                                    },

                                    gtk4::Button {
                                        add_css_class: "deck-ctrl-btn",
                                        connect_clicked => AppMsg::ToggleBluetooth,
                                        gtk4::Label {
                                            #[watch]
                                            set_markup: if model.bluetooth_powered {
                                                "<span font_family='Geist Mono' font_weight='bold' font_size='7.5pt' foreground='#BE5D3C'>PWR OFF</span>"
                                            } else {
                                                "<span font_family='Geist Mono' font_weight='bold' font_size='7.5pt' foreground='#999208'>PWR ON</span>"
                                            },
                                        },
                                    },
                                },

                                // Status Capsule
                                gtk4::Box {
                                    set_orientation: gtk4::Orientation::Horizontal,
                                    set_spacing: 6,
                                    add_css_class: "deck-status-capsule",

                                    gtk4::Label {
                                        #[watch]
                                        set_markup: &format!(
                                            "<span font_family='Geist Mono' font_weight='bold' font_size='8pt' foreground='{}'>STATUS: {}</span>",
                                            if model.bluetooth_powered { "#FBFFE1" } else { "#BE5D3C" },
                                            if !model.bluetooth_powered {
                                                "POWERED OFF".to_string()
                                            } else if let Some(ref dev) = model.bluetooth_device {
                                                format!("CONNECTED // {}", dev)
                                            } else {
                                                "IDLE (NO ACTIVE CONNECTION)".to_string()
                                            }
                                        ),
                                    },
                                },

                                // Scrolled Devices List
                                gtk4::ScrolledWindow {
                                    set_max_content_height: 200,
                                    set_propagate_natural_height: true,
                                    set_hscrollbar_policy: gtk4::PolicyType::Never,
                                    set_vscrollbar_policy: gtk4::PolicyType::Automatic,
                                    add_css_class: "deck-scrolled-list",

                                    #[name = "bt_device_list_box"]
                                    gtk4::Box {
                                        set_orientation: gtk4::Orientation::Vertical,
                                        set_spacing: 3,
                                        add_css_class: "device-list-box",
                                    }
                                },
                            }
                        }
                    },

                    // Network Actionable Cell (MenuButton with native GTK4 Popover)
                    gtk4::MenuButton {
                        add_css_class: "tactile-pod",
                        add_css_class: "instrument-btn",
                        add_css_class: "pod-net",
                        set_direction: gtk4::ArrowType::Down,

                        #[wrap(Some)]
                        set_child = &gtk4::Box {
                            set_orientation: gtk4::Orientation::Horizontal,
                            set_spacing: 5,
                            set_valign: gtk4::Align::Center,

                            gtk4::Image {
                                set_from_file: get_asset_path("icons/network.svg").as_deref(),
                                set_pixel_size: 14,
                            },
                            gtk4::Label {
                                #[watch]
                                set_markup: &network_markup(model.net_quality, &model.net_type, model.net_ssid.as_deref(), model.is_compact),
                            }
                        },

                        #[wrap(Some)]
                        set_popover = &gtk4::Popover {
                            add_css_class: "tactile-popover",

                            gtk4::Box {
                                set_orientation: gtk4::Orientation::Vertical,
                                set_spacing: 6,
                                add_css_class: "popover-content",
                                add_css_class: "deck-wifi-popover",

                                // Popover Header Bar
                                gtk4::Box {
                                    set_orientation: gtk4::Orientation::Horizontal,
                                    set_spacing: 6,
                                    set_valign: gtk4::Align::Center,

                                    gtk4::Label {
                                        set_hexpand: true,
                                        set_halign: gtk4::Align::Start,
                                        set_markup: "<span font_family='Goga' font_weight='bold' font_size='8.5pt' letter_spacing='1200' foreground='#999208'>WI-FI // DECK</span>",
                                    },

                                    gtk4::Button {
                                        add_css_class: "deck-ctrl-btn",
                                        connect_clicked => AppMsg::RescanWifi,
                                        gtk4::Label {
                                            #[watch]
                                            set_markup: if model.wifi_scanning {
                                                "<span font_family='Geist Mono' font_weight='bold' font_size='7.5pt' foreground='#D8A664'>RESCANNING…</span>"
                                            } else {
                                                "<span font_family='Geist Mono' font_weight='bold' font_size='7.5pt' foreground='#D1CF8B'>RESCAN</span>"
                                            },
                                        },
                                    },

                                    gtk4::Button {
                                        add_css_class: "deck-ctrl-btn",
                                        connect_clicked => AppMsg::ToggleWifiRadio,
                                        gtk4::Label {
                                            #[watch]
                                            set_markup: if model.wifi_radio_on {
                                                "<span font_family='Geist Mono' font_weight='bold' font_size='7.5pt' foreground='#BE5D3C'>RADIO OFF</span>"
                                            } else {
                                                "<span font_family='Geist Mono' font_weight='bold' font_size='7.5pt' foreground='#999208'>RADIO ON</span>"
                                            },
                                        },
                                    },
                                },

                                // Optional Status Message Banner
                                gtk4::Box {
                                    set_orientation: gtk4::Orientation::Horizontal,
                                    add_css_class: "deck-status-capsule",
                                    #[watch]
                                    set_visible: model.wifi_status_msg.is_some(),

                                    gtk4::Label {
                                        #[watch]
                                        set_markup: &format!(
                                            "<span font_family='Geist Mono' font_weight='bold' font_size='8pt' foreground='#D8A664'>{}</span>",
                                            model.wifi_status_msg.as_deref().unwrap_or("")
                                        ),
                                    },
                                },

                                // Active Connection Pill (when connected to Wi-Fi)
                                gtk4::Box {
                                    set_orientation: gtk4::Orientation::Horizontal,
                                    set_spacing: 6,
                                    set_valign: gtk4::Align::Center,
                                    add_css_class: "deck-active-capsule",
                                    #[watch]
                                    set_visible: model.net_ssid.is_some() && model.net_type == "WIFI",

                                    gtk4::Label {
                                        set_hexpand: true,
                                        set_halign: gtk4::Align::Start,
                                        #[watch]
                                        set_markup: &format!(
                                            "<span font_family='Geist Mono' font_weight='bold' font_size='8.5pt' foreground='#FBFFE1'>ACTIVE: {}</span> <span font_family='Geist Mono' font_weight='bold' font_size='8pt' foreground='#999208'>({}%)</span>",
                                            model.net_ssid.as_deref().unwrap_or(""),
                                            model.net_quality.unwrap_or(0)
                                        ),
                                    },

                                    gtk4::Button {
                                        add_css_class: "btn-action-disconnect",
                                        connect_clicked => AppMsg::DisconnectWifi,
                                        gtk4::Label {
                                            set_markup: "<span font_family='Geist Mono' font_weight='bold' font_size='7.5pt' foreground='#BE5D3C'>DISCONNECT</span>",
                                        }
                                    }
                                },

                                // Scrolled Networks List
                                gtk4::ScrolledWindow {
                                    set_max_content_height: 200,
                                    set_propagate_natural_height: true,
                                    set_hscrollbar_policy: gtk4::PolicyType::Never,
                                    set_vscrollbar_policy: gtk4::PolicyType::Automatic,
                                    add_css_class: "deck-scrolled-list",

                                    #[name = "wifi_network_list_box"]
                                    gtk4::Box {
                                        set_orientation: gtk4::Orientation::Vertical,
                                        set_spacing: 3,
                                        add_css_class: "network-list-box",
                                    }
                                },

                                // Inline Password Entry Form
                                #[name = "wifi_pw_box"]
                                gtk4::Box {
                                    set_orientation: gtk4::Orientation::Vertical,
                                    set_spacing: 5,
                                    add_css_class: "wifi-pw-box",
                                    #[watch]
                                    set_visible: model.wifi_selected_ssid.is_some(),

                                    gtk4::Label {
                                        set_halign: gtk4::Align::Start,
                                        #[watch]
                                        set_markup: &format!(
                                            "<span font_family='Goga' font_weight='bold' font_size='8pt' letter_spacing='1000' foreground='#D8A664'>JOIN // {}</span>",
                                            gtk4::glib::markup_escape_text(model.wifi_selected_ssid.as_deref().unwrap_or(""))
                                        ),
                                    },

                                    #[name = "wifi_password_entry"]
                                    gtk4::PasswordEntry {
                                        set_show_peek_icon: true,
                                        set_placeholder_text: Some("Enter WPA Key..."),
                                        add_css_class: "wifi-pw-input",
                                        connect_activate[sender] => move |entry| {
                                            let pw = entry.text().to_string();
                                            sender.input(AppMsg::SubmitWifiPassword(pw));
                                        },
                                    },

                                    gtk4::Box {
                                        set_orientation: gtk4::Orientation::Horizontal,
                                        set_spacing: 6,
                                        set_halign: gtk4::Align::End,

                                        gtk4::Button {
                                            add_css_class: "pw-btn-cancel",
                                            connect_clicked => AppMsg::SelectWifi(None),
                                            gtk4::Label {
                                                set_markup: "<span font_family='Geist Mono' font_weight='bold' font_size='7.5pt' foreground='#5A6042'>CANCEL</span>",
                                            },
                                        },

                                        gtk4::Button {
                                            add_css_class: "pw-btn-join",
                                            connect_clicked[sender, wifi_password_entry] => move |_| {
                                                let pw = wifi_password_entry.text().to_string();
                                                sender.input(AppMsg::SubmitWifiPassword(pw));
                                            },
                                            gtk4::Label {
                                                set_markup: "<span font_family='Geist Mono' font_weight='bold' font_size='7.5pt' foreground='#141C10'>JOIN NETWORK</span>",
                                            },
                                        },
                                    },
                                },
                            }
                        }
                    },

                    // Volume Actionable Cell (MenuButton with native GTK4 Popover)
                    gtk4::MenuButton {
                        add_css_class: "tactile-pod",
                        add_css_class: "instrument-btn",
                        add_css_class: "pod-vol",
                        set_direction: gtk4::ArrowType::Down,

                        #[wrap(Some)]
                        set_child = &gtk4::Box {
                            set_orientation: gtk4::Orientation::Horizontal,
                            set_spacing: 5,
                            set_valign: gtk4::Align::Center,

                            gtk4::Image {
                                set_pixel_size: 14,
                                #[watch]
                                set_from_file: get_asset_path(if model.vol_muted { "icons/volume-muted.svg" } else { "icons/volume.svg" }).as_deref(),
                            },
                            gtk4::Label {
                                #[watch]
                                set_markup: &volume_markup(model.vol_level, model.vol_muted),
                            }
                        },

                        #[wrap(Some)]
                        set_popover = &gtk4::Popover {
                            add_css_class: "tactile-popover",
                            connect_show => AppMsg::RefreshAudioDevices,

                            gtk4::Box {
                                set_orientation: gtk4::Orientation::Vertical,
                                set_spacing: 6,
                                add_css_class: "popover-content",
                                add_css_class: "deck-audio-popover",

                                gtk4::Label {
                                    set_markup: "<span font_family='Goga' font_weight='bold' font_size='8.5pt' letter_spacing='1200' foreground='#999208'>AUDIO // OUTPUT SINK</span>",
                                },

                                gtk4::Label {
                                    #[watch]
                                    set_markup: &format!("<span font_family='Geist Mono' font_weight='bold' font_size='13pt' foreground='#FBFFE1'>{}%</span>", model.vol_level),
                                },

                                gtk4::Box {
                                    set_orientation: gtk4::Orientation::Horizontal,
                                    set_spacing: 4,
                                    set_halign: gtk4::Align::Center,

                                    gtk4::Button {
                                        add_css_class: "quick-vol-btn",
                                        connect_clicked => AppMsg::AdjustVolume(-10),
                                        gtk4::Label { set_markup: "<span font_family='Geist Mono' font_size='8pt'>-10%</span>" },
                                    },
                                    gtk4::Button {
                                        add_css_class: "quick-vol-btn",
                                        connect_clicked => AppMsg::AdjustVolume(-5),
                                        gtk4::Label { set_markup: "<span font_family='Geist Mono' font_size='8pt'>-5%</span>" },
                                    },
                                    gtk4::Button {
                                        add_css_class: "quick-vol-btn",
                                        connect_clicked => AppMsg::AdjustVolume(5),
                                        gtk4::Label { set_markup: "<span font_family='Geist Mono' font_size='8pt'>+5%</span>" },
                                    },
                                    gtk4::Button {
                                        add_css_class: "quick-vol-btn",
                                        connect_clicked => AppMsg::AdjustVolume(10),
                                        gtk4::Label { set_markup: "<span font_family='Geist Mono' font_size='8pt'>+10%</span>" },
                                    },
                                },

                                gtk4::Button {
                                    add_css_class: "menu-action-btn",
                                    connect_clicked => AppMsg::ToggleMute,
                                    gtk4::Label {
                                        #[watch]
                                        set_markup: if model.vol_muted {
                                            "<span font_family='Goga' font_weight='bold' font_size='8.5pt' foreground='#999208'>󰕾  UNMUTE AUDIO</span>"
                                        } else {
                                            "<span font_family='Goga' font_weight='bold' font_size='8.5pt' foreground='#BE5D3C'>󰝟  MUTE AUDIO</span>"
                                        },
                                    }
                                },

                                gtk4::Label {
                                    set_halign: gtk4::Align::Start,
                                    set_margin_top: 4,
                                    set_markup: "<span font_family='Goga' font_weight='bold' font_size='7.5pt' letter_spacing='1200' foreground='#999208'>OUTPUT</span>",
                                },
                                #[name = "audio_sink_list_box"]
                                gtk4::Box {
                                    set_orientation: gtk4::Orientation::Vertical,
                                    set_spacing: 3,
                                    add_css_class: "device-list-box",
                                },

                                gtk4::Label {
                                    set_halign: gtk4::Align::Start,
                                    set_margin_top: 4,
                                    set_markup: "<span font_family='Goga' font_weight='bold' font_size='7.5pt' letter_spacing='1200' foreground='#999208'>INPUT</span>",
                                },
                                #[name = "audio_source_list_box"]
                                gtk4::Box {
                                    set_orientation: gtk4::Orientation::Vertical,
                                    set_spacing: 3,
                                    add_css_class: "device-list-box",
                                },
                            }
                        }
                    },

                    // Dedicated Power Mode Latch (MenuButton with native GTK4 Popover)
                    gtk4::MenuButton {
                        add_css_class: "tactile-pod",
                        add_css_class: "pod-power",
                        set_direction: gtk4::ArrowType::Down,

                        #[wrap(Some)]
                        set_child = &gtk4::Label {
                            set_markup: power_markup(),
                        },

                        #[wrap(Some)]
                        set_popover = &gtk4::Popover {
                            add_css_class: "tactile-popover",

                            gtk4::Box {
                                set_orientation: gtk4::Orientation::Vertical,
                                set_spacing: 4,
                                add_css_class: "popover-content",

                                gtk4::Label {
                                    set_markup: "<span font_family='Goga' font_weight='bold' font_size='8.5pt' letter_spacing='1200' foreground='#999208'>SYSTEM // POWER</span>",
                                },

                                gtk4::Button {
                                    add_css_class: "menu-action-btn",
                                    connect_clicked => AppMsg::ExecLock,
                                    gtk4::Label {
                                        set_markup: "<span font_family='Goga' font_weight='bold' font_size='8.5pt' foreground='#FBFFE1'>  LOCK SCREEN</span>",
                                    }
                                },
                                gtk4::Button {
                                    add_css_class: "menu-action-btn",
                                    connect_clicked => AppMsg::ExecSuspend,
                                    gtk4::Label {
                                        set_markup: "<span font_family='Goga' font_weight='bold' font_size='8.5pt' foreground='#D1CF8B'>󰒲  SUSPEND / SLEEP</span>",
                                    }
                                },
                                gtk4::Button {
                                    add_css_class: "menu-action-btn",
                                    connect_clicked => AppMsg::ExecReboot,
                                    gtk4::Label {
                                        set_markup: "<span font_family='Goga' font_weight='bold' font_size='8.5pt' foreground='#D8A664'>󰑓  REBOOT SYSTEM</span>",
                                    }
                                },
                                gtk4::Button {
                                    add_css_class: "menu-action-btn",
                                    add_css_class: "btn-danger",
                                    connect_clicked => AppMsg::ExecPoweroff,
                                    gtk4::Label {
                                        set_markup: "<span font_family='Goga' font_weight='bold' font_size='8.5pt' foreground='#BE5D3C'>  POWER OFF</span>",
                                    }
                                },
                                gtk4::Button {
                                    add_css_class: "menu-action-btn",
                                    connect_clicked => AppMsg::ExecLogout,
                                    gtk4::Label {
                                        set_markup: "<span font_family='Goga' font_weight='bold' font_size='8.5pt' foreground='#5A6042'>󰗼  LOGOUT HYPRLAND</span>",
                                    }
                                },
                            }
                        }
                    },
                },
            },
        }
    }

    fn post_view() {
        if model.wifi_selected_ssid.is_none() {
            wifi_password_entry.set_text("");
        }

        if model.audio_dirty.swap(false, Ordering::Relaxed) {
            fill_audio_list(audio_sink_list_box, &model.audio_sinks, true, &sender);
            fill_audio_list(audio_source_list_box, &model.audio_sources, false, &sender);
        }

        if model.bt_dirty.swap(false, Ordering::Relaxed) {
            while let Some(child) = bt_device_list_box.first_child() {
                bt_device_list_box.remove(&child);
            }

            if !model.bluetooth_powered {
                let empty_lbl = markup_label("<span font_family='Geist Mono' font_size='8pt' foreground='#5A6042'>BLUETOOTH IS OFF</span>");
                empty_lbl.set_margin_top(8);
                empty_lbl.set_margin_bottom(8);
                bt_device_list_box.append(&empty_lbl);
            } else if model.bt_devices.is_empty() {
                let empty_lbl = markup_label("<span font_family='Geist Mono' font_size='8pt' foreground='#5A6042'>NO PAIRED DEVICES FOUND</span>");
                empty_lbl.set_margin_top(8);
                empty_lbl.set_margin_bottom(8);
                bt_device_list_box.append(&empty_lbl);
            } else {
                for dev in &model.bt_devices {
                    let row = gtk4::Box::builder()
                        .orientation(gtk4::Orientation::Horizontal)
                        .spacing(8)
                        .css_classes(["bt-device-row"])
                        .build();

                    let icon_glyph = if dev.name.to_lowercase().contains("bud")
                        || dev.name.to_lowercase().contains("headphone")
                        || dev.name.to_lowercase().contains("audio")
                    {
                        "󰋋"
                    } else if dev.name.to_lowercase().contains("mouse") {
                        "󰍽"
                    } else if dev.name.to_lowercase().contains("key") {
                        "󰌌"
                    } else {
                        "󰂯"
                    };

                    let icon_color = if dev.connected { "#999208" } else { "#5A6042" };
                    let icon_lbl = markup_label(&format!(
                        "<span font_family='Geist Mono' font_size='10pt' foreground='{}'>{}</span>",
                        icon_color, icon_glyph
                    ));
                    row.append(&icon_lbl);

                    let info_box = gtk4::Box::builder()
                        .orientation(gtk4::Orientation::Vertical)
                        .spacing(1)
                        .hexpand(true)
                        .build();

                    let name_escaped = gtk4::glib::markup_escape_text(&dev.name);
                    let name_lbl = markup_label(&format!(
                        "<span font_family='Geist Mono' font_weight='bold' font_size='8pt' foreground='{}'>{}</span>",
                        if dev.connected { "#FBFFE1" } else { "#D1CF8B" },
                        name_escaped
                    ));
                    name_lbl.set_halign(gtk4::Align::Start);
                    name_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);

                    let mac_lbl = markup_label(&format!(
                        "<span font_family='Geist Mono' font_size='6.5pt' foreground='#5A6042'>{}</span>",
                        dev.mac
                    ));
                    mac_lbl.set_halign(gtk4::Align::Start);

                    info_box.append(&name_lbl);
                    info_box.append(&mac_lbl);
                    row.append(&info_box);

                    let btn = gtk4::Button::builder().build();
                    let sender_c = sender.clone();
                    let mac_c = dev.mac.clone();

                    if dev.connected {
                        btn.add_css_class("btn-action-disconnect");
                        let btn_lbl = markup_label("<span font_family='Geist Mono' font_weight='bold' font_size='7pt' foreground='#BE5D3C'>DISCONNECT</span>");
                        btn.set_child(Some(&btn_lbl));
                        btn.connect_clicked(move |_| {
                            sender_c.input(AppMsg::DisconnectBluetooth(mac_c.clone()));
                        });
                    } else {
                        btn.add_css_class("btn-action-connect");
                        let btn_lbl = markup_label("<span font_family='Geist Mono' font_weight='bold' font_size='7pt' foreground='#141C10'>CONNECT</span>");
                        btn.set_child(Some(&btn_lbl));
                        btn.connect_clicked(move |_| {
                            sender_c.input(AppMsg::ConnectBluetooth(mac_c.clone()));
                        });
                    }
                    row.append(&btn);
                    bt_device_list_box.append(&row);
                }
            }
        }

        if model.wifi_dirty.swap(false, Ordering::Relaxed) {
            while let Some(child) = wifi_network_list_box.first_child() {
                wifi_network_list_box.remove(&child);
            }

            if !model.wifi_radio_on {
                let empty_lbl = markup_label("<span font_family='Geist Mono' font_size='8pt' foreground='#5A6042'>WI-FI RADIO IS DISABLED</span>");
                empty_lbl.set_margin_top(8);
                empty_lbl.set_margin_bottom(8);
                wifi_network_list_box.append(&empty_lbl);
            } else if model.wifi_networks.is_empty() {
                let empty_lbl = markup_label("<span font_family='Geist Mono' font_size='8pt' foreground='#5A6042'>SCANNING FOR NETWORKS...</span>");
                empty_lbl.set_margin_top(8);
                empty_lbl.set_margin_bottom(8);
                wifi_network_list_box.append(&empty_lbl);
            } else {
                for net in &model.wifi_networks {
                    let row = gtk4::Box::builder()
                        .orientation(gtk4::Orientation::Horizontal)
                        .spacing(8)
                        .css_classes(["wifi-network-row"])
                        .build();

                    let signal_glyph = if net.signal >= 75 {
                        "▂▄▆█"
                    } else if net.signal >= 50 {
                        "▂▄▆_"
                    } else if net.signal >= 25 {
                        "▂▄__"
                    } else {
                        "▂___"
                    };
                    let sig_color = if net.signal >= 60 {
                        "#999208"
                    } else if net.signal >= 35 {
                        "#D8A664"
                    } else {
                        "#5A6042"
                    };
                    let sig_lbl = markup_label(&format!(
                        "<span font_family='Geist Mono' font_size='7pt' foreground='{}'>{}</span>",
                        sig_color, signal_glyph
                    ));
                    row.append(&sig_lbl);

                    let info_box = gtk4::Box::builder()
                        .orientation(gtk4::Orientation::Vertical)
                        .spacing(1)
                        .hexpand(true)
                        .build();

                    let ssid_escaped = gtk4::glib::markup_escape_text(&net.ssid);
                    let ssid_lbl = markup_label(&format!(
                        "<span font_family='Geist Mono' font_weight='bold' font_size='8pt' foreground='{}'>{}</span>",
                        if net.in_use { "#FBFFE1" } else { "#D1CF8B" },
                        ssid_escaped
                    ));
                    ssid_lbl.set_halign(gtk4::Align::Start);
                    ssid_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);

                    let sec_str = if net.security.is_empty() || net.security == "--" {
                        "OPEN"
                    } else {
                        &net.security
                    };
                    let tag_str = if net.in_use {
                        format!(
                            "<span foreground='#999208'>CONNECTED</span> · <span foreground='#5A6042'>{} · {}%</span>",
                            sec_str, net.signal
                        )
                    } else if net.is_known {
                        format!(
                            "<span foreground='#D8A664'>SAVED</span> · <span foreground='#5A6042'>{} · {}%</span>",
                            sec_str, net.signal
                        )
                    } else {
                        format!(
                            "<span foreground='#5A6042'>{} · {}%</span>",
                            sec_str, net.signal
                        )
                    };
                    let tag_lbl = markup_label(&format!(
                        "<span font_family='Geist Mono' font_size='6.5pt'>{}</span>",
                        tag_str
                    ));
                    tag_lbl.set_halign(gtk4::Align::Start);

                    info_box.append(&ssid_lbl);
                    info_box.append(&tag_lbl);
                    row.append(&info_box);

                    let btn = gtk4::Button::builder().build();
                    let sender_c = sender.clone();
                    let ssid_c = net.ssid.clone();

                    if net.in_use {
                        btn.add_css_class("btn-action-disconnect");
                        let btn_lbl = markup_label("<span font_family='Geist Mono' font_weight='bold' font_size='7pt' foreground='#BE5D3C'>DISCONNECT</span>");
                        btn.set_child(Some(&btn_lbl));
                        btn.connect_clicked(move |_| {
                            sender_c.input(AppMsg::DisconnectWifi);
                        });
                    } else if net.is_known || net.security.is_empty() || net.security == "--" {
                        btn.add_css_class("btn-action-connect");
                        let btn_lbl = markup_label("<span font_family='Geist Mono' font_weight='bold' font_size='7pt' foreground='#141C10'>CONNECT</span>");
                        btn.set_child(Some(&btn_lbl));
                        btn.connect_clicked(move |_| {
                            sender_c.input(AppMsg::ConnectWifi(ssid_c.clone()));
                        });
                    } else {
                        btn.add_css_class("btn-action-join");
                        let btn_lbl = markup_label("<span font_family='Geist Mono' font_weight='bold' font_size='7pt' foreground='#D8A664'>JOIN</span>");
                        btn.set_child(Some(&btn_lbl));
                        btn.connect_clicked(move |_| {
                            sender_c.input(AppMsg::SelectWifi(Some(ssid_c.clone())));
                        });
                    }
                    row.append(&btn);
                    wifi_network_list_box.append(&row);
                }
            }
        }
    }

    fn init(
        target_monitor: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let is_compact = target_monitor.starts_with("eDP");
        if is_compact {
            root.add_css_class("compact");
        }

        // Initialize Wayland Layer Shell
        root.init_layer_shell();
        root.set_layer(Layer::Top);
        root.set_anchor(Edge::Top, true);
        root.set_anchor(Edge::Left, true);
        root.set_anchor(Edge::Right, true);
        root.set_keyboard_mode(KeyboardMode::OnDemand);
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
        let mem_used_gb = sys.used_memory() as f32 / (1024.0 * 1024.0 * 1024.0);
        let mem_total_gb = sys.total_memory() as f32 / (1024.0 * 1024.0 * 1024.0);
        let cpu_temp = read_cpu_temp();
        let (gpu_usage, gpu_vram_used_mb, gpu_vram_total_mb, gpu_temp) = read_gpu();
        let (disk_used_pct, disk_free_gb) = read_disk();
        let (week_ribbon, week_stats) = format_calendar();

        let (vol_level, vol_muted) = read_volume();
        let (audio_sinks, audio_sources) = read_audio_devices();
        let (bat_capacity, bat_status) = read_battery();
        let (net_quality, net_type, net_ssid) = read_network();
        let (bluetooth_powered, bluetooth_device) = read_bluetooth();
        let bt_devices = read_bluetooth_devices();
        let wifi_radio_on = read_wifi_radio();
        let wifi_networks = read_wifi_networks();
        let (media_status, media_title, media_artist, media_player) = read_media()
            .map(|(s, t, a, p)| (Some(s), Some(t), a, p))
            .unwrap_or((None, None, None, None));
        let (active_ws, occupied_ws, window_title, ws_base) =
            read_hyprland_state_fast(&target_monitor);

        let now = chrono::Local::now();
        let initial_screen = std::env::var("TAKT_SCREEN")
            .ok()
            .and_then(|s| DeckScreen::from_str_name(&s))
            .unwrap_or(DeckScreen::Sys);
        let model = AppModel {
            target_monitor: target_monitor.clone(),
            ws_base,
            is_compact,
            current_screen: initial_screen,

            time_str: now.format("%H:%M").to_string(),
            date_str: now.format("%a · %d %b").to_string().to_uppercase(),
            week_ribbon,
            week_stats,

            active_workspace: active_ws,
            occupied_workspaces: occupied_ws,
            window_title,

            cpu_usage: sys.global_cpu_usage(),
            cpu_temp,
            mem_usage: mem,
            mem_used_gb,
            mem_total_gb,

            gpu_usage,
            gpu_vram_used_mb,
            gpu_vram_total_mb,
            gpu_temp,

            disk_used_pct,
            disk_free_gb,

            vol_level,
            vol_muted,
            audio_sinks,
            audio_sources,
            audio_dirty: AtomicBool::new(true),
            bat_capacity,
            bat_status,
            net_quality,
            net_type,
            net_ssid,

            bluetooth_powered,
            bluetooth_device,
            bt_devices,
            bt_scanning: false,
            bt_dirty: AtomicBool::new(true),

            wifi_radio_on,
            wifi_networks,
            wifi_scanning: false,
            wifi_selected_ssid: None,
            wifi_status_msg: None,
            wifi_dirty: AtomicBool::new(true),

            media_status,
            media_title,
            media_artist,
            media_player,
        };

        let widgets = view_output!();

        // Mouse Wheel Scroll Controller for cockpit deck cycling
        let last_scroll = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));
        attach_scroll_cycle(
            &widgets.pod_deck_selector,
            sender.clone(),
            last_scroll.clone(),
        );
        attach_scroll_cycle(&widgets.screen_sys_box, sender.clone(), last_scroll.clone());
        attach_scroll_cycle(
            &widgets.screen_perf_box,
            sender.clone(),
            last_scroll.clone(),
        );
        attach_scroll_cycle(&widgets.screen_cal_box, sender.clone(), last_scroll.clone());
        attach_scroll_cycle(&widgets.screen_media_box, sender.clone(), last_scroll);

        // IPC Server Listener for multi-monitor synchronization & CLI commands
        let sock_path = get_monitor_socket_path(&target_monitor);
        let _ = std::fs::remove_file(&sock_path);
        let sender_ipc = sender.clone();
        let mon_for_ipc = target_monitor.clone();

        std::thread::spawn(move || {
            if let Ok(listener) = std::os::unix::net::UnixListener::bind(&sock_path) {
                if !mon_for_ipc.starts_with("eDP") {
                    let runtime = std::env::var("XDG_RUNTIME_DIR")
                        .unwrap_or_else(|_| "/run/user/1000".to_string());
                    let global_sock = std::path::PathBuf::from(runtime).join("takt.sock");
                    let _ = std::fs::remove_file(&global_sock);
                    let _ = std::os::unix::fs::symlink(&sock_path, &global_sock);
                }

                for stream in listener.incoming().flatten() {
                    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(500)));
                    let _ = stream.set_write_timeout(Some(std::time::Duration::from_millis(500)));
                    let mut reader = BufReader::new(stream);
                    let mut line = String::new();
                    if reader.read_line(&mut line).is_ok() {
                        let trimmed = line.trim();
                        let parts: Vec<&str> = trimmed.split_whitespace().collect();
                        if let Some(cmd) = parts.first() {
                            match cmd.to_ascii_uppercase().as_str() {
                                "NEXT" => {
                                    sender_ipc.input(AppMsg::CycleDeckScreen(1));
                                    let mut s = reader.into_inner();
                                    let _ = s.write_all(b"OK NEXT\n");
                                }
                                "PREV" => {
                                    sender_ipc.input(AppMsg::CycleDeckScreen(-1));
                                    let mut s = reader.into_inner();
                                    let _ = s.write_all(b"OK PREV\n");
                                }
                                "SET" => {
                                    if let Some(target) = parts.get(1) {
                                        if let Some(screen) = DeckScreen::from_str_name(target) {
                                            sender_ipc.input(AppMsg::SetDeckScreenRemote(screen));
                                            let mut s = reader.into_inner();
                                            let _ = s.write_all(
                                                format!("OK {}\n", screen.as_str()).as_bytes(),
                                            );
                                        }
                                    }
                                }
                                "SCREEN" => {
                                    if let Some(target) = parts.get(1) {
                                        if let Some(screen) = DeckScreen::from_str_name(target) {
                                            sender_ipc.input(AppMsg::SetDeckScreen(screen));
                                            let mut s = reader.into_inner();
                                            let _ = s.write_all(
                                                format!("OK {}\n", screen.as_str()).as_bytes(),
                                            );
                                        }
                                    }
                                }
                                "PING" => {
                                    let mut s = reader.into_inner();
                                    let _ = s.write_all(b"PONG\n");
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        });

        // Background Telemetry Worker (every 1 sec)
        let sender_telemetry = sender.clone();
        std::thread::spawn(move || {
            let mut sys = sysinfo::System::new_all();
            let mut tick_count: u32 = 0;
            loop {
                std::thread::sleep(std::time::Duration::from_millis(1000));
                tick_count = tick_count.wrapping_add(1);
                sys.refresh_cpu_all();
                sys.refresh_memory();

                let cpu = sys.global_cpu_usage();
                let cpu_temp = read_cpu_temp();
                let mem = (sys.used_memory() as f32 / sys.total_memory() as f32) * 100.0;
                let mem_used_gb = sys.used_memory() as f32 / (1024.0 * 1024.0 * 1024.0);
                let mem_total_gb = sys.total_memory() as f32 / (1024.0 * 1024.0 * 1024.0);
                let (gpu_usage, gpu_vram_used_mb, gpu_vram_total_mb, gpu_temp) = read_gpu();
                let (disk_used_pct, disk_free_gb) = read_disk();
                let (vol, vol_muted) = read_volume();
                let (bat_capacity, bat_status) = read_battery();
                let (net_quality, net_type, net_ssid) = read_network();
                let (bluetooth_powered, bluetooth_device) = read_bluetooth();
                let (media_status, media_title, media_artist, media_player) = read_media()
                    .map(|(s, t, a, p)| (Some(s), Some(t), a, p))
                    .unwrap_or((None, None, None, None));

                sender_telemetry.input(AppMsg::UpdateTelemetry(Box::new(TelemetryData {
                    cpu,
                    cpu_temp,
                    mem,
                    mem_used_gb,
                    mem_total_gb,
                    gpu_usage,
                    gpu_vram_used_mb,
                    gpu_vram_total_mb,
                    gpu_temp,
                    disk_used_pct,
                    disk_free_gb,
                    vol,
                    vol_muted,
                    bat_capacity,
                    bat_status,
                    net_quality,
                    net_type,
                    net_ssid,
                    bluetooth_powered,
                    bluetooth_device,
                    media_status,
                    media_title,
                    media_artist,
                    media_player,
                })));
                sender_telemetry.input(AppMsg::Tick);

                // Periodic device sync every 8 seconds
                if tick_count.is_multiple_of(8) {
                    let s_sync = sender_telemetry.clone();
                    std::thread::spawn(move || {
                        let devs = read_bluetooth_devices();
                        let nets = read_wifi_networks();
                        let (sinks, sources) = read_audio_devices();
                        s_sync.input(AppMsg::UpdateBtDevices(devs));
                        s_sync.input(AppMsg::UpdateWifiNetworks(nets));
                        s_sync.input(AppMsg::UpdateAudioDevices(sinks, sources));
                    });
                }
            }
        });

        // Real-Time Push Event Listener (.socket2.sock)
        let sender_hypr_event = sender.clone();
        let mon_for_hypr_event = target_monitor.clone();
        std::thread::spawn(move || {
            let runtime =
                std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/run/user/1000".to_string());
            if let Ok(sig) = std::env::var("HYPRLAND_INSTANCE_SIGNATURE") {
                let path = format!("{}/hypr/{}/.socket2.sock", runtime, sig);
                if let Ok(stream) = UnixStream::connect(path) {
                    let reader = BufReader::new(stream);
                    for line in reader.lines().map_while(Result::ok) {
                        if line.starts_with("monitoradded>>") {
                            let added_mon = line.trim_start_matches("monitoradded>>").trim();
                            if !added_mon.is_empty() && added_mon != mon_for_hypr_event {
                                let exe = std::env::current_exe()
                                    .unwrap_or_else(|_| std::path::PathBuf::from("takt"));
                                let _ = std::process::Command::new(&exe)
                                    .args(["--monitor", added_mon])
                                    .stdin(std::process::Stdio::null())
                                    .stdout(std::process::Stdio::null())
                                    .stderr(std::process::Stdio::null())
                                    .spawn();
                            }
                        }

                        if line.starts_with("windowtitle")
                            || line.starts_with("activewindow")
                            || line.starts_with("workspace")
                            || line.starts_with("focusedmon")
                            || line.starts_with("openwindow")
                            || line.starts_with("closewindow")
                            || line.starts_with("movewindow")
                            || line.starts_with("monitoradded")
                            || line.starts_with("monitorremoved")
                        {
                            let (active_ws, occupied_ws, title, ws_base) =
                                read_hyprland_state_fast(&mon_for_hypr_event);
                            sender_hypr_event.input(AppMsg::UpdateHyprland {
                                active_ws,
                                occupied_ws,
                                title,
                                ws_base,
                            });
                        }
                    }
                }
            }
        });

        // Fast Delta-Checking Fallback Worker (every 40ms)
        let sender_hypr_fast = sender.clone();
        let mon_for_hypr_fast = target_monitor;
        std::thread::spawn(move || {
            let mut last_title = String::new();
            let mut last_ws = 0u32;
            let mut last_ws_base = 0u32;
            loop {
                std::thread::sleep(std::time::Duration::from_millis(40));
                let (active_ws, occupied_ws, title, ws_base) =
                    read_hyprland_state_fast(&mon_for_hypr_fast);
                if title != last_title || active_ws != last_ws || ws_base != last_ws_base {
                    last_title = title.clone();
                    last_ws = active_ws;
                    last_ws_base = ws_base;
                    sender_hypr_fast.input(AppMsg::UpdateHyprland {
                        active_ws,
                        occupied_ws,
                        title,
                        ws_base,
                    });
                }
            }
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            AppMsg::Tick => {
                let now = chrono::Local::now();
                self.time_str = now.format("%H:%M").to_string();
                self.date_str = now.format("%a · %d %b").to_string().to_uppercase();
                let (ribbon, stats) = format_calendar();
                self.week_ribbon = ribbon;
                self.week_stats = stats;
            }
            AppMsg::UpdateTelemetry(data) => {
                self.cpu_usage = data.cpu;
                self.cpu_temp = data.cpu_temp;
                self.mem_usage = data.mem;
                self.mem_used_gb = data.mem_used_gb;
                self.mem_total_gb = data.mem_total_gb;
                self.gpu_usage = data.gpu_usage;
                self.gpu_vram_used_mb = data.gpu_vram_used_mb;
                self.gpu_vram_total_mb = data.gpu_vram_total_mb;
                self.gpu_temp = data.gpu_temp;
                self.disk_used_pct = data.disk_used_pct;
                self.disk_free_gb = data.disk_free_gb;
                self.vol_level = data.vol;
                self.vol_muted = data.vol_muted;
                self.bat_capacity = data.bat_capacity;
                self.bat_status = data.bat_status;
                self.net_quality = data.net_quality;
                self.net_type = data.net_type;
                self.net_ssid = data.net_ssid;
                self.bluetooth_powered = data.bluetooth_powered;
                self.bluetooth_device = data.bluetooth_device;
                self.media_status = data.media_status;
                self.media_title = data.media_title;
                self.media_artist = data.media_artist;
                self.media_player = data.media_player;
            }
            AppMsg::UpdateHyprland {
                active_ws,
                occupied_ws,
                title,
                ws_base,
            } => {
                self.active_workspace = active_ws;
                self.occupied_workspaces = occupied_ws;
                self.window_title = title;
                self.ws_base = ws_base;
            }
            AppMsg::SwitchWorkspaceSlot(slot) => {
                let ws_id = self.ws_base + slot;
                let _ = std::process::Command::new("hyprctl")
                    .args(["dispatch", "workspace", &ws_id.to_string()])
                    .spawn();
            }
            AppMsg::CycleDeckScreen(delta) => {
                let new_screen = if delta > 0 {
                    self.current_screen.next()
                } else {
                    self.current_screen.prev()
                };
                self.current_screen = new_screen;
                broadcast_ipc_screen_change(new_screen, Some(&self.target_monitor));
            }
            AppMsg::SetDeckScreen(screen) => {
                self.current_screen = screen;
                broadcast_ipc_screen_change(screen, Some(&self.target_monitor));
            }
            AppMsg::SetDeckScreenRemote(screen) => {
                self.current_screen = screen;
            }
            AppMsg::ToggleMute => {
                let _ = std::process::Command::new("wpctl")
                    .args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"])
                    .spawn();
                let (vol, muted) = read_volume();
                self.vol_level = vol;
                self.vol_muted = muted;
            }
            AppMsg::AdjustVolume(delta) => {
                let arg = if delta >= 0 {
                    format!("{}%+", delta)
                } else {
                    format!("{}%-", delta.abs())
                };
                let _ = std::process::Command::new("wpctl")
                    .args(["set-volume", "-l", "1.5", "@DEFAULT_AUDIO_SINK@", &arg])
                    .status();
                let (vol, muted) = read_volume();
                self.vol_level = vol;
                self.vol_muted = muted;
            }
            AppMsg::RefreshAudioDevices => {
                let sender_c = _sender.clone();
                std::thread::spawn(move || {
                    let (sinks, sources) = read_audio_devices();
                    sender_c.input(AppMsg::UpdateAudioDevices(sinks, sources));
                });
            }
            AppMsg::SetDefaultSink(name) => {
                let sender_c = _sender.clone();
                std::thread::spawn(move || {
                    set_default_audio_device(true, &name);
                    let (sinks, sources) = read_audio_devices();
                    sender_c.input(AppMsg::UpdateAudioDevices(sinks, sources));
                });
            }
            AppMsg::SetDefaultSource(name) => {
                let sender_c = _sender.clone();
                std::thread::spawn(move || {
                    set_default_audio_device(false, &name);
                    let (sinks, sources) = read_audio_devices();
                    sender_c.input(AppMsg::UpdateAudioDevices(sinks, sources));
                });
            }
            AppMsg::UpdateAudioDevices(sinks, sources) => {
                if sinks != self.audio_sinks || sources != self.audio_sources {
                    self.audio_sinks = sinks;
                    self.audio_sources = sources;
                    self.audio_dirty.store(true, Ordering::Relaxed);
                }
                let (vol, muted) = read_volume();
                self.vol_level = vol;
                self.vol_muted = muted;
            }
            AppMsg::ToggleBluetooth => {
                let new_state = if self.bluetooth_powered { "off" } else { "on" };
                let _ = std::process::Command::new("bluetoothctl")
                    .args(["power", new_state])
                    .status();
                let (powered, dev) = read_bluetooth();
                self.bluetooth_powered = powered;
                self.bluetooth_device = dev;
                self.bt_devices = read_bluetooth_devices();
                self.bt_dirty.store(true, Ordering::Relaxed);
            }
            AppMsg::ScanBluetooth => {
                self.bt_scanning = true;
                let sender_c = _sender.clone();
                std::thread::spawn(move || {
                    let _ = std::process::Command::new("bluetoothctl")
                        .args(["--timeout", "5", "scan", "on"])
                        .output();
                    let devs = read_bluetooth_devices();
                    sender_c.input(AppMsg::UpdateBtDevices(devs));
                });
            }
            AppMsg::ConnectBluetooth(mac) => {
                let sender_c = _sender.clone();
                std::thread::spawn(move || {
                    let _ = std::process::Command::new("bluetoothctl")
                        .args(["connect", &mac])
                        .output();
                    let devs = read_bluetooth_devices();
                    sender_c.input(AppMsg::UpdateBtDevices(devs));
                });
            }
            AppMsg::DisconnectBluetooth(mac) => {
                let sender_c = _sender.clone();
                std::thread::spawn(move || {
                    let _ = std::process::Command::new("bluetoothctl")
                        .args(["disconnect", &mac])
                        .output();
                    let devs = read_bluetooth_devices();
                    sender_c.input(AppMsg::UpdateBtDevices(devs));
                });
            }
            AppMsg::UpdateBtDevices(devs) => {
                self.bt_scanning = false;
                self.bt_devices = devs;
                let (powered, dev) = read_bluetooth();
                self.bluetooth_powered = powered;
                self.bluetooth_device = dev;
                self.bt_dirty.store(true, Ordering::Relaxed);
            }
            AppMsg::ToggleWifiRadio => {
                let new_state = if self.wifi_radio_on { "off" } else { "on" };
                let _ = std::process::Command::new("nmcli")
                    .args(["radio", "wifi", new_state])
                    .status();
                self.wifi_radio_on = !self.wifi_radio_on;
                self.wifi_networks = read_wifi_networks();
                let (q, t, s) = read_network();
                self.net_quality = q;
                self.net_type = t;
                self.net_ssid = s;
                self.wifi_dirty.store(true, Ordering::Relaxed);
            }
            AppMsg::RescanWifi => {
                self.wifi_scanning = true;
                let sender_c = _sender.clone();
                std::thread::spawn(move || {
                    let _ = std::process::Command::new("nmcli")
                        .args(["device", "wifi", "rescan"])
                        .output();
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                    let nets = read_wifi_networks();
                    sender_c.input(AppMsg::UpdateWifiNetworks(nets));
                });
            }
            AppMsg::ConnectWifi(ssid) => {
                self.wifi_status_msg = Some(format!("CONNECTING TO {}…", ssid));
                let sender_c = _sender.clone();
                std::thread::spawn(move || {
                    let _ = std::process::Command::new("nmcli")
                        .args(["connection", "up", "id", &ssid])
                        .output();
                    let nets = read_wifi_networks();
                    sender_c.input(AppMsg::UpdateWifiNetworks(nets));
                    sender_c.input(AppMsg::WifiActionStatus(None));
                });
            }
            AppMsg::SubmitWifiPassword(pw) => {
                if let Some(ssid) = self.wifi_selected_ssid.clone() {
                    self.wifi_status_msg = Some(format!("JOINING {}…", ssid));
                    let sender_c = _sender.clone();
                    std::thread::spawn(move || {
                        let res = std::process::Command::new("nmcli")
                            .args(["device", "wifi", "connect", &ssid, "password", &pw])
                            .output();
                        let nets = read_wifi_networks();
                        sender_c.input(AppMsg::UpdateWifiNetworks(nets));
                        match res {
                            Ok(o) if o.status.success() => {
                                sender_c.input(AppMsg::SelectWifi(None));
                                sender_c.input(AppMsg::WifiActionStatus(None));
                            }
                            Ok(o) => {
                                let err = String::from_utf8_lossy(&o.stderr);
                                let msg = if err.contains("Secrets were required")
                                    || err.contains("password")
                                {
                                    "AUTH FAILED: WRONG PASSWORD".to_string()
                                } else {
                                    "CONNECTION FAILED".to_string()
                                };
                                sender_c.input(AppMsg::WifiActionStatus(Some(msg)));
                            }
                            Err(_) => {
                                sender_c.input(AppMsg::WifiActionStatus(Some(
                                    "ERROR RUNNING NMCLI".to_string(),
                                )));
                            }
                        }
                    });
                }
            }
            AppMsg::DisconnectWifi => {
                let sender_c = _sender.clone();
                std::thread::spawn(move || {
                    let _ = std::process::Command::new("nmcli")
                        .args(["device", "disconnect", "wlan0"])
                        .output();
                    let nets = read_wifi_networks();
                    sender_c.input(AppMsg::UpdateWifiNetworks(nets));
                });
            }
            AppMsg::SelectWifi(opt) => {
                self.wifi_selected_ssid = opt;
                self.wifi_status_msg = None;
            }
            AppMsg::UpdateWifiNetworks(nets) => {
                self.wifi_scanning = false;
                self.wifi_networks = nets;
                let (q, t, s) = read_network();
                self.net_quality = q;
                self.net_type = t;
                self.net_ssid = s;
                self.wifi_dirty.store(true, Ordering::Relaxed);
            }
            AppMsg::WifiActionStatus(status) => {
                self.wifi_status_msg = status;
            }
            AppMsg::ToggleMedia => {
                if let Some(ref mut s) = self.media_status {
                    if s == "Playing" {
                        *s = "Paused".to_string();
                    } else {
                        *s = "Playing".to_string();
                    }
                }
                let _ = std::process::Command::new("playerctl")
                    .arg("play-pause")
                    .spawn();
            }
            AppMsg::PrevMedia => {
                let _ = std::process::Command::new("playerctl")
                    .arg("previous")
                    .spawn();
            }
            AppMsg::NextMedia => {
                let _ = std::process::Command::new("playerctl").arg("next").spawn();
            }
            AppMsg::ExecLock => {
                let _ = std::process::Command::new("hyprlock").spawn();
            }
            AppMsg::ExecSuspend => {
                let _ = std::process::Command::new("systemctl")
                    .arg("suspend")
                    .spawn();
            }
            AppMsg::ExecReboot => {
                let _ = std::process::Command::new("systemctl")
                    .arg("reboot")
                    .spawn();
            }
            AppMsg::ExecPoweroff => {
                let _ = std::process::Command::new("systemctl")
                    .arg("poweroff")
                    .spawn();
            }
            AppMsg::ExecLogout => {
                let _ = std::process::Command::new("hyprctl")
                    .args(["dispatch", "exit"])
                    .spawn();
            }
            AppMsg::TriggerLauncher => {
                let _ = std::process::Command::new("rofi")
                    .args(["-show", "drun"])
                    .spawn();
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

/// Sinks and sources as PipeWire's pulse server reports them, monitors excluded.
fn read_audio_devices() -> (Vec<AudioDeviceInfo>, Vec<AudioDeviceInfo>) {
    (
        read_audio_list("sinks", "get-default-sink"),
        read_audio_list("sources", "get-default-source"),
    )
}

fn read_audio_list(kind: &str, default_cmd: &str) -> Vec<AudioDeviceInfo> {
    let default = std::process::Command::new("pactl")
        .arg(default_cmd)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let Ok(out) = std::process::Command::new("pactl")
        .args(["-f", "json", "list", kind])
        .output()
    else {
        return Vec::new();
    };
    let Ok(serde_json::Value::Array(items)) = serde_json::from_slice(&out.stdout) else {
        return Vec::new();
    };
    let is_sink = kind == "sinks";
    items
        .iter()
        .filter_map(|item| {
            let props = item.get("properties")?;
            let prop = |key: &str| props.get(key).and_then(|v| v.as_str()).unwrap_or("");
            if prop("device.class") == "monitor" {
                return None;
            }
            let name = item.get("name")?.as_str()?.to_string();
            let description = item
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or(&name)
                .to_string();
            let lname = name.to_lowercase();
            let glyph = if name.starts_with("bluez_") || prop("device.bus") == "bluetooth" {
                "󰋋"
            } else if lname.contains("hdmi") {
                "󰍹"
            } else if lname.contains("headphone") || lname.contains("headset") {
                "󰋋"
            } else if is_sink {
                "󰓃"
            } else {
                "󰍬"
            };
            Some(AudioDeviceInfo {
                is_default: name == default,
                name,
                description,
                glyph,
            })
        })
        .collect()
}

/// Make `name` the default sink/source and move every movable stream onto it, so playing
/// audio follows the switch even when the stream was pinned to the previous device.
fn set_default_audio_device(is_sink: bool, name: &str) {
    let (set_cmd, streams, move_cmd) = if is_sink {
        ("set-default-sink", "sink-inputs", "move-sink-input")
    } else {
        ("set-default-source", "source-outputs", "move-source-output")
    };
    let _ = std::process::Command::new("pactl")
        .args([set_cmd, name])
        .status();
    let Ok(out) = std::process::Command::new("pactl")
        .args(["-f", "json", "list", streams])
        .output()
    else {
        return;
    };
    let Ok(serde_json::Value::Array(items)) = serde_json::from_slice(&out.stdout) else {
        return;
    };
    for item in &items {
        let Some(index) = item.get("index").and_then(|v| v.as_u64()) else {
            continue;
        };
        let props = item.get("properties");
        let prop = |key: &str| {
            props
                .and_then(|p| p.get(key))
                .and_then(|v| v.as_str())
                .unwrap_or("")
        };
        // WirePlumber's Bluetooth loopbacks and streams that asked not to be moved stay put
        if prop("media.class").ends_with("/Internal")
            || prop("node.name").starts_with("bluez_")
            || prop("node.dont-move") == "true"
        {
            continue;
        }
        let _ = std::process::Command::new("pactl")
            .args([move_cmd, &index.to_string(), name])
            .status();
    }
}

fn fill_audio_list(
    list: &gtk4::Box,
    devices: &[AudioDeviceInfo],
    is_sink: bool,
    sender: &ComponentSender<AppModel>,
) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    if devices.is_empty() {
        let empty_lbl = markup_label(
            "<span font_family='Geist Mono' font_size='8pt' foreground='#5A6042'>NO DEVICES</span>",
        );
        empty_lbl.set_margin_top(4);
        empty_lbl.set_margin_bottom(4);
        list.append(&empty_lbl);
        return;
    }
    for dev in devices {
        let row = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(8)
            .css_classes(["bt-device-row"])
            .build();
        if dev.is_default {
            row.add_css_class("audio-device-active");
        }

        row.append(&markup_label(&format!(
            "<span font_family='Geist Mono' font_size='10pt' foreground='{}'>{}</span>",
            if dev.is_default { "#999208" } else { "#5A6042" },
            dev.glyph
        )));

        let name_lbl = markup_label(&format!(
            "<span font_family='Geist Mono' font_weight='bold' font_size='8pt' foreground='{}'>{}</span>",
            if dev.is_default { "#FBFFE1" } else { "#D1CF8B" },
            gtk4::glib::markup_escape_text(&dev.description)
        ));
        name_lbl.set_hexpand(true);
        name_lbl.set_halign(gtk4::Align::Start);
        name_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        name_lbl.set_max_width_chars(28);
        row.append(&name_lbl);

        if dev.is_default {
            row.append(&markup_label("<span font_family='Geist Mono' font_weight='bold' font_size='7pt' foreground='#999208'>ACTIVE</span>"));
        } else {
            let btn = gtk4::Button::builder()
                .css_classes(["btn-action-connect"])
                .build();
            btn.set_child(Some(&markup_label("<span font_family='Geist Mono' font_weight='bold' font_size='7pt' foreground='#141C10'>USE</span>")));
            let sender_c = sender.clone();
            let name_c = dev.name.clone();
            btn.connect_clicked(move |_| {
                sender_c.input(if is_sink {
                    AppMsg::SetDefaultSink(name_c.clone())
                } else {
                    AppMsg::SetDefaultSource(name_c.clone())
                });
            });
            row.append(&btn);
        }
        list.append(&row);
    }
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

fn read_network() -> (Option<u32>, String, Option<String>) {
    let mut ssid = None;
    if let Ok(out) = std::process::Command::new("nmcli")
        .args([
            "-t",
            "-f",
            "NAME,TYPE,DEVICE",
            "connection",
            "show",
            "--active",
        ])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 2 && parts[1].contains("wireless") {
                ssid = Some(parts[0].to_string());
                break;
            }
        }
    }

    if let Ok(content) = std::fs::read_to_string("/proc/net/wireless") {
        for line in content.lines() {
            if line.contains("wlan0:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    let qual_str = parts[2].trim_end_matches('.');
                    if let Ok(qual) = qual_str.parse::<f32>() {
                        let pct = ((qual / 70.0) * 100.0).min(100.0) as u32;
                        return (Some(pct), "WIFI".to_string(), ssid);
                    }
                }
            }
        }
    }
    if let Ok(state) = std::fs::read_to_string("/sys/class/net/eno1/operstate") {
        if state.trim() == "up" {
            return (Some(100), "ETH".to_string(), None);
        }
    }
    if let Ok(state) = std::fs::read_to_string("/sys/class/net/wlan0/operstate") {
        if state.trim() == "up" {
            return (Some(100), "WIFI".to_string(), ssid);
        }
    }
    (None, "OFF".to_string(), None)
}

fn read_bluetooth() -> (bool, Option<String>) {
    let mut powered = false;
    if let Ok(out) = std::process::Command::new("bluetoothctl")
        .arg("show")
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        powered = text.contains("Powered: yes");
    }

    let mut device = None;
    if powered {
        if let Ok(out) = std::process::Command::new("bluetoothctl")
            .args(["devices", "Connected"])
            .output()
        {
            let text = String::from_utf8_lossy(&out.stdout);
            if let Some(line) = text.lines().next() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    device = Some(parts[2..].join(" "));
                }
            }
        }
    }

    (powered, device)
}

fn read_wifi_radio() -> bool {
    if let Ok(out) = std::process::Command::new("nmcli")
        .args(["radio", "wifi"])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        return text.trim() == "enabled";
    }
    true
}

fn read_wifi_networks() -> Vec<WifiNetworkInfo> {
    let saved_conns: std::collections::HashSet<String> = std::process::Command::new("nmcli")
        .args(["-t", "-f", "NAME,TYPE", "connection", "show"])
        .output()
        .ok()
        .map(|out| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter_map(|l| {
                    let mut parts = l.split(':');
                    let name = parts.next()?;
                    let conn_type = parts.next().unwrap_or("");
                    if conn_type.contains("wireless") || conn_type.contains("802-11") {
                        Some(name.to_string())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let mut networks_map: std::collections::HashMap<String, WifiNetworkInfo> =
        std::collections::HashMap::new();

    if let Ok(out) = std::process::Command::new("nmcli")
        .args([
            "-t",
            "-f",
            "IN-USE,SSID,SIGNAL,SECURITY",
            "device",
            "wifi",
            "list",
        ])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 4 {
                let in_use = parts[0].contains('*');
                let ssid = parts[1].trim().to_string();
                if ssid.is_empty() {
                    continue;
                }
                let signal = parts[2].trim().parse::<u32>().unwrap_or(0);
                let security = parts[3].trim().to_string();
                let is_known = saved_conns.contains(&ssid);

                let info = WifiNetworkInfo {
                    in_use,
                    ssid: ssid.clone(),
                    signal,
                    security,
                    is_known,
                };

                match networks_map.get_mut(&ssid) {
                    Some(existing) => {
                        if in_use || (!existing.in_use && signal > existing.signal) {
                            *existing = info;
                        }
                    }
                    None => {
                        networks_map.insert(ssid, info);
                    }
                }
            }
        }
    }

    let mut list: Vec<WifiNetworkInfo> = networks_map.into_values().collect();
    list.sort_by(|a, b| {
        b.in_use
            .cmp(&a.in_use)
            .then_with(|| b.is_known.cmp(&a.is_known))
            .then_with(|| b.signal.cmp(&a.signal))
            .then_with(|| a.ssid.cmp(&b.ssid))
    });
    list
}

fn read_bluetooth_devices() -> Vec<BtDeviceInfo> {
    let mut connected_set = std::collections::HashSet::new();
    if let Ok(out) = std::process::Command::new("bluetoothctl")
        .args(["devices", "Connected"])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 && parts[0] == "Device" {
                connected_set.insert(parts[1].to_uppercase());
            }
        }
    }

    let mut paired_set = std::collections::HashSet::new();
    if let Ok(out) = std::process::Command::new("bluetoothctl")
        .args(["devices", "Paired"])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 && parts[0] == "Device" {
                paired_set.insert(parts[1].to_uppercase());
            }
        }
    }

    let mut devices = Vec::new();
    let mut seen_macs = std::collections::HashSet::new();

    if let Ok(out) = std::process::Command::new("bluetoothctl")
        .args(["devices"])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 && parts[0] == "Device" {
                let mac = parts[1].to_uppercase();
                let name = parts[2..].join(" ");
                let connected = connected_set.contains(&mac);
                let paired = paired_set.contains(&mac) || true;
                if seen_macs.insert(mac.clone()) {
                    devices.push(BtDeviceInfo {
                        mac,
                        name,
                        connected,
                        paired,
                    });
                }
            }
        }
    }

    devices.sort_by(|a, b| {
        b.connected
            .cmp(&a.connected)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    devices
}

fn read_cpu_temp() -> Option<u32> {
    if let Ok(entries) = std::fs::read_dir("/sys/class/hwmon") {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Ok(name) = std::fs::read_to_string(path.join("name")) {
                if name.trim() == "coretemp" {
                    if let Ok(temp_str) = std::fs::read_to_string(path.join("temp1_input")) {
                        if let Ok(temp_milli) = temp_str.trim().parse::<u32>() {
                            return Some(temp_milli / 1000);
                        }
                    }
                }
            }
        }
    }
    None
}

fn read_gpu() -> (Option<u32>, Option<u32>, Option<u32>, Option<u32>) {
    if let Ok(out) = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=utilization.gpu,memory.used,memory.total,temperature.gpu",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        if let Some(line) = text.lines().next() {
            let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if parts.len() >= 4 {
                let util = parts[0].parse::<u32>().ok();
                let used = parts[1].parse::<u32>().ok();
                let total = parts[2].parse::<u32>().ok();
                let temp = parts[3].parse::<u32>().ok();
                return (util, used, total, temp);
            }
        }
    }
    (None, None, None, None)
}

fn read_disk() -> (u32, u32) {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    for disk in &disks {
        if disk.mount_point() == std::path::Path::new("/") {
            let total = disk.total_space();
            let avail = disk.available_space();
            if total > 0 {
                let used = total.saturating_sub(avail);
                let pct = ((used as f64 / total as f64) * 100.0).round() as u32;
                let free_gb = (avail / (1024 * 1024 * 1024)) as u32;
                return (pct, free_gb);
            }
        }
    }
    (0, 0)
}

fn read_media() -> Option<(String, String, Option<String>, Option<String>)> {
    let out = std::process::Command::new("playerctl")
        .args([
            "metadata",
            "--format",
            "{{status}}:::{{artist}}:::{{title}}:::{{playerName}}",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().next()?.trim();
    if line.is_empty() {
        return None;
    }
    let parts: Vec<&str> = line.split(":::").collect();
    if parts.len() < 3 {
        return None;
    }
    let status = parts[0].trim().to_string();
    if status != "Playing" && status != "Paused" {
        return None;
    }
    let artist = if parts[1].trim().is_empty() {
        None
    } else {
        Some(parts[1].trim().to_string())
    };
    let title = parts[2].trim().to_string();
    if title.is_empty() {
        return None;
    }
    let player = if parts.len() >= 4 && !parts[3].trim().is_empty() {
        Some(parts[3].trim().to_string())
    } else {
        None
    };

    Some((status, title, artist, player))
}

fn hypr_request(cmd: &[u8]) -> Option<Vec<u8>> {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/run/user/1000".to_string());
    let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").ok()?;
    let path = format!("{}/hypr/{}/.socket.sock", runtime, sig);
    let mut stream = UnixStream::connect(path).ok()?;
    stream.write_all(cmd).ok()?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn compute_ws_base(
    target_monitor: &str,
    num_monitors: usize,
    has_external: bool,
    active_ws: u32,
) -> u32 {
    let is_internal = target_monitor.starts_with("eDP");
    let is_dual = num_monitors > 1 && has_external;

    if is_dual {
        if is_internal {
            6
        } else {
            1
        }
    } else if active_ws >= 6 {
        ((active_ws - 1) / 5) * 5 + 1
    } else {
        1
    }
}

fn read_hyprland_state_fast(target_monitor: &str) -> (u32, Vec<u32>, String, u32) {
    let mut active_ws = 1u32;
    let mut occupied = Vec::new();
    let mut title = "Desktop".to_string();
    let mut num_monitors = 0;
    let mut has_external = false;
    let mut target_found = false;

    if let Some(bytes) = hypr_request(b"j/activewindow") {
        if let Ok(win) = serde_json::from_slice::<HyprWindow>(&bytes) {
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

    if let Some(bytes) = hypr_request(b"j/monitors") {
        if let Ok(monitors) = serde_json::from_slice::<Vec<HyprMonitor>>(&bytes) {
            num_monitors = monitors.len();
            has_external = monitors.iter().any(|m| !m.name.starts_with("eDP"));
            for m in &monitors {
                if m.name == target_monitor || (m.focused && target_monitor.is_empty()) {
                    target_found = true;
                    if m.active_workspace.id > 0 {
                        active_ws = m.active_workspace.id as u32;
                    }
                    break;
                }
            }
        }
    }

    if num_monitors > 0
        && !target_found
        && !target_monitor.is_empty()
        && !target_monitor.starts_with("eDP")
    {
        // External monitor was disconnected - exit this secondary bar instance
        std::process::exit(0);
    }

    if let Some(bytes) = hypr_request(b"j/workspaces") {
        if let Ok(workspaces) = serde_json::from_slice::<Vec<HyprWorkspace>>(&bytes) {
            for ws in workspaces {
                if ws.id > 0 {
                    occupied.push(ws.id as u32);
                }
            }
        }
    }

    let ws_base = compute_ws_base(target_monitor, num_monitors, has_external, active_ws);

    (active_ws, occupied, title, ws_base)
}

fn detect_monitors() -> Vec<String> {
    let mut names = Vec::new();
    if let Some(bytes) = hypr_request(b"j/monitors") {
        if let Ok(monitors) = serde_json::from_slice::<Vec<HyprMonitor>>(&bytes) {
            for m in &monitors {
                if !m.name.starts_with("eDP") {
                    names.push(m.name.clone());
                }
            }
            for m in monitors {
                if m.name.starts_with("eDP") {
                    names.push(m.name);
                }
            }
        }
    }
    if names.is_empty() {
        names.push("eDP-2".to_string());
    }
    names
}

fn run_monitor(mon_name: String) {
    let clean_mon = mon_name.replace(['-', '.'], "_");
    let app_id = format!("dev.s1re.takt.{}", clean_mon);
    let app = RelmApp::new(&app_id).with_args(vec!["takt".to_string()]);
    relm4::set_global_css(include_str!("style.css"));
    app.run::<AppModel>(mon_name);
}

fn attach_scroll_cycle<W: gtk4::prelude::WidgetExt>(
    widget: &W,
    sender: ComponentSender<AppModel>,
    last_scroll: std::sync::Arc<std::sync::atomic::AtomicI64>,
) {
    let controller = gtk4::EventControllerScroll::new(gtk4::EventControllerScrollFlags::VERTICAL);
    controller.connect_scroll(move |_, _dx, dy| {
        let now_us = gtk4::glib::monotonic_time();
        let prev_us = last_scroll.load(std::sync::atomic::Ordering::Relaxed);
        if (now_us - prev_us).abs() > 100_000 {
            if dy > 0.0 {
                last_scroll.store(now_us, std::sync::atomic::Ordering::Relaxed);
                sender.input(AppMsg::CycleDeckScreen(1));
            } else if dy < 0.0 {
                last_scroll.store(now_us, std::sync::atomic::Ordering::Relaxed);
                sender.input(AppMsg::CycleDeckScreen(-1));
            }
        }
        gtk4::glib::Propagation::Stop
    });
    widget.add_controller(controller);
}

fn get_ipc_dir() -> std::path::PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/run/user/1000".to_string());
    let dir = std::path::PathBuf::from(runtime).join("takt");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn get_monitor_socket_path(mon_name: &str) -> std::path::PathBuf {
    let clean = mon_name.replace(['-', '.', ':', '/'], "_");
    get_ipc_dir().join(format!("{}.sock", clean))
}

fn send_to_sockets(msg: &str, exclude_sock: Option<&str>) -> usize {
    let dir = get_ipc_dir();
    let mut sent = 0;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("sock") {
                if let Some(ex) = exclude_sock {
                    if path.file_name().and_then(|s| s.to_str()) == Some(ex) {
                        continue;
                    }
                }
                match UnixStream::connect(&path) {
                    Ok(mut stream) => {
                        let _ =
                            stream.set_write_timeout(Some(std::time::Duration::from_millis(100)));
                        if stream.write_all(msg.as_bytes()).is_ok() {
                            sent += 1;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                        let _ = std::fs::remove_file(&path);
                    }
                    Err(_) => {}
                }
            }
        }
    }
    sent
}

fn broadcast_ipc_screen_change(screen: DeckScreen, exclude_mon: Option<&str>) {
    let exclude_filename =
        exclude_mon.map(|m| format!("{}.sock", m.replace(['-', '.', ':', '/'], "_")));
    let msg = format!("SET {}\n", screen.as_str());
    send_to_sockets(&msg, exclude_filename.as_deref());
}

fn send_ipc_cycle_command(cmd: &str) -> Option<String> {
    let dir = get_ipc_dir();
    let mut sock_paths = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("sock") {
                sock_paths.push(path);
            }
        }
    }
    sock_paths.sort_by(|a, b| {
        let a_str = a.to_string_lossy();
        let b_str = b.to_string_lossy();
        let a_edp = a_str.contains("eDP");
        let b_edp = b_str.contains("eDP");
        match (a_edp, b_edp) {
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            _ => a.cmp(b),
        }
    });

    for path in sock_paths {
        match UnixStream::connect(&path) {
            Ok(mut stream) => {
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(300)));
                let _ = stream.set_write_timeout(Some(std::time::Duration::from_millis(300)));
                if stream.write_all(cmd.as_bytes()).is_ok() {
                    let mut reader = BufReader::new(stream);
                    let mut reply = String::new();
                    if reader.read_line(&mut reply).is_ok() && !reply.trim().is_empty() {
                        return Some(reply.trim().to_string());
                    }
                    return Some("OK".to_string());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                let _ = std::fs::remove_file(&path);
            }
            Err(_) => {}
        }
    }
    None
}

fn handle_cli_command(args: &[String]) -> Option<i32> {
    if args.len() <= 1 {
        return None;
    }

    let mut next = false;
    let mut prev = false;
    let mut screen: Option<String> = None;
    let mut status = false;
    let mut help = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-n" | "--next" => next = true,
            "-p" | "--prev" => prev = true,
            "-s" | "--screen" => {
                if i + 1 < args.len() {
                    screen = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--status" => status = true,
            "-h" | "--help" => help = true,
            _ => {}
        }
        i += 1;
    }

    if help {
        println!("takt - SOTA Wayland Tactile Telemetry Deck in Rust\n");
        println!("USAGE:");
        println!("  takt [OPTIONS]\n");
        println!("OPTIONS:");
        println!("  -n, --next              Cycle to next cockpit deck screen (01 SYS -> 02 PERF -> 03 CAL -> 04 MEDIA)");
        println!("  -p, --prev              Cycle to previous cockpit deck screen (04 MEDIA -> 03 CAL -> 02 PERF -> 01 SYS)");
        println!("  -s, --screen <ID|NAME>  Set cockpit deck screen directly (1..4, sys, perf, cal, media)");
        println!("  -m, --monitor <NAME>    Run bar on specific monitor (daemon mode)");
        println!("      --status            Query status of running takt instances");
        println!("  -h, --help              Print this help text");
        return Some(0);
    }

    if status {
        let dir = get_ipc_dir();
        let mut active = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("sock") {
                    if let Ok(mut stream) = UnixStream::connect(&path) {
                        let _ = stream.write_all(b"PING\n");
                        let mut buf = [0u8; 16];
                        if let Ok(n) = stream.read(&mut buf) {
                            if &buf[..n] == b"PONG\n" {
                                if let Some(stem) = path.file_stem() {
                                    active.push(stem.to_string_lossy().to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        if active.is_empty() {
            println!("takt: no active daemon instances running.");
            return Some(1);
        } else {
            println!("takt: active daemon instances on: {}", active.join(", "));
            return Some(0);
        }
    }

    if next || prev {
        let cmd = if next { "NEXT\n" } else { "PREV\n" };
        if let Some(res) = send_ipc_cycle_command(cmd) {
            println!("takt: {}", res);
            return Some(0);
        } else {
            eprintln!("takt: error: could not connect to running takt daemon. Is takt running?");
            return Some(1);
        }
    }

    if let Some(target) = screen {
        if let Some(deck) = DeckScreen::from_str_name(&target) {
            let cmd = format!("SCREEN {}\n", deck.as_str());
            let sent = send_to_sockets(&cmd, None);
            if sent > 0 {
                println!(
                    "takt: switched cockpit deck to {} ({} display(s) updated)",
                    deck.as_str().to_uppercase(),
                    sent
                );
                return Some(0);
            }
            std::env::set_var("TAKT_SCREEN", deck.as_str());
        } else {
            eprintln!(
                "takt: invalid screen '{}'. Valid screens: 1 (sys), 2 (perf), 3 (cal), 4 (media)",
                target
            );
            return Some(1);
        }
    }

    None
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(code) = handle_cli_command(&args) {
        std::process::exit(code);
    }

    let mut target_monitor = std::env::var("TAKT_MONITOR").ok();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--monitor" && i + 1 < args.len() {
            target_monitor = Some(args[i + 1].clone());
            i += 1;
        } else if args[i] == "--screen" && i + 1 < args.len() {
            std::env::set_var("TAKT_SCREEN", &args[i + 1]);
            i += 1;
        }
        i += 1;
    }

    if let Some(mon_name) = target_monitor {
        run_monitor(mon_name);
    } else {
        let mut monitors = detect_monitors();
        if monitors.is_empty() {
            monitors.push("eDP-2".to_string());
        }
        let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("takt"));
        for mon in monitors.iter().skip(1) {
            let _ = std::process::Command::new(&exe)
                .args(["--monitor", mon])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        }
        run_monitor(monitors[0].clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_monitor_laptop_starts_at_one() {
        // When laptop screen is the only monitor (monitor unplugged) and on workspace 1..5
        assert_eq!(compute_ws_base("eDP-2", 1, false, 1), 1);
        assert_eq!(compute_ws_base("eDP-2", 1, false, 3), 1);
        assert_eq!(compute_ws_base("eDP-2", 1, false, 5), 1);
    }

    #[test]
    fn test_single_monitor_laptop_switches_to_upper_bank() {
        // When laptop screen is the only monitor and active workspace is in 6..10
        assert_eq!(compute_ws_base("eDP-2", 1, false, 6), 6);
        assert_eq!(compute_ws_base("eDP-2", 1, false, 8), 6);
        assert_eq!(compute_ws_base("eDP-2", 1, false, 10), 6);
    }

    #[test]
    fn test_dual_monitor_modes() {
        // When both external monitor and laptop monitor are connected
        // External monitor always starts at 1
        assert_eq!(compute_ws_base("HDMI-A-1", 2, true, 1), 1);
        assert_eq!(compute_ws_base("DP-1", 2, true, 2), 1);

        // Laptop monitor in dual-monitor mode always starts at 6
        assert_eq!(compute_ws_base("eDP-2", 2, true, 6), 6);
        assert_eq!(compute_ws_base("eDP-2", 2, true, 1), 6);
    }

    #[test]
    fn test_single_external_monitor() {
        // When only external monitor is connected (e.g. laptop lid closed)
        assert_eq!(compute_ws_base("HDMI-A-1", 1, true, 1), 1);
        assert_eq!(compute_ws_base("HDMI-A-1", 1, true, 7), 6);
    }

    #[test]
    fn test_deck_screen_cycling() {
        assert_eq!(DeckScreen::Sys.next(), DeckScreen::Perf);
        assert_eq!(DeckScreen::Perf.next(), DeckScreen::Cal);
        assert_eq!(DeckScreen::Cal.next(), DeckScreen::Media);
        assert_eq!(DeckScreen::Media.next(), DeckScreen::Sys);

        assert_eq!(DeckScreen::Sys.prev(), DeckScreen::Media);
        assert_eq!(DeckScreen::Perf.prev(), DeckScreen::Sys);
        assert_eq!(DeckScreen::Cal.prev(), DeckScreen::Perf);
        assert_eq!(DeckScreen::Media.prev(), DeckScreen::Cal);
    }

    #[test]
    fn test_deck_screen_parsing() {
        assert_eq!(DeckScreen::from_str_name("1"), Some(DeckScreen::Sys));
        assert_eq!(DeckScreen::from_str_name("sys"), Some(DeckScreen::Sys));
        assert_eq!(DeckScreen::from_str_name("perf"), Some(DeckScreen::Perf));
        assert_eq!(DeckScreen::from_str_name("3"), Some(DeckScreen::Cal));
        assert_eq!(DeckScreen::from_str_name("cal"), Some(DeckScreen::Cal));
        assert_eq!(DeckScreen::from_str_name("media"), Some(DeckScreen::Media));
        assert_eq!(DeckScreen::from_str_name("invalid"), None);
    }
}
