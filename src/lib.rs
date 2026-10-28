//! Compass Bar for Horizon.
//!
//! A bar at the top of the screen shows where you face, in degrees with
//! N/E/S/W marks, sliding as you turn. Every time you die a death point is
//! kept on the bar: all of them stay (dying on the way back does not replace
//! the older one). A point goes away when you reach it (it turns into an X
//! for a few seconds) or when you look at it and press X. Death points are
//! never saved: they vanish when the game closes. No commands.
use horizon_mod_sdk::{
    Mod, Registration, chat,
    game::{self},
    input::{self, Event},
    render::{self, Point, QuadShape, Shape, TextShape},
    storage,
    ui::{self, ButtonWidget, TextWidget, Widget},
};

const SETTINGS_KEY: &str = "settings";

/// Degrees shown on each side of the centre.
const HALF_SPAN: f64 = 75.0;
const MAX_BAR_W: f32 = 280.0;
// Layout at scale 100%, in GUI pixels from the top of the screen.
const PANEL_Y: f32 = 6.0;
const PANEL_H: f32 = 44.0;
/// How many degrees off the centre still counts as "looking at it".
const DISMISS_TOL: f64 = 8.0;

// Colours (0xRRGGBB for text, 0xAARRGGBB for shapes). Mostly white with a
// dark outline, so the bar reads on a bright sky and on a dark cave alike.
const TEXT_COLOR: u32 = 0xF2F2F2;
const NORTH_COLOR: u32 = 0xFF7A7A;
const OUTLINE: u32 = 0x8000_0000;
/// The very faint shadow behind each tick of the scale.
const TICK_OUTLINE: u32 = 0x2800_0000;
/// Room added at both sides of the background box, so the outermost labels
/// (up to three digits wide) stay inside it.
const BOX_PAD: f32 = 11.0;
const MARK_RED: u32 = 0xFFEE_4444;

const ARRIVE_OPTS: [f64; 4] = [2.0, 3.0, 5.0, 8.0];
const X_OPTS: [u64; 4] = [5, 10, 20, 30];
const MAX_OPTS: [usize; 4] = [3, 5, 8, 12];
/// 0 off, 1 soft, 2 dark.
const BG_OPTS: [u8; 3] = [0, 1, 2];
const SCALE_OPTS: [u32; 4] = [75, 100, 125, 150];
/// Choices for the colour of the compass letters (N, NE, E ...), as
/// (name, 0xRRGGBB). The first one is the default: a medium red.
const CARD_COLORS: [(&str, u32); 8] = [
    ("Red", 0xE04848),
    ("Orange", 0xF0903A),
    ("Yellow", 0xF0D040),
    ("Green", 0x55D060),
    ("Cyan", 0x4CC8E0),
    ("Blue", 0x5A8CF0),
    ("Pink", 0xF070B0),
    ("White", 0xFFFFFF),
];
/// Indices into `CARD_COLORS`, so the button can cycle through them.
const CARD_OPTS: [u8; 8] = [0, 1, 2, 3, 4, 5, 6, 7];

// The settings page (Options > Mods > Compass Bar).
const ID_S_INFO: u32 = 20;
const ID_S_BAR: u32 = 21;
const ID_S_ARRIVE: u32 = 22;
const ID_S_XSECS: u32 = 23;
const ID_S_MAX: u32 = 24;
const ID_S_BG: u32 = 25;
const ID_S_SCALE: u32 = 26;
const ID_S_CARD: u32 = 27;

// --------------------------------------------------------------- settings

#[derive(Clone, Debug, PartialEq)]
struct Settings {
    /// The bar itself on or off.
    bar: bool,
    /// Blocks from a death point that count as "arrived".
    arrive: f64,
    /// Seconds the X stays after arriving.
    x_secs: u64,
    /// Death points kept at once; the oldest goes first.
    max_points: usize,
    /// The box behind the bar: 0 none, 1 soft, 2 dark.
    background: u8,
    /// Size of the bar in percent. The game's font can't be resized, so this
    /// changes the bar's width and spacing, and the marks' size.
    scale_pct: u32,
    /// Colour of the compass letters: an index into `CARD_COLORS`.
    cardinal: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            bar: true,
            arrive: 3.0,
            x_secs: 10,
            max_points: 8,
            background: 0,
            scale_pct: 100,
            cardinal: 0,
        }
    }
}

