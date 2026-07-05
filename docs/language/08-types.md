# Types

The type system defines primitive values, collections, algebraic types, records, streams, and type constructors.  
All values have a type, either declared explicitly or inferred by the compiler.

---

## Primitive Types

### Boolean
- `bool` with values `true` or `false`.

### Numeric
- `int` – signed integer
- `float` – single-precision floating point
- `double` – double-precision floating point

Numeric types may optionally declare a **range**:
- Example: `int(0..127)`, `float(-1..0.1)`
- Ranges are optional in general but **mandatory** for numeric types of module port streams.

### Frequency
- `frequency = 440.0Hz`
- `frequency = 440.0`   # Float representation
- `frequency = A4`      # Musical note representation
- `frequency = 69`      # Integer representation (Midi value)

### Time
- `time = 200ms`
- `time = 200`          # Integer representation (in ms)
- `time = 1beat`        # Musical timing representation

### Unit
- `unit` is a type with a single value `()`.

---

## Option Type
Represents a value that may or may not be present.
- `option<T>`
- Constructors:  
  `some(x)` – a value of type `T`  
  `none` – absence of a value

Example:

    find_note = (name string) option<Note> ->
      if name == 'A4' then some(A4) else none

---

## Collections

### Lists
- **Dynamic list:** `T[]`    
  Example: `my_dynamic_list int[] = [100, 200, 300]`
- **Fixed-size array:** `T[N]`    
  Example: `my_fixed_size_list int[3] = [100, 200, 300]`    
  List literals are inferred to fixed size lists: `my_fixed_size_list = [100, 200, 300] // int[3]`

### Tuples
- `(T1, T2, ...)`  
  Example: `x (int, string) = (42, 'makes sense')`
  Example: `x = (42, 'makes sense')`

### Maps
- `{K: V}`  
  Example: `x {string: int} = {'foo': 1, 'bar': 2}`
  Example: `x = {'foo': 1, 'bar': 2}`

---

## Enums

Enums define a type with variants. Each variant may optionally carry a single payload type.

    enum Midi_Message
      note_on(Note)
      note_off(Note)
      control_change(Control)
      something_else
      something_with_mode_values(string, float)

Enums are written in `Pascal_Snake_Case`, variants in `snake_case`.

---

## Records

Immutable data structures with named fields and methods.  
Methods may return modified copies instead of mutating state.

    record Envelope
      attack float
      decay float
      sustain float
      release float

      public with_attack (a float) -> Envelope =
        this.{ attack = a }

Record names are written in `Pascal_Snake_Case`.

---

## Ranges

    range = -1.0..1.0

---

## Type Aliases

Aliases can introduce shorthand or unions:

    type Numeric = int | float | double

Type aliases are written in `Pascal_Snake_Case`.

---

## Generics

Types and functions may be parameterized:

    id <T> (T) -> T = x -> x
    list_of <T> (T) -> T[] = x -> [x]

Generic parameters can include **constraints**:

    add <T: Numeric> (T, T) -> T

---

## Streams

Streams are time-varying values. Declared with a symbol prefixing the **variable name** (not the type):

- **Signals (`~`)**  
  Provide a value at every sample. Can "run" at audio or control rate (default is audio rate).

- **Parameter streams (`@`)**  
  Hold the last known value. Must declare a default value. Can "run" at audio or control rate (default is control rate).

- **Event streams (`$`)**  
  Discrete events. Accessing them as a value yields `option<T>`.

Examples:

    ~audio_in: float(-1.0..0.1)
    @pan:      float(-1.0..1.0) = 0.0
    $midi_in:  Midi_Message[]

Within modules, the current value of a stream can be accessed via the identifier without the symbol.

---

## Literals

### Numeric
- Integers: `42`, `-127`, `2048`, `2_048` 
- Floats: `3.14`, `-2.71`, `20_000.0`
- Underscores can be omitted or used consistently to group the digits in groups of 3 around the decimal dot.

### Strings
- Written with single quotes: `'hello world'`

### Time
- Physical: `100ms`
- Musical: `1beat`

### Frequencies
- Physical: `440Hz`
- Musical: `A3`
