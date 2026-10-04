# OpenThunder Server

The dedicated server for [**OpenThunder**](https://github.com/Wynplusplus/openthunder),
an unofficial War Thunder Air-RB-style flight prototype.

It is a small, **dependency-free** TCP server (Rust `std` only) that:

- accepts clients and keeps the authoritative player roster,
- relays aircraft state as snapshots (20 Hz),
- runs a configurable **gamemode**, chosen and configured by the **map**.

Movement is currently *client-authoritative* (each client simulates its own
aircraft and streams its state; the server relays it). That is enough for
free-flight/co-op and leaves room to move simulation server-side later.

---

## Build & run

```sh
cargo run --release -- --bind 0.0.0.0:7777 --map training
```

| Option | Default | Meaning |
| --- | --- | --- |
| `--bind <addr>` | `0.0.0.0:7777` | Address to listen on |
| `--maps <dir>` | `maps` | Where to find `*.map` files |
| `--map <id\|name>` | first map | Which map to run |
| `--planes <dir>` | `planes` | Where the plane directories live |
| `--crew <file>` | `crew.conf` | Pilot crew config shipped to clients |

Then either pick the server in the game's launcher, or run the game directly:

```sh
cargo run -- --server 127.0.0.1:7777
```

---

## Planes

Planes live in `planes/`, one **directory per plane**, each holding that plane's
assets:

```
planes/
  f4u-4-corsair/
    plane.conf          # flight model + model + armament
  bf-109-g6/
    plane.conf
  spitfire-f-mk-ixc/
    plane.conf
```

`plane.conf` is a `key = value` file with `[gun N]` sections. It holds the whole
**flight model** (mass, wing, engine power, lift/drag, handling, limits), the
**model** parameters (length, wing chord, tail span, colour) and the
**armament**. Copy any shipped plane as a template.

**To add a plane:** create `planes/<id>/` with a `plane.conf` in it — nothing
else to do. The server ships every plane's config to clients when they connect,
so clients load them automatically (no client rebuild). Any other files (notes,
future model meshes, ...) can live in the same directory.

Example:

```text
name = Test Fighter
mass = 3000
wing_span = 9.4
max_power = 1500000
body_color = 0.5 0.5 0.52

[gun 0]
name = Test Cannon
caliber_mm = 20
rounds_per_second = 10
muzzle_velocity = 850
damage = 11
ammo = 300
muzzle = -2.5 -0.05 -1.0
muzzle = 2.5 -0.05 -1.0
```

---

## Pilot crew

`crew.conf` sets the pilot model used by **every client on this server**, and is
shipped to clients when they connect (the `CREW` message). Lower `g_tolerance`
for a harsher, more realistic server; raise it for a forgiving one.

```text
g_tolerance = 6.5           # positive g held before the pilot blacks out
negative_g_tolerance = -3.0 # negative g before the pilot reds out
blackout_rate = 0.18        # blackout gained per g above tolerance, per second
recovery_rate = 0.4         # vision recovered per second once the g comes off
stamina_drain = 0.012       # stamina lost per g above 3, per second
stamina_recovery = 0.1      # stamina recovered per second below 3 g
```

Any key may be omitted (clients keep their built-in default). War Thunder's
maxed "G-tolerance" crew skill is about 6.9 g. If the file is missing the server
ships its built-in defaults.

---

## Maps

A map is a small `name = value` file under `maps/`. It picks the gamemode and
supplies that gamemode's **rules**:

```text
name = Training Islands
gamemode = free_flight
max_players = 16
```

Adding a map is just dropping a new `.map` file in `maps/`. Different maps can
run the same gamemode with different rules, or entirely different gamemodes.

Two ship:

- `training.map` — **Training Islands**, `free_flight`.
- `pacific_islands.map` — **Pacific Islands**, `team_deathmatch` (island chain,
  first team to 50 kills, 15-minute rounds). The client builds matching island
  terrain for it.

---

## Gamemodes

A gamemode is a configurable rule set the server ticks. The rules come from the
map, so a gamemode can be configured per map. Two ship:

- **`free_flight`** — no objectives, just fly. Rule: `max_players`.
- **`team_deathmatch`** — two teams race to a kill target. Rules: `max_players`,
  `score_limit`, `time_limit`. The server balances players across the teams,
  credits kills (to the last attacker) when a client reports its death, and
  restarts the round automatically when a team hits the target or time runs out.

To add one:

1. Implement the `GameMode` trait in `src/gamemode.rs`.
2. Register it in `create()` and `registered_ids()`.

That is the whole extension point — see `FreeFlight` for a minimal example and
`TeamDeathmatch` for a scoring one.

---

## Protocol

Newline-delimited, tab-separated text over TCP (see `src/protocol.rs`), which
makes it easy to debug by hand. The protocol file is **shared verbatim** with the
game — keep both copies in sync and bump `PROTOCOL_VERSION` on any change.

Messages:

- Client → server: `JOIN`, `STATE`, `LEAVE`, `HIT`, `DEATH`
- Server → client: `WELCOME`, `PLANES`, `CREW`, `SNAPSHOT`, `PLAYER_LEFT`,
  `MATCH`, `KILL`, `HIT`, `ERROR`

`WELCOME` carries the player's team, `SNAPSHOT` carries each player's team,
kills and deaths, `MATCH` carries the team scores and round timer, and `KILL`
carries a kill-feed line (killer and victim names).

---

## Tests

```sh
cargo test
```

Includes an end-to-end test that starts the real server binary, connects a raw
client, joins, sends state and receives a snapshot.

---

## License

GPL-3.0-or-later. This is an **unofficial fan project**, not affiliated with or
endorsed by Gaijin Entertainment. "War Thunder" is a trademark of Gaijin
Entertainment, referenced only descriptively.