fn encode_settings(s: &Settings) -> String {
    format!(
        "bar={}\narrive={}\nx_secs={}\nmax={}\nbg={}\nscale={}\ncardinal={}\n",
        u8::from(s.bar),
        s.arrive as u32,
        s.x_secs,
        s.max_points,
        s.background,
        s.scale_pct,
        s.cardinal
    )
}

fn decode_settings(text: &str) -> Settings {
    let mut s = Settings::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "bar" => s.bar = value == "1",
            "arrive" => {
                if let Ok(n) = value.parse::<f64>() {
                    if ARRIVE_OPTS.contains(&n) {
                        s.arrive = n;
                    }
                }
            }
            "x_secs" => {
                if let Ok(n) = value.parse::<u64>() {
                    if X_OPTS.contains(&n) {
                        s.x_secs = n;
                    }
                }
            }
            "max" => {
                if let Ok(n) = value.parse::<usize>() {
                    if MAX_OPTS.contains(&n) {
                        s.max_points = n;
                    }
                }
            }
            "bg" => {
                if let Ok(n) = value.parse::<u8>() {
                    if BG_OPTS.contains(&n) {
                        s.background = n;
                    }
                }
            }
            "scale" => {
                if let Ok(n) = value.parse::<u32>() {
                    if SCALE_OPTS.contains(&n) {
                        s.scale_pct = n;
                    }
                }
            }
            "cardinal" => {
                if let Ok(n) = value.parse::<u8>() {
                    if CARD_OPTS.contains(&n) {
                        s.cardinal = n;
                    }
                }
            }
            _ => {}
        }
    }
    s
}

/// The next value in a list, wrapping around (the first if `cur` isn't in it).
fn next_in<T: Copy + PartialEq>(opts: &[T], cur: T) -> T {
    let i = opts
        .iter()
        .position(|o| *o == cur)
        .map_or(0, |i| (i + 1) % opts.len());
    opts[i]
}

fn background_name(bg: u8) -> &'static str {
    match bg {
        0 => "Off",
        1 => "Soft",
        _ => "Dark",
    }
}

fn card_color(idx: u8) -> u32 {
    CARD_COLORS
        .get(usize::from(idx))
        .map_or(CARD_COLORS[0].1, |c| c.1)
}

fn card_color_name(idx: u8) -> &'static str {
    CARD_COLORS
        .get(usize::from(idx))
        .map_or(CARD_COLORS[0].0, |c| c.0)
}

/// The 0xAARRGGBB of the box behind the bar, or None for no box.
fn background_color(bg: u8) -> Option<u32> {
    match bg {
        0 => None,
        1 => Some(0x6000_0000),
        _ => Some(0xB000_0000),
    }
}

// ------------------------------------------------------------------ maths

fn wrap180(a: f64) -> f64 {
    (a + 180.0).rem_euclid(360.0) - 180.0
}

/// Compass bearing from a vanilla yaw: 0 is north, 90 east, 180 south, 270
/// west. (Vanilla yaw 0 faces south, 90 west, 180 north, -90 east.)
fn bearing_of_yaw(yaw: f64) -> f64 {
    (yaw + 180.0).rem_euclid(360.0)
}

/// Compass bearing of the direction (dx, dz) from the player to a point.
fn bearing_to(dx: f64, dz: f64) -> f64 {
    bearing_of_yaw((-dx).atan2(dz).to_degrees())
}

/// Of the candidates `(index, bearing)`, the one closest to where you face,
/// if it is within `tol` degrees. Returns its index.
fn pick_aimed(cands: &[(usize, f64)], facing: f64, tol: f64) -> Option<usize> {
    cands
        .iter()
        .map(|(i, b)| (*i, wrap180(b - facing).abs()))
        .filter(|(_, d)| *d <= tol)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

fn card_name(deg: i32) -> Option<&'static str> {
    match deg {
        0 => Some("N"),
        45 => Some("NE"),
        90 => Some("E"),
        135 => Some("SE"),
        180 => Some("S"),
        225 => Some("SW"),
        270 => Some("W"),
        315 => Some("NW"),
        _ => None,
    }
}

