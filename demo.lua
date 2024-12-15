-- lsp disabled so we don't need to feel bad about ourselves while we litter the global scope
-- instead of literring our project file with `local`s
---@diagnostic disable: lowercase-global

-- import the `plunder` library and create a `new` project, putting it into variable `plunder`
plunder = require 'plunder'.new 'demo.plunder'

-- some helpful numbers that might get used later
tempo = 130      -- beats per minute
bps = tempo / 60 -- beats per second
beat = 1 / bps   -- seconds per beat
bitrate = 44100  -- bits per second

-- import some audio files, exposing functions to play/pause/stop them
sample = plunder:load '~/music/songs/bt.wav'
kick = plunder:load '~/music/sfx/kick.mp3'
snare = plunder:load '~/music/sfx/snare.flac'
hat = plunder:load '~/music/sfx/hat.ogg'

-- the only way to play sound in parallel is with grids,
-- which once defined, can be plugged in with what to do
-- when a character or string is encountered
--
-- new grids can be constructed by simply calling your variable holding your pludner project
song = plunder [[
        . . . . . . . .
         . , . , . , . ,
        ................
[......][......)        (......]
]]

-- sub-grid whose `play` method can be invoked from inside the `song` grid
hats = plunder {
  '....'
}
hats[1] { ['.'] = hat.start }

-- defining how to play each of the 4 playlists in the `song` grid
song[1] { ['.'] = kick.start }
song[2] { ['.'] = snare.start, [' ,'] = plunder.after(beat, snare.start) }
song[3] { ['..'] = function() hats:play(beat / 2) end }
song[4] {
  ['['] = sample.start,
  [']'] = sample.stop,
  [')'] = sample.pause,
  ['('] = sample.resume
}

-- flatten the `song` grid into a mashed sequence of samples
print(song:play(beat)) -- characters in grid `song` will be invoked at an interval of `beat`
print(hats:play(beat))
