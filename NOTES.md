
Parser as an interace is being cramped to adhere to 2 fundamentally different types of constructs:
1. In case of Wav instruments:
    - **input:**
      + lua-value containing, in some arbitrary format, events that affect the instrument's output stream
      + pattern string
    - **output:**
      + ordered list of what frame to emit what event onto the instrument

2. In case of Piano note parser:
    - **input:**
      + the instrument itself
      + pattern string
    - **output:** 
      + ordered list of what frame to emit what event onto the instrument