/// Scales a 0xRRGGBB colour towards black (the bar fades at its edges).
fn dim(color: u32, f: f64) -> u32 {
    let f = f.clamp(0.0, 1.0);
    let part = |shift: u32| (f64::from((color >> shift) & 0xFF) * f) as u32;
    (part(16) << 16) | (part(8) << 8) | part(0)
}

fn distance_text(blocks: f64) -> String {
    if blocks >= 1000.0 {
        format!("{:.1}km", blocks / 1000.0)
    } else {
        format!("{:.0}m", blocks)
    }
}

// -------------------------------------------------------------- smoothing

/// The game hands a mod the player's state once per tick (20 Hz) but the
/// camera moves every frame. This keeps the last two samples and estimates
/// where the camera is now, so the bar doesn't stutter as you turn.
struct Smooth<const N: usize> {
    cur: [f64; N],
    prev: [f64; N],
    /// Seconds since `cur` arrived.
    since: f64,
    /// Seconds between the last two samples.
    interval: f64,
    ready: bool,
}

impl<const N: usize> Smooth<N> {
    fn new() -> Self {
        Smooth {
            cur: [0.0; N],
            prev: [0.0; N],
            since: 0.0,
            interval: 0.05,
            ready: false,
        }
    }

    fn reset(&mut self) {
        self.ready = false;
    }

    fn push(&mut self, sample: [f64; N], dt: f64) {
        if !self.ready {
            self.cur = sample;
            self.prev = sample;
            self.since = 0.0;
            self.ready = true;
            return;
        }
        self.since += dt;
        if sample != self.cur {
            self.interval = self.since.clamp(0.03, 0.12);
            self.prev = self.cur;
            self.cur = sample;
            self.since = 0.0;
        }
    }

    /// Positions: glide from the previous tick to this one.
    fn lerp(&self) -> [f64; N] {
        let t = (self.since / self.interval).clamp(0.0, 1.0);
        let mut out = self.cur;
        for i in 0..N {
            out[i] = self.prev[i] + (self.cur[i] - self.prev[i]) * t;
        }
        out
    }

    /// Rotation: the mouse turns the view in real time, so carry the last
    /// movement forward, and fade it out when no new sample comes.
    fn extrapolate(&self) -> [f64; N] {
        let ahead = self.since.min(self.interval);
        let fade = if self.since <= self.interval {
            1.0
        } else {
            (2.0 - self.since / self.interval).clamp(0.0, 1.0)
        };
        let mut out = self.cur;
        for i in 0..N {
            let vel = (self.cur[i] - self.prev[i]) / self.interval;
            out[i] = self.cur[i] + vel * ahead * fade;
        }
        out
    }
}

// ---------------------------------------------------------------- drawing

#[derive(Clone, Debug, PartialEq)]
struct Death {
    x: f64,
    y: f64,
    z: f64,
    dim: String,
    /// The game tick you arrived at, once you did.
    arrived_at: Option<u64>,
}

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn rect(out: &mut Vec<Shape>, x0: f32, y0: f32, x1: f32, y1: f32, color: u32) {
    out.push(Shape::Quad(QuadShape {
        corners: (point(x0, y0), point(x1, y0), point(x1, y1), point(x0, y1)),
        color,
    }));
}

fn text(out: &mut Vec<Shape>, s: &str, x: f32, y: f32, color: u32, centered: bool) {
    out.push(Shape::Text(TextShape {
        text: s.to_string(),
        at: point(x, y),
        color,
        centered,
        shadow: true,
    }));
}

fn diamond(out: &mut Vec<Shape>, x: f32, y: f32, r: f32, color: u32) {
    out.push(Shape::Quad(QuadShape {
        corners: (
            point(x, y - r),
            point(x + r * 0.75, y),
            point(x, y + r),
            point(x - r * 0.75, y),
        ),
        color,
    }));
}

