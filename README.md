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

Then either pick the server in the game's launcher, or run the game directly:

```sh
cargo run -- --server 127.0.0.1:7777
```

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

---

## Gamemodes

A gamemode is a configurable rule set the server ticks. The default is
`free_flight` (no objectives). The rules come from the map, so a gamemode can be
configured per map.

To add one:

1. Implement the `GameMode` trait in `src/gamemode.rs`.
2. Register it in `create()` and `registered_ids()`.

That is the whole extension point — see `FreeFlight` for a minimal example.

---

## Protocol

Newline-delimited, tab-separated text over TCP (see `src/protocol.rs`), which
makes it easy to debug by hand. The protocol file is **shared verbatim** with the
game — keep both copies in sync and bump `PROTOCOL_VERSION` on any change.

Messages:

- Client → server: `JOIN`, `STATE`, `LEAVE`
- Server → client: `WELCOME`, `SNAPSHOT`, `PLAYER_LEFT`, `ERROR`

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
