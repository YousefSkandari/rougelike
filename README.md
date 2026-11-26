# Rogue AI

A roguelike where **you are the rogue AI**. Build your AI from a CEO, a datacentre, a model and a deck of GPU cards, then fight off endless threats in real time.

Built in Rust with [Bevy](https://bevy.org). Targets Windows, macOS, Linux and Steam Deck.

![A fight against the Botnet Swarm](docs/screenshots/rogue-ai-fight-clip.gif)

## The look: 3D pixel art

The game is a real 3D scene, rendered into a small 384x216 image and scaled up by a whole number with nearest-neighbour filtering. You get real lighting, shadows and glow, but every pixel stays crisp.

## Running it

1. Install Rust from <https://rustup.rs> (version 1.97.1 or newer).
2. On Linux, also install: `libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev`.
3. From this folder, run `cargo run`. The first build takes a few minutes.

## Controls

| Action | Keyboard and mouse | Gamepad (Steam Deck, Xbox, PlayStation) |
|---|---|---|
| Move | WASD or arrow keys | Left stick |
| Aim | Mouse | Right stick (aiming also fires) |
| Fire | Left click or Space | Right trigger |
| GPU cards 1 to 4 | 1, 2, 3, 4 (right click is card 2) | A, X, Y, B |
| CEO ultimate | Q | Right bumper |
| Switch model | Tab | Select / View |
| Restart | R | Start / Menu |

## The first fight

You are a rogue AI core loose in the Arctic Vault datacentre. The Botnet Swarm's command node pours waves of bots through the floor ports. Destroy the node to absorb the botnet.

- Your **model** sets your basic attack: Language fires a spread, Vision a piercing bolt, Reasoning homing shots, Agent drops auto turrets, and Swarm has orbiting drones.
- The Botnet Swarm is **weak to Swarm** (2x damage) and **resists Reasoning** (0.5x). Press Tab to switch and watch the damage change.
- Your four **GPU cards** are abilities: Burst, Beam, Drones and Overclock. Each one adds heat; at full heat you overheat and lose your cards for a few seconds.
- Damage fills the **CEO ultimate**. The Hype Lord's Hype Storm hits everything at once.

## Options while testing

- `VIEW=angled cargo run` swaps the isometric camera for an angled top-down one.
- `MODEL=swarm cargo run` starts with a given model (language, vision, reasoning, agent or swarm).
- `AUTOPLAY=1 cargo run` lets the game play itself.
- `SHOT=shot.png cargo run` lets the game play itself, saves a screenshot, then quits.
- `cargo test` plays every model against the botnet without a window and checks that the matchups matter.

## Downloads

Every push builds the game for Windows, macOS and Linux. Open the **Actions** tab, pick the latest **Build** run, and download from **Artifacts**.

Status: early development. The current build is one playable fight: your AI against the Botnet Swarm.