/// Every shape of the bar for one frame. `facing` is the compass bearing of
/// the camera, `me` the player's position.
fn build_bar(
    w: f32,
    facing: f64,
    me: (f64, f64, f64),
    dimension: &str,
    deaths: &[Death],
    now: u64,
    s: &Settings,
) -> Vec<Shape> {
    let mut out: Vec<Shape> = Vec::new();

    // `k` stretches the bar sideways; `kv` is the height and the size of the
    // marks (the text keeps the game's size, so it never shrinks below 85%).
    let k = s.scale_pct as f32 / 100.0;
    let kv = k.max(0.85);
    let panel_h = PANEL_H * kv;
    let num_y = PANEL_Y + 4.0 * kv;
    let tri_y = PANEL_Y + 14.0 * kv;
    let tick_y = PANEL_Y + 22.0 * kv;
    let label_y = PANEL_Y + 32.0 * kv;
    let (tick_short, tick_long) = (5.0 * kv, 8.0 * kv);

    let bar_w = (w - 20.0).min(MAX_BAR_W * k).max(80.0);
    let cx = w / 2.0;
    let (x0, x1) = (cx - bar_w / 2.0, cx + bar_w / 2.0);
    let ppd = f64::from(bar_w) / (2.0 * HALF_SPAN);

    if let Some(color) = background_color(s.background) {
        let (bx0, bx1) = ((x0 - BOX_PAD).max(0.0), (x1 + BOX_PAD).min(w));
        rect(&mut out, bx0, PANEL_Y, bx1, PANEL_Y + panel_h, color);
    }

    // Ticks and labels every 15 degrees; the cardinal ones get letters.
    let first = ((facing - HALF_SPAN) / 15.0).ceil() as i32;
    let last = ((facing + HALF_SPAN) / 15.0).floor() as i32;
    for step in first..=last {
        let theta = f64::from(step) * 15.0;
        let d = theta - facing;
        let x = (f64::from(cx) + d * ppd).round() as f32;
        // Fades to a soft grey at the edges, never to black: with no box
        // behind the bar, dark text would vanish against the sky.
        let fade = 1.0 - (d.abs() / HALF_SPAN).powi(2) * 0.4;
        let deg = (theta.rem_euclid(360.0).round() as i32) % 360;
        let cardinal = card_name(deg);
        let tick_len = if cardinal.is_some() { tick_long } else { tick_short };
        // A very faint outline first, then the tick itself.
        rect(&mut out, x - 1.5, tick_y - 1.0, x + 1.5, tick_y + tick_len + 1.0, TICK_OUTLINE);
        rect(
            &mut out,
            x - 0.5,
            tick_y,
            x + 0.5,
            tick_y + tick_len,
            0xFF00_0000 | dim(0xFFFFFF, fade),
        );
        let (label, color) = match cardinal {
            Some(c) => (c.to_string(), card_color(s.cardinal)),
            None => (deg.to_string(), TEXT_COLOR),
        };
        text(&mut out, &label, x, label_y, dim(color, fade), true);
    }

    // The centre: the number you face and a marker under it.
    let number = (facing.round() as i32) % 360;
    text(&mut out, &number.to_string(), cx, num_y, 0xFFFFFF, true);
    out.push(Shape::Quad(QuadShape {
        corners: (
            point(cx - 5.5, tri_y - 1.0),
            point(cx + 5.5, tri_y - 1.0),
            point(cx, tri_y + 7.0),
            point(cx, tri_y + 7.0),
        ),
        color: OUTLINE,
    }));
    out.push(Shape::Quad(QuadShape {
        corners: (
            point(cx - 4.0, tri_y),
            point(cx + 4.0, tri_y),
            point(cx, tri_y + 5.0),
            point(cx, tri_y + 5.0),
        ),
        color: 0xFFFF_FFFF,
    }));

    // Death points: on the bar when in view, an arrow at the edge when not.
    let mut rows = [0u8; 2];
    for d in deaths.iter().filter(|d| d.dim == dimension) {
        let (dx, dy, dz) = (d.x - me.0, d.y - me.1, d.z - me.2);
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        let rel = wrap180(bearing_to(dx, dz) - facing);
        let label = match d.arrived_at {
            Some(t) => {
                let left = (t + s.x_secs * 20).saturating_sub(now);
                format!("{}s", left.div_ceil(20))
            }
            None => distance_text(dist),
        };

        if rel.abs() <= HALF_SPAN {
            let x = (f64::from(cx) + rel * ppd).round() as f32;
            let (line_top, line_bottom) = (PANEL_Y + 13.0 * kv, PANEL_Y + panel_h - 3.0);
            rect(&mut out, x - 1.5, line_top - 1.0, x + 1.5, line_bottom + 1.0, OUTLINE);
            rect(&mut out, x - 0.75, line_top, x + 0.75, line_bottom, MARK_RED);
            if d.arrived_at.is_some() {
                text(&mut out, "X", x, PANEL_Y + 4.0 * kv, NORTH_COLOR, true);
            } else {
                diamond(&mut out, x, PANEL_Y + 9.0 * kv, 6.5 * kv, 0xC000_0000);
                diamond(&mut out, x, PANEL_Y + 9.0 * kv, 5.0 * kv, MARK_RED);
            }
            text(&mut out, &label, x, PANEL_Y + panel_h + 2.0, TEXT_COLOR, true);
        } else {
            let side = usize::from(rel > 0.0);
            if rows[side] < 3 {
                let y = num_y + f32::from(rows[side]) * 9.0;
                if side == 0 {
                    text(&mut out, &format!("< {label}"), x0 + 24.0, y, TEXT_COLOR, true);
                } else {
                    text(&mut out, &format!("{label} >"), x1 - 24.0, y, TEXT_COLOR, true);
                }
                rows[side] += 1;
            }
        }
    }
    out
}

