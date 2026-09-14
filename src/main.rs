use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use relm4::prelude::*;

struct AppModel {
    cpu_usage: f32,
    mem_usage: f32,
    time_str: String,
    date_str: String,
}

#[derive(Debug)]
enum AppMsg {
    Tick,
    UpdateTelemetry { cpu: f32, mem: f32 },
}

#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    view! {
        #[root]
        gtk4::ApplicationWindow {
            set_title: Some("takt"),
            set_default_size: (2560, 48),
            add_css_class: "takt-bar",

            gtk4::CenterBox {
                set_orientation: gtk4::Orientation::Horizontal,
                set_hexpand: true,
                add_css_class: "takt-rail",

                #[wrap(Some)]
                set_start_widget = &gtk4::Box {
                    set_orientation: gtk4::Orientation::Horizontal,
                    set_valign: gtk4::Align::Center,
                    add_css_class: "tactile-pod",
                    add_css_class: "pod-workspaces",

                    gtk4::Button {
                        set_label: "01",
                        add_css_class: "ws-btn",
                        add_css_class: "active",
                    },
                    gtk4::Button {
                        set_label: "02",
                        add_css_class: "ws-btn",
                    },
                    gtk4::Button {
                        set_label: "03",
                        add_css_class: "ws-btn",
                    },
                },

                #[wrap(Some)]
                set_center_widget = &gtk4::Box {
                    set_orientation: gtk4::Orientation::Horizontal,
                    set_valign: gtk4::Align::Center,
                    add_css_class: "tactile-pod",
                    add_css_class: "pod-clock",

                    gtk4::Label {
                        set_markup: &format!(
                            "<span font_family='Freigeist' font_weight='800' font_size='11pt' foreground='#FBFFE1'>{}</span> <span font_family='Geist Mono' foreground='#5A6042'>::</span> <span font_family='Goga' font_weight='bold' font_size='10pt' foreground='#D1CF8B'>{}</span>",
                            model.time_str,
                            model.date_str
                        ),
                        #[watch]
                        set_markup: &format!(
                            "<span font_family='Freigeist' font_weight='800' font_size='11pt' foreground='#FBFFE1'>{}</span> <span font_family='Geist Mono' foreground='#5A6042'>::</span> <span font_family='Goga' font_weight='bold' font_size='10pt' foreground='#D1CF8B'>{}</span>",
                            model.time_str,
                            model.date_str
                        ),
                    },
                },

                #[wrap(Some)]
                set_end_widget = &gtk4::Box {
                    set_orientation: gtk4::Orientation::Horizontal,
                    set_valign: gtk4::Align::Center,
                    add_css_class: "tactile-pod",
                    add_css_class: "pod-hardware",

                    // CPU
                    gtk4::Label {
                        add_css_class: "instrument-cell",
                        set_markup: &format!(
                            "<span font_family='Goga' font_size='9pt' font_weight='bold' foreground='#5A6042'>CPU</span> <span font_family='Geist Mono' font_weight='bold' foreground='#999208'>{:>2.0}%</span>",
                            model.cpu_usage
                        ),
                        #[watch]
                        set_markup: &format!(
                            "<span font_family='Goga' font_size='9pt' font_weight='bold' foreground='#5A6042'>CPU</span> <span font_family='Geist Mono' font_weight='bold' foreground='#999208'>{:>2.0}%</span>",
                            model.cpu_usage
                        ),
                    },
                    // RAM
                    gtk4::Label {
                        add_css_class: "instrument-cell",
                        set_markup: &format!(
                            "<span font_family='Goga' font_size='9pt' font_weight='bold' foreground='#5A6042'>RAM</span> <span font_family='Geist Mono' font_weight='bold' foreground='#D1CF8B'>{:>2.0}%</span>",
                            model.mem_usage
                        ),
                        #[watch]
                        set_markup: &format!(
                            "<span font_family='Goga' font_size='9pt' font_weight='bold' foreground='#5A6042'>RAM</span> <span font_family='Geist Mono' font_weight='bold' foreground='#D1CF8B'>{:>2.0}%</span>",
                            model.mem_usage
                        ),
                    },
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        // Initialize Wayland Layer Shell
        root.init_layer_shell();
        root.set_layer(Layer::Top);
        root.set_anchor(Edge::Top, true);
        root.set_anchor(Edge::Left, true);
        root.set_anchor(Edge::Right, true);
        root.auto_exclusive_zone_enable();

        let mut sys = sysinfo::System::new_all();
        sys.refresh_memory();
        let mem = (sys.used_memory() as f32 / sys.total_memory() as f32) * 100.0;

        let now = chrono::Local::now();
        let model = AppModel {
            cpu_usage: 0.0,
            mem_usage: mem,
            time_str: now.format("%H:%M").to_string(),
            date_str: now.format("%a %d %b").to_string(),
        };

        let widgets = view_output!();

        // Spawn background ticker
        let sender_clone = sender.clone();
        std::thread::spawn(move || {
            let mut sys = sysinfo::System::new_all();
            loop {
                std::thread::sleep(std::time::Duration::from_secs(2));
                sys.refresh_cpu_all();
                sys.refresh_memory();

                let cpu = sys.global_cpu_usage();
                let mem = (sys.used_memory() as f32 / sys.total_memory() as f32) * 100.0;
                sender_clone.input(AppMsg::UpdateTelemetry { cpu, mem });
                sender_clone.input(AppMsg::Tick);
            }
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            AppMsg::Tick => {
                let now = chrono::Local::now();
                self.time_str = now.format("%H:%M").to_string();
                self.date_str = now.format("%a %d %b").to_string();
            }
            AppMsg::UpdateTelemetry { cpu, mem } => {
                self.cpu_usage = cpu;
                self.mem_usage = mem;
            }
        }
    }
}

fn main() {
    let app = RelmApp::new("dev.s1re.takt");
    relm4::set_global_css(include_str!("style.css"));
    app.run::<AppModel>(());
}
