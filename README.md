# plunder

+ TODO should ClipEvent's be clip-dependent? On the restrictive case that they are not, only a single clip can be played by a list where the list parsing yields a sequence of clip-events that apply to the passed clip. i think making them clip-dependent (`kick.start`) is the way to go
+ TODO piano note parser should also account for velocity of individual notes. way to account for in default parser would also be good

```sh
# Run your project-file
# you can export audio by using plunder.export somewhere in the file
# you can play audio by using plunder.play somewhere in the file
lua demo.lua
```
