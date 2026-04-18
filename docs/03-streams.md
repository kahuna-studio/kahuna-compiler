# Streams

A **stream** is a sequence of values that flows through a module. Streams can carry different types of data, and their
behavior depends on their **stream type**. Streams are the primary way to represent **audio signals**,
**parameters** (think of knobs and automation), and **events** (such as MIDI).

| Stream Prefix | Name             | Description                                                                       | Default Rate   |
|---------------|------------------|-----------------------------------------------------------------------------------|----------------|
| `~`           | Signal           | Streams of continuous numeric values (e.g., audio samples).                       | `audio_rate`   |
| `@`           | Parameter Stream | Streams of control values (e.g., knobs, automation). Buffer holds the last value. | `control_rate` |
| `$`           | Event            | Streams of discrete events (e.g., MIDI messages, Note_On/Note_Off).               | n/a            |

---

## Reading Streams

- Streams can be **read multiple times** inside a module without consuming them.
- Parameter streams maintain their **current value**, which can be accessed inside pipelines **without the `@` prefix** for readability.

---

## Stream Constructors

New streams can be created directly inside a module:

- Signal:

  ```
  ~out = ~() // Produces silence (0.0) by default.
  ```

- Parameter stream with a default value:

  ```
  @cutoff = @(1000.0)  
  @resonance = @(0.5)
  ```

### Special constructors

    Audio_Rate_Phase(@frequency)

---

## Pipelines

Pipelines transform streams step by step. They start from a **source** stream and apply operators in sequence.  
Operators are written using the pipe syntax.

### `|` map
Apply a function to every value in the stream.

    ~out = ~in
      | x -> x * gain

Is the same as

    ~out = ~in
      | $ * gain   // $ represents the current value

Is the same as

    ~out = ~in
      | * gain


---

### `#` filter
Filter stream values or events by a predicate.

    $note_on_events = $midi_in
      # msg -> msg:type == Note_On

Is the same as

    $note_on_events = $midi_in
      # $:type == Note_On

A filtered out value on a parameter stream leads the parameter stream to keep the previous value.

If signals get filtered they are transformed into parameter streams.

---

### `?` and `: ... =>` match
Pattern match on events or control changes, branching by type or condition.

    $out = $midiIn
      ?
        : NoteOn  => note -> ...
        : NoteOff => note -> ...
      | ...

Instead of taking the current value of the stream, you can also match on any other value, like the current value of a parameter stream:

    $out = Audio_Rate_Phase(@frequency)
      ? waveform
        : Waveform.sine   => pahse -> ...
        : Waveform.square => phase -> ...
      | ...

---

### `|-` branch
Duplicate the stream into multiple branches. Each branch can modify the stream independently. Branches can be projected
into the future by adding a physical or musical time vale. 
Branches must be recombined using `: ` followed by an expression to combine the values.

    ~out = ~in
      |-     in -> in
      |-29ms in -> in * 0.7
      |-37ms in -> in * 0.6
      |-41ms in -> in * 0.5
      : sum

    $midi_out = @midi_in
      |-      in -> in
      |-1beat in -> in
        ? $:type
          : Note_On => note -> note.{ velocity: * 0.5, note_number: + 12 }
          : _ => message -> message
      |-2beat in ->
        ? $:type
          : Note_On => note -> note.{ velocity: * 0.3, note_number: + 24 }
          : _ => message -> message
      : merge

    ~out = ~in
      |-      in -> in
      |-delay in -> in * 0.3
      : sum

---

### `|-->` let-binding
Give the current stream value a name to reuse it further down the pipeline.

    ~out = ~in
      |--> dry
      |-200ms x -> x * 0.8
      : sum(dry)