// -------------------------------------------------------------------- mod

struct CompassBar {
    settings: Settings,
    deaths: Vec<Death>,
    /// True from the tick you die until you are alive again, so one death
    /// makes one point.
    dead: bool,
    /// The game's tick counter, from the last tick.
    now: u64,
    dim: String,
    pos: Smooth<3>,
    rot: Smooth<1>,
    drawn: bool,
}

impl CompassBar {
    fn save_settings(&self) {
        let _ = storage::set(SETTINGS_KEY, encode_settings(&self.settings).as_bytes());
    }

    fn trim(&mut self) {
        while self.deaths.len() > self.settings.max_points {
            self.deaths.remove(0);
        }
    }

    /// The key: dismisses the death point you are looking at.
    fn dismiss(&mut self) {
        let Some(p) = game::get_player() else { return };
        let facing = bearing_of_yaw(f64::from(p.yaw));
        let cands: Vec<(usize, f64)> = self
            .deaths
            .iter()
            .enumerate()
            .filter(|(_, d)| d.dim == p.dimension)
            .map(|(i, d)| (i, bearing_to(d.x - p.x, d.z - p.z)))
            .collect();
        match pick_aimed(&cands, facing, DISMISS_TOL) {
            Some(i) => {
                self.deaths.remove(i);
                chat::show("Death point dismissed.");
            }
            None => chat::show("Look at a death point on the bar, then press the key."),
        }
    }

    fn push_settings(&self) {
        let s = &self.settings;
        let widgets = vec![
            Widget::Text(TextWidget {
                id: ID_S_INFO,
                text: "Death points are not saved: they vanish when you close the game."
                    .to_string(),
                color: 0,
            }),
            Widget::Button(ButtonWidget {
                id: ID_S_BAR,
                label: format!("Compass bar: {}", if s.bar { "ON" } else { "OFF" }),
            }),
            Widget::Button(ButtonWidget {
                id: ID_S_BG,
                label: format!("Background: {}", background_name(s.background)),
            }),
            Widget::Button(ButtonWidget {
                id: ID_S_SCALE,
                label: format!("Scale: {}%", s.scale_pct),
            }),
            Widget::Button(ButtonWidget {
                id: ID_S_CARD,
                label: format!("Compass letters: {}", card_color_name(s.cardinal)),
            }),
            Widget::Button(ButtonWidget {
                id: ID_S_ARRIVE,
                label: format!("Arrival distance: {} blocks", s.arrive as u32),
            }),
            Widget::Button(ButtonWidget {
                id: ID_S_XSECS,
                label: format!("The X stays: {} s", s.x_secs),
            }),
            Widget::Button(ButtonWidget {
                id: ID_S_MAX,
                label: format!("Death points kept: {}", s.max_points),
            }),
        ];
        ui::set_settings(&widgets);
    }
}

