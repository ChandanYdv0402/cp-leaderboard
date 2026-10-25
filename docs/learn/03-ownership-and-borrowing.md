# 3. Ownership and borrowing

This is the part of Rust that's genuinely new. It's how Rust manages memory **without a garbage
collector** and without manual `free()`, and it's checked entirely at compile time.

## The three rules

1. Every value has exactly **one owner** (a variable).
2. When the owner goes out of scope (the `}` ends), the value is freed.
3. You can **lend** a value out: either
   - any number of **shared borrows** `&T` (read-only), **or**
   - exactly **one mutable borrow** `&mut T` (read-write),
   but never both at the same time.

## Moving

Assigning or passing an owned value **moves** it. The old variable can't be used anymore.

```rust
let stats = previous_stats(api, &profile.id)?;
rows.push(SnapshotRow { stats, /* ... */ });   // `stats` MOVES into the row
// println!("{}", stats.points());             // ❌ compile error: value was moved
```

That's why in `main.rs` we compute `points` and `solved` **before** moving `stats` into the row:

```rust
rows.push(SnapshotRow {
    points: stats.points(),   // uses stats (borrow) …
    solved: stats.solved(),
    stats,                    // … then moves it, last
});
```

Small number types like `u32`, `i64`, `f64`, `bool` are **`Copy`**: they're copied instead of moved,
so no problem there. `String`, `Vec` and our structs are moved.

## Borrowing with `&`

Instead of giving a value away, lend it:

```rust
fn fetch_users(api: &Supabase, client: &reqwest::blocking::Client) -> Result<()>
```

`fetch_users` *borrows* `api` and `client`. When it returns, `main` still owns them and can pass
them to `fetch_contests` next:

```rust
fetch_users(&api, &client)?;      // lend
fetch_contests(&api, &client)?;   // lend again, still ours
```

## Mutable borrowing with `&mut`

```rust
pub fn fetch_codeforces(c: &Client, handle: &str, s: &mut Stats) -> Result<()>
```

`fetch_codeforces` may **change** the caller's `Stats`:

```rust
let mut stats = previous_stats(...)?;                                  // must be `mut` to lend mutably
platforms::fetch_codeforces(client, &account.handle, &mut stats)?;     // fills in cf_* fields
// stats now has the new numbers
```

Rule 3 means while `fetch_codeforces` holds `&mut stats`, nothing else can read or write `stats`.
That makes whole classes of bugs (two things editing the same data) impossible.

## `String` vs `&str`

- `String`: owns its text (on the heap). Use for struct fields that must keep the text.
- `&str`: borrows text owned by someone else. Use for function parameters that only read it.

```rust
struct Account { handle: String }               // the struct owns its handle
fn fetch_leetcode(c: &Client, user: &str, ...)  // the function just reads one
fetch_leetcode(client, &account.handle, ...)    // &String automatically works as &str
```

## `.clone()`: explicit copies

When you need a second owned copy, say so:

```rust
by_user.entry(account.user_id.clone()).or_default().push(account);
```

`account` is moved into the list, but we also need its `user_id` as the map key, so we clone the
id first. Cloning costs a memory copy, which is why Rust makes you write it.

## Lifetimes, in one paragraph

A borrow must never outlive the thing it borrows. The compiler tracks this with *lifetimes*. You
mostly don't write them; the only one in this project is `&'static str`. `'static` means "lives for
the whole program", which is true of text written directly in the code like `"codeforces"`.

## When the compiler complains

Errors like *"borrow of moved value"* or *"cannot borrow as mutable"* are these rules at work.
Usual fixes:
- pass `&x` instead of `x`,
- use the value before moving it,
- add `mut` to the variable,
- or `.clone()` it if you truly need two owners.
