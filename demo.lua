-- lsp disabled so we don't need to feel bad about ourselves while we litter the global scope
-- instead of littering our project file with `local`s
---@diagnostic disable: lowercase-global

-- import the `plunder` library and create a `new` project, putting it into variable `plunder`
plunder = require 'plunder'.new 'demo.plunder'

-- some helpful numbers that might get used later
tempo = 130      -- beats per minute
bps = tempo / 60 -- beats per second
beat = 1 / bps   -- seconds per beat
bitrate = 44100  -- bits per second

-- import some audio files, exposing functions to play/pause/stop them
sample = plunder:load './bt.wav'
kick = plunder:load './bt.wav'
snare = plunder:load './bt.wav'
hat = plunder:load './bt.wav'
-- piano = plunder:load_sf '~/music/songs/bt.wav'

-- check the 'bt.wav' sample we've loaded by playing the first 5 seconds of it
-- sample:play(5)

-- new grids can be constructed by simply calling your variable holding your plunder project
song = plunder [[
        . . . . . . . .
         . , . , . , . ,
        ................
[......][......)        (......]
piano|>                       <|
]]

-- defining how to play each of the 4 playlists in the `song` grid
song[1] { ['.'] = kick.play }
-- song[2] { ['.'] = snare.play, [' ,'] = plunder.after(beat, snare.play) }
song[2] { ['.'] = snare.play, [' ,'] = snare.play }
song[4] {
  ['['] = sample.play,
  [']'] = sample.stop,
  [')'] = sample.pause,
  ['('] = sample.resume
}

-- sub-grid whose `play` method can be invoked from inside the `song` grid
-- we're doing this so we can play two hats in the interval of a single character
-- hats = plunder {
--   '....'
-- }
-- hats[1] { ['.'] = hat.play }
-- song[3] { ['..'] = function() hats:play(beat / 2) end }

-- melody = plunder 'C5 Eb5 G5 Bb5 D6 Bb5 G5 Eb5'

-- using the .p switch to tell plunder we want to use our own parser
--[[
--  the default parser attaches every matched string to the index where the string was
--  encountered, but we don't want that to be done for our piano notes
--  instead, parse_note will parse notes from the string into a list of notes
--]]
-- melody .p (piano.parse_note)
-- attaching the `melody` sub-grid to our song
-- using the .r switch so we can use regex keys
-- we're doing this so we don't need to update here every time we extend the duration of the
-- loop in the song
-- song[5] .r { ['piano|> *<|'] = function() melody:loop(beat) end }
-- song[5] { ['piano|> *<|'] = function() melody:loop(beat) end } 'regex'

-- flatten the `song` grid into a mashed sequence of samples
print(song:play(beat)) -- characters in grid `song` will be invoked at an interval of `beat`
-- print(hats:play(beat))

-- render the first 8 bars (8x4 = 32 beats) of the song
-- song:render('remix.wav', 32)
