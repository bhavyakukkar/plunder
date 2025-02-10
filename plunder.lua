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

---
---Render the given set of `instruments` with the given `event-stream iterator` by spacing each unit with `interval` no. of samples and stopping after `duration` no. of samples
---
---@generic T: table, V
---@param path string
---@param instruments table
---@param interval integer
---@param duration integer
---@param event_stream_fun fun(table: V[], i?: integer):integer, V
---@param event_stream_obj T
---@param event_stream_initial_value integer
plunder.render  = function(path, instruments, interval, duration, event_stream_fun, event_stream_obj,
                           event_stream_initial_value)
  libplunder.render(path, instruments, interval, duration, event_stream_fun, event_stream_obj, event_stream_initial_value)
end

---@param value any
plunder.help    = function(value) libplunder.help(value) end

---@param value any
plunder.Debug   = function(value) libplunder.Debug(value) end

plunder.Sampler = libplunder.Sampler
plunder.Parser  = libplunder.Parser

---
---Add all plunder items to the global scope
---
plunder.global  = function()
  for key, val in pairs(plunder) do
    _G[key] = val
  end
end

return plunder
