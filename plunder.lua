---@diagnostic disable: lowercase-global
local plunder = {}

-- import libplunder.so
-- default to release build, if absent, then use debug build, if absent fail
local libplunder
if pcall(function()
      libplunder = package.loadlib('target/release/libplunder.so', 'luaopen_libplunder')()
    end) then
  print('Using target/release/libplunder.so')
elseif pcall(function()
      libplunder = package.loadlib('target/debug/libplunder.so', 'luaopen_libplunder')()
    end) then
  print('Using target/debug/libplunder.so')
else
  print(
    "libplunder.so not found, please build it first by running `cargo build -p package` or `cargo build -p package --release`")
  return (1)
end

---@generic T: table, V
---@alias event_stream_iter [fun(table: V[], i?: integer):integer, V, T, integer]

---
---Render the given set of `instruments` with the given `event-stream iterator` by spacing each unit with `interval` no. of samples and stopping after `duration` no. of samples. Write as .wav to `path`
---
---@generic T: table, V
---@param path string
---@param instruments table
---@param bitrate integer
---@param interval integer
---@param duration integer
---@param event_streams table<any, event_stream_iter>
plunder.render  = function(path, instruments, bitrate, interval, duration, event_streams)
  return libplunder.render(path, instruments, bitrate, interval, duration, event_streams)
end

plunder.walk    = function(value)
  return { ipairs(value) }
end

---@param value any
plunder.help    = function(value) libplunder.help(value) end

---@param value any
plunder.Debug   = function(value) libplunder.Debug(value) end

plunder.Sampler = libplunder.Sampler
plunder.Parser  = libplunder.Parser
plunder.Synth   = libplunder.Synth
plunder.Midi    = libplunder.Midi

---
---Add all plunder items to the global scope
---
plunder.global  = function()
  -- core
  _G.render = plunder.render

  -- instruments
  _G.Sampler = plunder.Sampler
  _G.Synth = plunder.Synth

  -- parsers
  _G.Parser = plunder.Parser
  _G.Midi = plunder.Midi

  -- utils
  _G.Debug = plunder.Debug
  _G.help = plunder.help
  _G.walk = plunder.walk
end

return plunder
