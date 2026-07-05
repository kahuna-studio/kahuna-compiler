# Identifiers

- snake_case for variables, enum variants, function and method names
- Pascal_Snake_Case for types, type aliases, enums, classes
- ALL_CAPS for built-in values

# Variables

    x = true
    y double = 42.0

- All variables are immutable

# Built-in values

    PI
    AUDIO_RATE
    CONTROL_RATE

# Control structures

if/else clauses are expressions (they return a value). Thus, the else is mandatory.

    x = if y == 42 then "equals 42" else "something else"

if or else branches may contain blocks (which are optional sequences of assignments followed by a mandatory expression)

    x = if y == 42 then
      message = "equals 42"
      message
    else
      "something else"

# Comments

    // This is a comment
    let a = b // This as well
    (* This...
      is a multi-line comment
      an needs to be closed *)
    (* Also when (* nested *) they have to be closed *)

