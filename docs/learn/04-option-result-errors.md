# 4. `Option`, `Result` and error handling

Rust has **no `null`** and **no exceptions**. Instead, two types make "missing" and "failed"
explicit, and the compiler won't let you ignore them.

## `Option<T>`: maybe there's a value

```rust
enum Option<T> {
    Some(T),
    None,
}
```

(`enum` = a type that is exactly one of several variants.)

In our code:

```rust
pub cf_rating: Option<i64>,              // unrated users have None
s.cf_rating = info[0]["rating"].as_i64(); // as_i64() returns Option<i64>
```

### Getting the value out

| Code | Meaning |
|------|---------|
| `x.unwrap_or(800)` | value, or 800 if `None` |
| `x.unwrap_or_default()` | value, or the type's default (`0`, `""`, empty list) |
| `x.map(\|r\| r.round() as i32)` | transform the value if present, keep `None` otherwise |
| `x.map_or_else(default_fn, \|v\| ...)` | handle both cases, produce one value |
| `match x { Some(v) => …, None => … }` | handle both explicitly |
| `if let Some(v) = x { … }` | run code only when present |
| `x.unwrap()` | value, or **crash** if `None` (we avoid it) |

Real example from `main.rs`:

```rust
lc_rating: stats.lc_rating.map(|rating| rating.round() as i32),
```

## `Result<T, E>`: success or error

```rust
enum Result<T, E> {
    Ok(T),     // success, with a value
    Err(E),    // failure, with an error
}
```

Anything that can fail returns a `Result`: network calls, JSON parsing, reading env vars.

## The `?` operator

The most important symbol in this codebase.

```rust
let v: Value = c.get(url).send()?.json()?;
```

`?` means: **if this is `Err`, return that error from the current function immediately; if it's
`Ok(x)`, give me `x`.** Without `?` you'd write:

```rust
let response = match c.get(url).send() {
    Ok(r) => r,
    Err(e) => return Err(e.into()),
};
```

`?` only works inside functions that themselves return `Result` (or `Option`), which is why nearly
every function here ends in `-> Result<...>`. Even `main` does: if it returns `Err`, the program
exits with an error and GitHub Actions marks the run as failed.

`?` also works on `Option` inside functions returning `Option`. In `main.rs` we convert an
`Option` into a `Result` first with `.context(...)`:

```rust
start: DateTime::from_timestamp(contest.start, 0)   // Option<DateTime>
    .context("contest has invalid start timestamp")?, // None → error with this message
```

## `anyhow`: errors made easy

Different libraries return different error types. `anyhow::Result<T>` is a `Result` whose error can
hold *any* error, so `?` works with all of them. Its helpers:

```rust
use anyhow::{bail, Context, Result};

env::var("SUPABASE_URL").context("SUPABASE_URL is not set")?;  // replace a vague error with a clear one
bail!("leetcode user {user} not found");                       // return an error right now
```

## Handling an error instead of returning it

Sometimes failure is OK and we just log it. `match` on the `Result`:

```rust
match platforms::codeforces_contests(client) {
    Ok(found) => contests.extend(found),
    Err(error) => eprintln!("codeforces contests: {error}"),   // log, keep going
}
```

In `fetch_users`, one person's broken handle is logged and skipped, so it doesn't stop everyone
else's snapshot. That's a deliberate choice you can see in the code.

## Collecting `Result`s

```rust
let rows: Result<Vec<ContestRow>> = contests.into_iter().map(|c| Ok(ContestRow { … })).collect();
```

Collecting many `Result`s into one `Result<Vec<_>>` gives `Ok(all items)` or the **first** `Err`.
