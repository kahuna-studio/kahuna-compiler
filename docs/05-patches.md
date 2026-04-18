# Patches

Patches are defined with a visual editor. The editor creates a readable textual representation to be able to analyze
diffs and merge branches.

    patch Simple_Synth:
    
      patch:
    
        (midi:0).note_event -> in.{poly:0}
        {poly:0}.out -> note_on_to_freq -> freq.[Sine_Osc:0]
        [Sine_Osc:0].out -> in.0.[Multiply:0]
        {poly:0}.out -> note_to_gate -> gate.[Adsr_Envelope:0]
        [Adsr_Envelope:0].env -> in.1.[Multiply:0]
        (midi:0).velocity -> in.2.[Multiply:0]
        [Multiply:1].out -> in.{merge:0}
        {merge:0}.out -> in.[Reverb:0]
        [Reverb:0].out -> in.(audio:0)

- Every line represents a connection from one node to another.
- There might be a type converter in between. 
- Nodes can be
  - module instances (`[My_Module:42]`),
  - interfaces (`(audio:0)` or `(midi:0)`), or 
  - voice allocators (`{poly:5}`) and mergers (`{merge:5}`) for polyphony. 
- The number behind the colons is the index of the module instance / interface index / the polyphonic region.

