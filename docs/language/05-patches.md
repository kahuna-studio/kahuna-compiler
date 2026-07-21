# Patches

Patches are defined with a visual editor. The editor creates a readable textual representation to be able to analyze
diffs and merge branches.

    patch Simple_Synth:
    
      patch:
    
        (midi_in:0).note_events -> in.{poly}
        {poly}.out -> midi_event_to_frequency_parameter -> frequency.[Sine_Osc:0]
        [Sine_Osc:0].out -> in.0.[Multiply:0]
        {poly}.out -> midi_event_to_gate_parameter -> gate.[Gate_Envelope:0]
        [Gate_Envelope:0].envelope -> in.1.[Multiply:0]
        [Multiply:1].out -> in.{merge}
        {merge}.out -> left.(audio_out:0)
        {merge}.out -> right.(audio_out:0)

- Every line represents a connection from one node to another.
- There might be a type converter in between. 
- Nodes can be
  - module instances (`[My_Module:42]`),
  - interfaces (`(audio_out:0)` or `(midi_in:0)`), or 
  - voice allocators (`{poly}`) and mergers (`{merge}`) for polyphony. 
- The number behind the colons is the index of the module instance / interface index / the polyphonic region.