impl Mod for CompassBar {
    fn init() -> Result<(Self, Registration), String> {
        input::register_key(
            "dismiss",
            "Dismiss the death point you look at",
            "key.keyboard.x",
        )?;
        let settings = storage::get(SETTINGS_KEY)
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .map(|text| decode_settings(&text))
            .unwrap_or_default();
        let state = CompassBar {
            settings,
            deaths: Vec::new(),
            dead: false,
            now: 0,
            dim: String::new(),
            pos: Smooth::new(),
            rot: Smooth::new(),
            drawn: false,
        };
        state.push_settings();
        Ok((state, Registration::new().on_tick().on_frame()))
    }

    fn tick(&mut self, tick: u64) {
        self.now = tick;

        if let Some(p) = game::get_player() {
            if p.health <= 0.0 {
                if !self.dead {
                    self.dead = true;
                    self.deaths.push(Death {
                        x: p.x,
                        y: p.y,
                        z: p.z,
                        dim: p.dimension.clone(),
                        arrived_at: None,
                    });
                    self.trim();
                    chat::show("Death point added to the compass bar.");
                }
            } else {
                self.dead = false;
                let radius = self.settings.arrive;
                for d in self
                    .deaths
                    .iter_mut()
                    .filter(|d| d.dim == p.dimension && d.arrived_at.is_none())
                {
                    let (dx, dy, dz) = (d.x - p.x, d.y - p.y, d.z - p.z);
                    if (dx * dx + dy * dy + dz * dz).sqrt() <= radius {
                        d.arrived_at = Some(tick);
                    }
                }
            }
        }

        // An X that has stayed long enough goes away.
        let hold = self.settings.x_secs * 20;
        self.deaths
            .retain(|d| d.arrived_at.map_or(true, |t| tick < t + hold));

        for event in input::poll() {
            if let Event::KeyDown(key) = event {
                if key == "dismiss" {
                    self.dismiss();
                }
            }
        }
    }

    fn ui_select(&mut self, id: u32, _row: u32) {
        match id {
            ID_S_BAR => self.settings.bar = !self.settings.bar,
            ID_S_BG => self.settings.background = next_in(&BG_OPTS, self.settings.background),
            ID_S_SCALE => self.settings.scale_pct = next_in(&SCALE_OPTS, self.settings.scale_pct),
            ID_S_CARD => self.settings.cardinal = next_in(&CARD_OPTS, self.settings.cardinal),
            ID_S_ARRIVE => self.settings.arrive = next_in(&ARRIVE_OPTS, self.settings.arrive),
            ID_S_XSECS => self.settings.x_secs = next_in(&X_OPTS, self.settings.x_secs),
            ID_S_MAX => {
                self.settings.max_points = next_in(&MAX_OPTS, self.settings.max_points);
                self.trim();
            }
            _ => return,
        }
        self.save_settings();
        self.push_settings();
    }

    fn ui_settings_opened(&mut self) {
        self.push_settings();
    }

    fn ui_closed(&mut self) {}

    fn frame(&mut self, seconds: f32) {
        let Some(p) = game::get_player() else {
            if self.drawn {
                render::set_hud(&[]);
                self.drawn = false;
            }
            self.pos.reset();
            self.rot.reset();
            return;
        };
        let dt = f64::from(seconds).min(0.25);
        if p.dimension != self.dim {
            self.dim = p.dimension.clone();
            self.pos.reset();
            self.rot.reset();
        }
        if self.pos.ready {
            let c = self.pos.cur;
            if (c[0] - p.x).abs() + (c[1] - p.y).abs() + (c[2] - p.z).abs() > 24.0 {
                self.pos.reset(); // teleport
            }
        }
        self.pos.push([p.x, p.y, p.z], dt);
        let yaw = if self.rot.ready {
            self.rot.cur[0] + wrap180(f64::from(p.yaw) - self.rot.cur[0])
        } else {
            f64::from(p.yaw)
        };
        self.rot.push([yaw], dt);

        if !self.settings.bar {
            if self.drawn {
                render::set_hud(&[]);
                self.drawn = false;
            }
            return;
        }

        let [px, py, pz] = self.pos.lerp();
        let [yaw] = self.rot.extrapolate();
        let (w, _h) = render::gui_size();
        let shapes = build_bar(
            w,
            bearing_of_yaw(yaw),
            (px, py, pz),
            &p.dimension,
            &self.deaths,
            self.now,
            &self.settings,
        );
        render::set_hud(&shapes);
        self.drawn = true;
    }
}

