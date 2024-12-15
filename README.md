```sh
# Run your project-file
# No considerations (you can export audio by using plunder.export somewhere in the file)
lua my-project

# TODO Play your project-file
# Some considerations:
# + The project-file must return a plunder.Loop
# + Every time file changes:
#   + If plunder.Loop.flat changes, you hear your changes instantly
#   + If plunder.Loop.duration changes, playing resets after the last duration
watch -n 0.01 -t 'cat my-project' | ./plunder-watch
```
