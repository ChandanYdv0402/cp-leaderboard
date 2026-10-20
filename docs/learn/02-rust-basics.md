# 2. Rust basics (with examples from this project)

If you know JavaScript, Python or Java, most of this will feel familiar. Each section shows the
syntax, then where it appears in our code.

---

## Comments

```rust
// normal comment
/// doc comment for the item below (shows up in generated docs and editor hovers)
//! doc comment for the whole file/module (top of main.rs and platforms.rs)
```

---

## Variables: `let` and `let mut`

Variables are **immutable by default**. Add `mut` if you need to change them.

```rust
let cutoff = Utc::now();      // can't be reassigned
let mut rows = Vec::new();    // can be changed: rows.push(...)
```

From `main.rs`: `let mut by_user = BTreeMap::new();` is `mut` because we insert into it.

Rust usually **infers types**. You can still write them, which sometimes tells a function what to
produce:

```rust
let profiles: Vec<Profile> = api.get("profiles?...")?;   // "parse the JSON into a list of Profile"
```

---

## Basic types

| Type | Meaning | Used for |
|------|---------|----------|
| `u32` | unsigned 32-bit integer (0 and up) | `points`, `solved`, `lc_easy` |
| `i32`, `i64` | signed integers (can be negative) | ratings, timestamps |
| `f64` | 64-bit decimal number | LeetCode rating (`1963.4`) |
| `bool` | `true` / `false` | — |
| `String` | owned text you can grow/change | names, URLs |
| `&str` | borrowed view of text (read-only) | function parameters like `handle: &str` |
| `Vec<T>` | growable list of `T` | `Vec<Profile>`, `Vec<Contest>` |
| `HashSet<T>` | set of unique values | problems already counted |
| `BTreeMap<K, V>` | sorted dictionary | accounts grouped by user |
| `Option<T>` | a `T` that may be missing | `cf_rating: Option<i64>` |
| `Result<T>` | success `T` or an error | almost every function return |

Convert between number types with `as`: `(x / 100) as u32`, `rating.round() as i32`.

---

## Functions

```rust
fn endpoint(&self, resource: &str) -> String {
    format!("{}/rest/v1/{resource}", self.url)
}
```

- `fn name(param: Type, ...) -> ReturnType { ... }`
- **The last expression without a `;` is the return value.** You rarely write `return`.
- `-> Result<()>` means "returns nothing useful, but can fail". `()` is the empty "unit" value.

---

## Structs: grouping data

```rust
struct Profile {
    id: String,
    name: String,
}
```

Create one with `Profile { id: ..., name: ... }`, read a field with `profile.name`.

If a variable has the same name as a field you can write just the name: `SnapshotRow { stats, ... }`
means `SnapshotRow { stats: stats, ... }`.

`pub` makes a struct or field visible outside its file (`pub struct Stats`, `pub cf_rating`).

---

## `impl`: methods on a struct

```rust
impl Stats {
    pub fn points(&self) -> u32 {
        self.cf_points + self.lc_easy * 5 + self.lc_medium * 10 + self.lc_hard * 20
    }
}
```

- `&self` = the method reads the struct (`stats.points()`).
- `&mut self` = the method may change it.
- No `self` = an associated function, called on the type: `Supabase::from_env()`, `Stats::default()`.

---

## `#[derive(...)]`: free code from attributes

```rust
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Stats { ... }
```

This auto-writes code so that `Stats` can be turned into JSON and back, copied with `.clone()`, and
created all-zero with `Stats::default()`. These capabilities are called **traits** (like interfaces).

---

## `if`, loops

```rust
if !user_accounts.iter().any(|a| a.platform == "leetcode") { ... }   // `!` = not

for account in accounts { ... }        // takes each item (accounts is used up afterwards)
for profile in &profiles { ... }       // borrows each item (profiles still usable after)

continue;   // skip to next iteration
```

There's also `while cond { }` and `loop { }` (forever, until `break`), but we don't use them.

---

## `match`: a better `switch`

```rust
let result = match account.platform.as_str() {
    "codeforces" => platforms::fetch_codeforces(client, &account.handle, &mut stats),
    "leetcode"   => platforms::fetch_leetcode(client, &account.handle, &mut stats),
    _            => continue,          // `_` = anything else
};
```

- `match` is an expression, so it can produce a value (`let result = match ...`).
- It must be **exhaustive**: every possible value must be handled, which is why `_` is there.
- It can unpack values: `Ok(found) => ...`, `Err(error) => ...`, `Some("Easy") => ...`.

---

## Closures (inline functions)

```rust
|row| row.stats                          // like JS: row => row.stats
|account| account.platform == "leetcode"
|x| Contest { platform: "leetcode", ... }
```

Closures are passed to iterator methods, `Option`/`Result` helpers, etc.

---

## Iterators: `.map`, `.filter`, `.collect`

Like JS array methods, but lazy until `.collect()`:

```rust
list.as_array().into_iter().flatten()
    .filter(|x| x["phase"] == "BEFORE")   // keep matching items
    .map(|x| Contest { ... })             // transform each
    .collect()                            // build the final Vec
```

Other ones in the code: `.any(...)` (like `.some`), `.next()` (first item), `.extend(...)` (append many).

---

## Macros: names ending in `!`

Macros generate code at compile time. The ones we use:

| Macro | Does |
|-------|------|
| `println!("{} pts", x)` | print a line to stdout |
| `eprintln!(...)` | print to stderr (errors/logs) |
| `format!("...")` | build a `String` (like a template literal) |
| `json!({ "a": 1 })` | write a JSON value inline |
| `bail!("msg")` | return an error right now |

Format strings: `{}` = next argument, `{name}` = variable `name` in scope.

---

## Modules and `use`

```rust
mod platforms;                    // include src/platforms.rs as module `platforms`
use platforms::{Contest, Stats};  // import names so we can write `Stats` instead of `platforms::Stats`
platforms::fetch_codeforces(...)  // or call with the full path
```

`::` is the path separator (like `.` in imports elsewhere): `std::collections::HashSet`.

---

## Generics (a peek)

```rust
fn get<T: serde::de::DeserializeOwned>(&self, resource: &str) -> Result<T>
```

"`get` works for any type `T`, as long as `T` can be built from JSON." The caller chooses `T`:
`let rows: Vec<Profile> = api.get(...)?;`. One function serves every table.

---

## Strings, quickly

```rust
"hello"                 // &'static str: text baked into the program
String::from("hi")      // owned String
"hi".to_string()        // same thing
s.as_str()              // borrow a String as &str
r#"raw "quotes" ok"#    // raw string: no escaping needed (used for the GraphQL query)
```

Next: [03-ownership-and-borrowing.md](03-ownership-and-borrowing.md), the one truly new idea in Rust.
