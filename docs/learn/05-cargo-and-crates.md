# 5. Cargo and crates

**Cargo** is Rust's build tool and package manager (npm + webpack in one).
A **crate** is a Rust package/library.

## Files

| File | What it is |
|------|------------|
| `Cargo.toml` | Project settings and dependencies (you edit this) |
| `Cargo.lock` | Exact versions of every dependency (generated; commit it, don't edit) |
| `src/main.rs` | Program entry point (`fn main`) |
| `src/platforms.rs` | A module, included by `mod platforms;` in main.rs |
| `target/` | Build output (ignored by git; safe to delete, `cargo clean` does it) |

## Commands

```bash
cargo check            # type-check only; fastest way to see errors
cargo build            # debug build → target/debug/cp-leaderboard
cargo build --release  # optimized build → target/release/cp-leaderboard
cargo run              # build + run (debug)
cargo run --release    # what GitHub Actions runs
cargo fmt              # auto-format code (needs: rustup component add rustfmt)
cargo clippy           # extra lint suggestions (needs: rustup component add clippy)
cargo add <crate>      # add a dependency to Cargo.toml
cargo doc --open       # generate HTML docs from your /// comments, plus all dependencies
```

## Our dependencies

| Crate | Why we use it | Where |
|-------|---------------|-------|
| `reqwest` | HTTP requests (to Codeforces, LeetCode, Supabase). The `blocking` feature gives simple synchronous calls. | both files |
| `serde` | The standard way to convert Rust structs ↔ data formats. `#[derive(Serialize, Deserialize)]` comes from here. | structs in both files |
| `serde_json` | JSON support for serde; `Value` (any JSON) and the `json!` macro | both files |
| `anyhow` | One error type for everything + `?`, `.context()`, `bail!` | both files |
| `chrono` | Dates and times (`Utc::now()`, timestamps → dates) | `main.rs` |
| `toml` | Parses TOML; left over from the old `users.toml` version, currently unused | — |

**Features** (`features = ["blocking", "json"]`) switch on optional parts of a crate.
`default-features = false` turns off the defaults. We do that on `reqwest` to use `rustls`
(pure-Rust HTTPS) instead of the system OpenSSL.

## Release vs debug

Debug builds compile fast and run slower. `--release` compiles slower and runs fast. The fetcher
mostly waits on the network (and deliberately sleeps 2.1s between Codeforces calls), so either is fine.

## Where the docs live

Every crate's docs are at `https://docs.rs/<crate-name>`, for example https://docs.rs/reqwest.
The Rust Book (https://doc.rust-lang.org/book/) is the standard free introduction.
