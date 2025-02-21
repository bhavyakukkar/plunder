require('plunder').global()

bitrate = 44100
piano = Synth.open './TimGM6mb.sf2'

-- TODO: melody = Midi { piano, split = '|' }
melody = Midi(piano)
melody = melody:parse [[
  a#5 g#5 f5 a#5 g#5 f5 a#5 g#5 f5 a#5 g#5 f5 f5 g#5 a#5 a#5 g#5 f5 a#5 g#5 f5 a#5 g#5 f5 a#5 g#5 f5 c#5 d#5 c#5 d#5 c#5 d#5 d5 d5 e5 e5 e5 e5 c#5 d#5 c#5 d#5 c#5 d#5 a5 a5 b5 b5 b5 b5
]]

help(piano)

Debug(melody)

render(
  "out.wav",
  { piano }, -- list of instruments whose output will be rendered
  bitrate,
  bitrate / 4,  -- every unit is one-fourth of a second
  bitrate * 8, -- render 8 seconds of audio
  { walk(melody) }
)
