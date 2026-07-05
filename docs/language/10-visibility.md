# Visibility and Package Exports

Visibility is controlled at **two levels**:

1. **File level** — using the `public` modifier
2. **Package level** — using `package.toml` exports

---

## File-Level Visibility

By default, every declaration in a file is **private to that file**.

Only declarations marked with `public` are visible to other files **within the same package**.

    public module Wavetable_Osc
      ...

    wavetable_sample = (...) ->
      ...

- `Wavetable_Osc` is visible to other files in the package.
- `wavetable_sample` is only visible inside this file.

This keeps helper functions, internal math, and implementation details local and hidden.

---

## Package-Level Visibility

Being `public` inside a package does **not** automatically expose a declaration to other packages.

To make a declaration available outside the package, it must be listed in `package.toml`.

### Example `package.toml`

    [exports]
    foo/bar/Baz
    foo/bar/FooBar as MyFooBar

Only the listed declarations can be imported by other packages.

---

## Summary of Rules

| Location       | Without `public` | With `public` | With `public` and exported in `package.toml` |
|----------------|------------------|----------------|----------------------------------------------|
| Same file      | Yes              | Yes            | Yes                                          |
| Same package   | No               | Yes            | Yes                                          |
| Other packages | No               | No             | Yes                                          |

---

## Why This Model?

- Keeps implementation details hidden by default
- Makes the public API of a package **explicit and intentional**
- Avoids complex visibility rules
