# Projections

A **projection** describes how a value evolves **into the future** after a triggering event.  
Instead of computing values frame-by-frame imperatively, projections define a **time function**
that is evaluated for each frame after the event occurs.

This mechanism makes it possible to describe structures such as:

- envelopes (ADSR, MSEG)
- ramps and fades
- scheduled automation
- rhythmic patterns

without mutable state.

A projection is started by an **event stream** and produces a **signal**.

---

# Concept

A projection is defined as:

    project(event_stream, time_function)

Semantics:

1. When an event occurs in `event_stream`, a new projection instance starts.
2. A local time variable `t` is created and initialized to `0`.
3. On every subsequent frame, `t` increases with the frame duration.
4. The output value is computed by evaluating `time_function(t)`.

Conceptually:

    event ──► start projection

    t = 0
    t += dt
    t += dt
    ...

The result is a **signal** whose values follow the time function.

---

# `project_at_audio_rate`

    project_at_audio_rate($events, t -> expression)

Creates a projection that starts whenever an event occurs in `$events`.

Parameters:

- `$events` – event stream that triggers the projection
- `t -> expression`
  - `t` – time since the triggering event
  - `expression` – function describing the projected value

Return value:

- signal at audio rate

Example:

    @ramp =
      project($trigger, t ->
        linear(0.0, 1.0, 500ms, t)
      )

Each `trigger` event starts a ramp from `0` to `1` over `500ms`.

---

# `sample`

    sample(signal, $event)

Samples the value of `signal` at the moment an event occurs.

Semantics:

- When an event occurs, the **current value of `signal`** is captured.
- The result is an **event stream containing the sampled value**.

Example:

    $release_start = sample(@envelope, $note_off)

Each `note_off` event emits the envelope value at that moment.

This is useful when a future projection must start from the **current value** of another signal.

---

# Using `sample` inside projections

Inside a projection, `sample` can be used to detect events that occur **during the projection**.

Example:

    project($note_on, t ->
      adsr(t, sample(t, $note_off))
    )

Here:

- `t` is the time since `note_on`
- `sample(t, $note_off)` records the time when `note_off` occurs

This allows the projection to **change behavior mid-projection**.

---

# ADSR Envelope Example

The following module implements a full ADSR envelope using projections.

The envelope is defined as a **time function** that depends on:

- time since `note_on`
- time of `note_off` (if it occurs)

---

## ADSR time function

    adsr(t, t_off):

      if t_off == none or t < t_off:

          if t < attack:
              attack_curve(t / attack)

          elif t < attack + decay:
              lerp(1.0, sustain, (t - attack) / decay)

          else:
              sustain

      else:

          release_start = adsr(t_off, none)

          release_curve((t - t_off) / release) * release_start

Properties:

- Attack and decay run normally until `note_off`
- Release starts exactly at the envelope value when `note_off` occurs
- The envelope is fully described as a **pure function**

---

# Complete ADSR Module

    module Adsr_Envelope:

      inputs:
        @attack:  time = 10ms
        @decay:   time = 100ms
        @sustain: control = 0.7
        @release: time = 200ms
        @gate:    boolean = false

      outputs:
        @envelope: control(audio_rate)

      # Detect gate transitions

      let $transitions =
        @gate
          |> buffer(1, false)
          |$ last, current -> last != current
          |> _, current -> current
          |> transition ->
               transition
                 ? Transitions.note_on
                 : Transitions.note_off

      let $note_on  = $transitions |/ $ == Transitions.note_on
      let $note_off = $transitions |/ $ == Transitions.note_off


      # ADSR projection

      @envelope =
        project($note_on, t ->

          let t_off = sample(t, $note_off)

          if t_off == none or t < t_off:

            if t < @attack:
              attack_curve(t / @attack)

            elif t < @attack + @decay:
              lerp(1.0, @sustain, (t - @attack) / @decay)

            else:
              @sustain

          else:

            let release_start =
              if t_off < @attack:
                attack_curve(t_off / @attack)

              elif t_off < @attack + @decay:
                lerp(1.0, @sustain, (t_off - @attack) / @decay)

              else:
                @sustain

            release_curve((t - t_off) / @release) * release_start


    enum Transitions:
      note_on
      note_off

---

# Advantages of Projections

Using projections provides several benefits.

### Declarative behavior

The envelope is defined as a **function of time**, not as a sequence of imperative steps.

### No explicit state

There are no mutable variables storing the envelope phase.

### Precise timing

Events can interrupt projections at any time without losing accuracy.

### Composability

The same mechanism can be used for:

- envelopes
- automation curves
- LFO resets
- sequencers
- scheduled parameter changes

---

# Summary

Projections allow streams to describe **future behavior triggered by events**.

Core primitives:

    project(event, t -> value)
    sample(signal, event)

With these two operators, complex time-dependent behavior can be expressed declaratively while remaining compatible with a frame-based processing pipeline.
