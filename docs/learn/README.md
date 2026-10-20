# Learning this codebase

A guided path for someone who has never read Rust. Every example comes from this project.

## Reading order

| # | File | What you'll learn | Time |
|---|------|-------------------|------|
| 1 | [01-big-picture.md](01-big-picture.md) | How the pieces fit: GitHub Actions → Rust → Supabase → website | 5 min |
| 2 | [02-rust-basics.md](02-rust-basics.md) | Variables, types, functions, structs, `impl`, `match`, loops, closures, macros | 20 min |
| 3 | [03-ownership-and-borrowing.md](03-ownership-and-borrowing.md) | Rust's big idea: `String` vs `&str`, `&` vs `&mut`, moves, `.clone()` | 15 min |
| 4 | [04-option-result-errors.md](04-option-result-errors.md) | No `null`, no exceptions: `Option`, `Result`, `?`, `anyhow` | 15 min |
| 5 | [05-cargo-and-crates.md](05-cargo-and-crates.md) | `Cargo.toml`, the libraries we use, everyday commands | 10 min |
| 6 | [06-platforms-rs.md](06-platforms-rs.md) | Walkthrough of `src/platforms.rs` (talking to Codeforces & LeetCode) | 15 min |
| 7 | [07-main-rs.md](07-main-rs.md) | Walkthrough of `src/main.rs` (talking to Supabase) | 15 min |
| 8 | [08-exercises.md](08-exercises.md) | Small changes to try, with hints | as long as you like |

The source files themselves are commented line by line: open `src/platforms.rs` and `src/main.rs`
next to the walkthroughs.

## Setup to follow along

```bash
cargo check        # type-check only (fast, no network, no Supabase needed)
cargo build        # compile
cargo run          # compile + run (needs SUPABASE_URL and SUPABASE_SERVICE_ROLE_KEY set)
```

If `cargo` isn't found in a new terminal, run `. "$HOME/.cargo/env"` first.