horizon_mod_sdk::export_mod!(CompassBar);

#[cfg(test)]
mod tests {
    use super::*;

    const OVERWORLD: &str = "minecraft:overworld";

    fn death(x: f64, z: f64, dim: &str) -> Death {
        Death {
            x,
            y: 64.0,
            z,
            dim: dim.to_string(),
            arrived_at: None,
        }
    }

    #[test]
    fn yaw_becomes_a_compass_bearing() {
        // vanilla: yaw 0 faces south, 90 west, 180 north, -90 east
        assert!(wrap180(bearing_of_yaw(180.0) - 0.0).abs() < 1e-6);
        assert!(wrap180(bearing_of_yaw(-90.0) - 90.0).abs() < 1e-6);
        assert!(wrap180(bearing_of_yaw(0.0) - 180.0).abs() < 1e-6);
        assert!(wrap180(bearing_of_yaw(90.0) - 270.0).abs() < 1e-6);
    }

    #[test]
    fn a_direction_becomes_a_compass_bearing() {
        // north is -Z, east is +X, south is +Z, west is -X
        assert!(wrap180(bearing_to(0.0, -1.0) - 0.0).abs() < 1e-6);
        assert!(wrap180(bearing_to(1.0, 0.0) - 90.0).abs() < 1e-6);
        assert!(wrap180(bearing_to(0.0, 1.0) - 180.0).abs() < 1e-6);
        assert!(wrap180(bearing_to(-1.0, 0.0) - 270.0).abs() < 1e-6);
    }

    #[test]
    fn facing_a_point_matches_its_bearing() {
        // a player standing at the origin who faces yaw 180 looks north
        let facing = bearing_of_yaw(180.0);
        assert!(wrap180(bearing_to(0.0, -20.0) - facing).abs() < 1e-6);
    }

    #[test]
    fn the_point_you_look_at_is_the_one_dismissed() {
        let cands = [(0, 10.0), (1, 94.0), (2, 200.0)];
        assert_eq!(pick_aimed(&cands, 90.0, DISMISS_TOL), Some(1));
        assert_eq!(pick_aimed(&cands, 150.0, DISMISS_TOL), None);
    }

    #[test]
    fn the_closest_aim_wins_when_two_are_in_range() {
        let cands = [(3, 86.0), (7, 91.0)];
        assert_eq!(pick_aimed(&cands, 90.0, DISMISS_TOL), Some(7));
    }

    #[test]
    fn bearings_wrap_around_north() {
        let cands = [(0, 359.0)];
        assert_eq!(pick_aimed(&cands, 2.0, DISMISS_TOL), Some(0));
    }

    #[test]
    fn settings_round_trip() {
        let s = Settings {
            bar: false,
            arrive: 5.0,
            x_secs: 20,
            max_points: 12,
            background: 2,
            scale_pct: 125,
            cardinal: 5,
        };
        assert_eq!(decode_settings(&encode_settings(&s)), s);
        assert_eq!(decode_settings("junk"), Settings::default());
        // a value that isn't one of the choices is ignored
        assert_eq!(decode_settings("arrive=99\n").arrive, 3.0);
        assert_eq!(decode_settings("scale=7\n").scale_pct, 100);
        assert_eq!(decode_settings("cardinal=99\n").cardinal, 0);
    }

    #[test]
    fn the_compass_letters_use_the_chosen_colour() {
        let me = (0.0, 64.0, 0.0);
        let north_color = |s: &Settings| {
            build_bar(480.0, 0.0, me, OVERWORLD, &[], 0, s)
                .iter()
                .find_map(|sh| match sh {
                    Shape::Text(t) if t.text == "N" => Some(t.color),
                    _ => None,
                })
        };
        // facing north the N is at the centre, so it is not faded
        assert_eq!(north_color(&Settings::default()), Some(CARD_COLORS[0].1));
        let white = Settings {
            cardinal: 7,
            ..Settings::default()
        };
        assert_eq!(north_color(&white), Some(0xFFFFFF));
    }

