<div align="center">

# 🧭 Compass Enhanced

**A compass bar enhanced for [Horizon](https://playhorizon.dev), with death points that guide you back to your stuff.**

![Minecraft 26.3](https://img.shields.io/badge/Minecraft-26.3-62b47a)
![Made for Horizon](https://img.shields.io/badge/made%20for-Horizon-5a8cf0)
![License: MIT](https://img.shields.io/badge/license-MIT-lightgrey)

</div>


     [Compass Bar](screenshots/screenshot.png)

## What it does

- A bar at the top of the screen shows where you are facing, in degrees, and slides smoothly as you turn.
- Compass letters (**N, NE, E, SE, S, SW, W, NW**) mark the directions, in a color you choose.
- **Death points:** every time you die, a marker stays on the bar with the distance to where you died. Dying again does not replace the old ones.
- When you reach a death point it turns into an **X** with a countdown, then goes away. You can also look at a marker and press **X** to dismiss it.
- If a death point is out of view, an arrow with its distance appears at the edge of the bar.
- No commands. Death points are not saved: they vanish when you close the game.

## Settings

Everything is in **Options → Mods → Compass Bar**:

| Setting | What it does |
| --- | --- |
| Compass bar | Turns the bar on or off |
| Background | Box behind the bar: Off, Soft or Dark |
| Scale | 75%, 100%, 125% or 150% |
| Compass letters | Color of N, NE, E...: Red, Orange, Yellow, Green, Cyan, Blue, Pink or White |
| Arrival distance | How close (in blocks) counts as "arrived" at a death point |
| The X stays | How many seconds the X stays after you arrive |
| Death points kept | How many death points are kept at once |

The dismiss key is **X** by default and can be changed in Controls.

## Install

Easiest: open the **Mods** page in the [Horizon launcher](https://playhorizon.dev), find **Compass Bar** and click install.

You need Minecraft: Java Edition and Horizon (version 26.3).

## Build from source

You need [Rust](https://rustup.rs) (1.90 or newer) and the `cargo horizon-mod` tool that comes with Horizon's mod kit (see the [Horizon mod docs](https://playhorizon.dev/docs)).

```bash
git clone https://github.com/Fernandes585/https://github.com/Fernandes585/Compass-Enhanced.git
cd YOUR_REPO

cargo horizon-mod test     # run the tests
cargo horizon-mod build    # build the mod
```

The built file is `target/horizon-mod/<id>.hmod`, where `<id>` is the `id` in `horizon-mod.toml`.
To check it the way the registry does:

```bash
cargo horizon-mod check target/horizon-mod/<id>.hmod
```

## Contributing

Issues and pull requests are welcome. Please run `cargo horizon-mod test` before opening a pull request.

## License

[MIT](LICENSE)
