
# Core

+ [x] Instrument
  + [ ] Allow extraction of not just events, but events wrapped in relative positions, so that things like after(emittable, 1sec) can be done
  + [ ] Output two channels instead of just one

+ [ ] Parser
  + [ ] Wrapper

+ [x] Engine
  + [x] Core implementation
  + [x] Instruments are lazily loaded (i.e. their output samples captured) upon the first event encountered from them


# Tooling

+ [ ] Hot-load project loop
  ```sh
  # Play your project-file
  # Some considerations:
  # + The project-file must return a plunder.Loop
  # + Every time file changes:
  #   + If plunder.Loop.flat changes, you hear your changes instantly
  #   + If plunder.Loop.duration changes, playing resets after the last duration
  watch -n 0.01 -t 'cat my-project' | ./plunder-watch
  ```

+ [ ] Soundfont support


# Modularity

+ [ ] Custom instruments in Rust
  + [x] Write it
  + [ ] Delegate it to be included in libplunder.so

+ [ ] Custom instruments in Lua

+ [ ] Custom parsers in Rust
  + [x] Write it
  + [ ] Delegate it to be included in libplunder.so

+ [ ] Custom parsers in Lua


