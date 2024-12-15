-- TODO luarocks template
-- TODO add terminator to end of invocations for every playlist-index

local utils = require "utils"

local plunder = {}

LOG = utils.Log.new(3)

plunder.Playlist = {
  meta = {
    __call = function(self, match)
      if type(match) ~= 'table' then
        LOG:error('Only Lua table can be attached as pattern-match for a clip')
        return
      end
      local sortedkeysvalues = {}
      for key, fn in utils.pairsByKeys(match, function(a, b)
        local key1 = a
        local key2 = b
        return key1 < key2
      end) do
        for i = 1, #key do
          if key:sub(i, i) == '\n' then
            LOG:error('Newlines are not allowed in keys, leading key `' .. key:escaped() .. '` to be ignored')
            return
          end
        end
        table.insert(sortedkeysvalues, { key, fn })
      end
      LOG:debug('attaching to playlist ' .. self.index)
      self.refgrid.playlists[self.index] = sortedkeysvalues
    end
  },

  new = function(refgrid, index)
    local self = {
      refgrid = refgrid,
      index = index,
    }
    setmetatable(self, plunder.Playlist.meta)
    return self
  end,
}


plunder.FlatGrid = {
  meta = {
    __tostring = function(self)
      -- if not self.border then return '<FlatGrid (of unbounded size)>' end
      local i, string = 1, '<FlatGrid'
      while self[i] ~= nil do
        string = string .. '\n  p' .. i .. ': '
        for j = 1, 1000 do
          if self[i][j] ~= nil then string = string .. j .. ',' end
        end
        i = i + 1
      end
      return string .. '>'
    end
  },

  new = function()
    local self = {
      mark = plunder.FlatGrid.mark,
    }
    setmetatable(self, plunder.FlatGrid.meta)
    return self
  end,

  mark = function(self, playlist_index, frame, fn)
    if not self[playlist_index] then
      self[playlist_index] = { [frame] = fn }
      LOG:debug('Inserting new frame [' .. playlist_index .. '][' .. frame .. ']')
    else
      self[playlist_index][frame] = fn
      LOG:debug('Inserting new frame [' .. playlist_index .. '][' .. frame .. ']')
    end
  end
}


plunder.Grid = {
  playlists = {},
  gridlist = nil,

  meta = {
    __index = function(self, i)
      return plunder.Playlist.new(self, i)
    end
  },

  play = function(self, interval)
    return self:flatten()
  end,

  flatten = function(self)
    LOG:debug(('flattening with %d playlists in the grid'):format(#self.playlists))
    local get_char -- iterator with items as single-character strings

    if type(self.gridlist) == "string" then
      get_char = function(n)
        if n <= #self.gridlist then return self.gridlist:sub(n, n) end
      end
    elseif type(self.gridlist) == "table" then
      local char_map = utils.CacheMap.new(
        { [1] = { char = 1, line = 1 } },
        function(cache_map, n)
          local last = cache_map[n - 1]
          if last == nil then
            -- last elem already out of bounds
            return nil
          elseif (last.char + 1) <= #self.gridlist[last.line] then
            -- last serial element was in the same line
            return { char = last.char + 1, line = last.line }
          elseif self.gridlist[last.line + 1] ~= nil then
            -- last serial element was in previous line and line after exists, add newline
            return { char = 0, line = last.line + 1 }
          else
            -- last elem in prev line and line after doesn't exist
            return nil
          end
        end
      )
      get_char = function(n)
        local pos = char_map[n]
        if pos then
          if pos.char == 0 then
            return '\n'
          else
            return self.gridlist[pos.line]:sub(pos.char, pos.char)
          end
        end
      end
    else
      LOG:error('Grids can be generated from strings or array of strings only')
      return
    end

    local gridstring = setmetatable({}, { __index = function(_, i) return get_char(i) end })
    local playlist_index, read, read_previous_lines = 1, 1, 0
    local flat_grid = plunder.FlatGrid.new()

    while gridstring[read] ~= nil do
      -- if newline, move to next line and start matching against next pattern
      -- newline can only occur at [read] because keys cannot have newlines and we move by 1 on
      --   unmatched keys
      if gridstring[read] == '\n' then
        LOG:debug('encountered \\n, going to next playlist-index ' .. (playlist_index + 1))
        read_previous_lines = read
        read = read + 1
        playlist_index = playlist_index + 1
      end
      if not self.playlists[playlist_index] then
        LOG:warn('No pattern-match attached for clip ' .. playlist_index)
        while gridstring[read] and gridstring[read] ~= '\n' do read = read + 1 end
      else
        for _, pair in ipairs(self.playlists[playlist_index]) do
          local pattern_end = utils.string_match(gridstring, read, pair[1])
          -- on match, update how much read
          if pattern_end then
            LOG:debug(('pattern matched at %d:%d'):format(read, pattern_end))
            flat_grid:mark(playlist_index, read - read_previous_lines, pair[2])
            read = pattern_end
            break
          end
        end
      end
      -- go to next character whether matches or not
      read = read + 1
    end
    -- TODO flat_grid.border = { playlist_index, read }
    return flat_grid
  end,

  new = function(gridlist)
    local self = {
      gridlist = gridlist,
      playlists = {},

      play = plunder.Grid.play,
      flatten = plunder.Grid.flatten,
    }
    setmetatable(self, plunder.Grid.meta)
    return self
  end,
}


plunder.Sample = {
  open = function(filename)
    local self = {
      start = function() return true end,
      stop = function() return true end,
      pause = function() return true end,
      resume = function() return true end,
    }
    return self
  end,
}


plunder.Project = {
  meta = {
    __call = function(_, gridlist)
      return plunder.Grid.new(gridlist)
    end
  },

  load = function(self, filepath)
    self.samples[filepath] = plunder.Sample.open(filepath)
    return self.samples[filepath]
  end,

  new = function()
    local self = {
      samples = {},

      load = plunder.Project.load,
      after = plunder.after,
    }
    setmetatable(self, plunder.Project.meta)
    return self
  end,
}


-- Utility Functions
function plunder.after(duration, callback)
  return true
end

function plunder.new(project_name)
  LOG:info(('New project `%s`'):format(project_name))
  return plunder.Project.new()
end

return plunder