    #[test]
    fn the_box_is_wider_than_the_scale() {
        // w = 480: the scale spans 100..380, the box must reach past both ends
        let on = Settings {
            background: 1,
            ..Settings::default()
        };
        let shapes = build_bar(480.0, 0.0, (0.0, 64.0, 0.0), OVERWORLD, &[], 0, &on);
        match &shapes[0] {
            Shape::Quad(q) => {
                assert!(q.corners.0.x < 100.0);
                assert!(q.corners.1.x > 380.0);
            }
            _ => panic!("the box is drawn first"),
        }
    }

    #[test]
    fn the_background_is_off_by_default() {
        assert_eq!(Settings::default().background, 0);
        assert_eq!(background_color(0), None);
    }

    #[test]
    fn choices_cycle() {
        assert_eq!(next_in(&X_OPTS, 5), 10);
        assert_eq!(next_in(&X_OPTS, 30), 5);
        assert_eq!(next_in(&X_OPTS, 7), 5);
        assert_eq!(next_in(&SCALE_OPTS, 150), 75);
    }

    #[test]
    fn the_background_adds_one_box() {
        let me = (0.0, 64.0, 0.0);
        let off = Settings::default();
        let on = Settings {
            background: 1,
            ..Settings::default()
        };
        let a = build_bar(480.0, 0.0, me, OVERWORLD, &[], 0, &off).len();
        let b = build_bar(480.0, 0.0, me, OVERWORLD, &[], 0, &on).len();
        assert_eq!(b, a + 1);
    }

    #[test]
    fn a_bigger_scale_widens_the_bar() {
        // the outermost tick sits further from the centre at 150% than at 100%
        let far = |shapes: &[Shape]| {
            shapes
                .iter()
                .filter_map(|sh| match sh {
                    Shape::Quad(q) => Some((q.corners.0.x - 240.0).abs()),
                    _ => None,
                })
                .fold(0.0f32, f32::max)
        };
        let me = (0.0, 64.0, 0.0);
        let normal = Settings::default();
        let big = Settings {
            scale_pct: 150,
            ..Settings::default()
        };
        let a = far(&build_bar(480.0, 0.0, me, OVERWORLD, &[], 0, &normal));
        let b = far(&build_bar(480.0, 0.0, me, OVERWORLD, &[], 0, &big));
        assert!(b > a);
    }

    #[test]
    fn deaths_in_other_dimensions_are_not_drawn() {
        let s = Settings::default();
        let me = (0.0, 64.0, 0.0);
        let none = build_bar(480.0, 0.0, me, OVERWORLD, &[], 0, &s).len();
        let nether = [death(0.0, -20.0, "minecraft:the_nether")];
        let with = build_bar(480.0, 0.0, me, OVERWORLD, &nether, 0, &s).len();
        assert_eq!(none, with);
    }

    #[test]
    fn a_death_in_view_adds_a_marker() {
        let s = Settings::default();
        let me = (0.0, 64.0, 0.0);
        let none = build_bar(480.0, 0.0, me, OVERWORLD, &[], 0, &s).len();
        let north = [death(0.0, -20.0, OVERWORLD)];
        let with = build_bar(480.0, 0.0, me, OVERWORLD, &north, 0, &s).len();
        assert!(with > none);
    }

    #[test]
    fn two_deaths_are_both_drawn() {
        let s = Settings::default();
        let me = (0.0, 64.0, 0.0);
        let one = [death(0.0, -20.0, OVERWORLD)];
        let two = [death(0.0, -20.0, OVERWORLD), death(20.0, 0.0, OVERWORLD)];
        let a = build_bar(480.0, 0.0, me, OVERWORLD, &one, 0, &s).len();
        let b = build_bar(480.0, 0.0, me, OVERWORLD, &two, 0, &s).len();
        assert!(b > a);
    }

    #[test]
    fn an_arrived_point_counts_down_in_seconds() {
        let s = Settings::default(); // the X stays 10 s = 200 ticks
        let mut d = death(0.0, -1.0, OVERWORLD);
        d.arrived_at = Some(100);
        let shapes = build_bar(480.0, 0.0, (0.0, 64.0, 0.0), OVERWORLD, &[d], 100, &s);
        let has = shapes.iter().any(|sh| match sh {
            Shape::Text(t) => t.text == "10s",
            _ => false,
        });
        assert!(has);
    }
}
