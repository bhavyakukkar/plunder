--- disable warnings about making globals CAPITAL
---@diagnostic disable: lowercase-global

--- bring everything to global scope
require('plunder').global()
-- plunder = require 'plunder' -- also ok

bitrate = 44100 -- bits per second

--- `Sampler` is an audio sampler packaged with plunder
--- It implements the Instrument API to add state-changing audio-sources for use in plunder
loop = Sampler.open './one.wav' --- small file, copy entirely into memory
-- long   = Sampler.open '/example-long-song.wav'   --- large file, don't copy entirely into memory
piano = Synth.open './TimGM6mb.sf2'

--- `help` can be used on an instrument to print the help/usage written by the instrument author
help(loop)

--- `Parser` (parser1) is a string parser packaged with plunder
--- Parsers in plunder are used to make writing feel more natural
beat = Parser()
beat:extend {
  ['('] = loop.resume,
  [')'] = loop.pause,
  ['['] = { loop[{ seek = "0s" }], loop.resume }, -- seek to start & resume
}
beat = beat:parse '(...,...)   (...)   *   [...,...)'

--- `Debug` can help identify plunder's internal types (hidden inside Lua UserData) as well as Lua
--- types and provide debug information for them
Debug(beat)

melody = Midi(piano)
melody = melody:parse 'A5 C6 E6 C6 F5 A5 C6 A5 C5 E5 G5 E5 G5 B5 D6 B5'

--- `render` invokes the primary plunder engine on the event stream (see below)
render(
  "out.wav",
  { piano }, -- list of instruments whose output will be rendered
  bitrate,
  bitrate / 4,  -- every unit is one-fourth of a second
  bitrate * 8, -- render 8 seconds of audio
  { walk(melody) }
)

--- the last argument to render just needs to be an iterator of the following format:
--- you may forego the parser and directly use it in this way
-- {
--   { 0, sample.resume },
--   { 0, bt.resume },
--   { 2, sample.pause },
--   { 2, bt.pause },
--   { 4, sample.resume },
--   { 4, bt.resume },
--   { 8, sample.pause },
--   { 8, bt.pause },
-- }

--- TODO
-- if terminal supports it, draw pretty output with tables and whatnot in like JSON or something

--- TODO
-- if any of plunder functions return large output, invoke a pager

--- TODO
-- automate '   <====>   '

--- TODO
-- instrument.__index(value) should return a complex type that lives as a lua value but can be
-- coerced as InstrumentAndEvent
-- for e.g.,
-- `instrument.seek` returns this value that can be coerced as
-- `InstrumentAndEvent(instrument, "seek")`, but when it's called once more, i.e.,
-- `instrument.seek('0s')`, it transforms into a value that can be coerced as `InstrumentAndEvent(instrument, {seek = '0s'})

--- TODO Scopes for filters
-- filter('reverb') { sample, bt, kick } --- attaches filters to the respective PackagedInstruments
--- <or>
-- reverb_group = { sample, bt, kick }
-- filter('reverb', reverb_group)
