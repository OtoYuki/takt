# takt (такт)

> **SOTA Industrial Micrographic Wayland Desktop Telemetry Deck in Rust**  
> Inspired by Teenage Engineering, Dieter Rams, and Nothing OS.

`takt` is a bespoke desktop shell engineered specifically for [[hyprland]]. Unlike off-the-shelf bars configured through static JSON and clunky GTK3 CSS, `takt` gives you full, uncompromising code ownership: reactive state, zero-overhead memory-mapped telemetry, sub-pixel vector rendering, and modern GTK4 / Wayland layer-shell composition.

---

## Technical Architecture

```
                       ┌──────────────────────────────┐
                       │          takt (Rust)         │
                       ├──────────────────────────────┤
                       │  Relm4 Component (Elm / MVU) │
                       │    AppModel / AppMsg / view! │
                       └──────────────┬───────────────┘
                                      │
              ┌───────────────────────┼───────────────────────┐
              ▼                       ▼                       ▼
    gtk4-layer-shell          sysinfo / sysfs          hyprland-rs IPC
    (Wayland Layer Surface)   (Direct Telemetry)       (Unix Sockets)
    - Anchored: Top/Left/Right - Memory-mapped metrics  - Workspace events
    - Auto exclusive zone     - Zero subprocesses      - Window titles
    - GTK4 GPU Scene Graph    - Async Tokio ticker     - Active dispatch
```

### Stack
* **Language**: Rust (2021 edition)
* **Framework**: [`relm4`](https://github.com/Relm4/Relm4) (v0.9)
* **Compositor Protocol**: [`gtk4-layer-shell`](https://github.com/wmww/gtk4-layer-shell) (v0.4)
* **Telemetry**: `sysinfo` (v0.33), `chrono`
* **Async Runtime**: `tokio` (v1)
* **Typography**: `Freigeist` (Display Chrono), `Goga` (Micro-labels), `Geist Mono` (Tabular Telemetry), `Geist Pixel Circle` (Workspace Presets)
* **Color Palette**: Solid Void Earthy Industrial Swatch (`#141C10`, `#999208`, `#D8A664`, `#5A6042`, `#D1CF8B`, `#FBFFE1`)

---

## Quickstart

### Build & Run
```bash
cd ~/dev/takt

# Check & Lint
cargo fmt --check
cargo clippy

# Run the live deck
cargo run
```

---

## Development Roadmap

- [x] **Milestone 1: Foundations**
  - [x] GTK4 Layer-Shell initialization anchored to Top/Left/Right with auto exclusive zone.
  - [x] Relm4 reactive component loop (`AppModel` + `AppMsg`).
  - [x] In-process background telemetry worker (`sysinfo` CPU & Memory ticker).
  - [x] Embedded GTK4 CSS theme with 6-color swatch palette, precision hairline dividers, and inset bevels.
- [ ] **Milestone 2: Hyprland Realtime IPC**
  - [ ] Integrate `hyprland-rs` event listener thread.
  - [ ] Dynamic workspace buttons reflecting active/occupied workspaces without polling.
  - [ ] Active window title streaming over Hyprland Unix socket.
- [ ] **Milestone 3: Audio & MPRIS Integration**
  - [ ] Connect to PipeWire / PulseAudio via async crate (`pulsectl-rs` or `libpulse-binding`).
  - [ ] MPRIS D-Bus listener via `zbus` for playback states, track titles, and tape animations.
- [ ] **Milestone 4: Interactive Popover Decks**
  - [ ] Click Battery: Slide out precision battery health & power telemetry HUD.
  - [ ] Click Audio: Integrated 5-band EQ mixer / per-app slider deck.
  - [ ] Click Clock: Teenage Engineering style calendar & world timer.
- [ ] **Milestone 5: Custom Vector Canvas & Physics**
  - [ ] Custom `gtk4::DrawingArea` with Cairo / GSK render nodes.
  - [ ] Live audio oscilloscope / VU meter wave rendering.
  - [ ] Rolling mechanical odometer number counters for CPU and RAM digits.
