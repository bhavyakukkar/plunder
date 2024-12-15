local utils = {}

function utils.pairsByKeys(t, f)
  local a = {}
  for n in pairs(t) do table.insert(a, n) end
  table.sort(a, f)
  local i = 0             -- iterator variable
  local iter = function() -- iterator function
    i = i + 1
    if a[i] == nil then
      return nil
    else
      return a[i], t[a[i]]
    end
  end
  return iter
end

utils.Log = {
  new   = function(level)
    assert(type(level) == 'number')
    local log = utils.Log
    log.level = level
    return log
  end,

  error = function(self, message) if self.level >= 1 then print('ERROR :: ' .. message) end end,
  info  = function(self, message) if self.level >= 2 then print('INFO  :: ' .. message) end end,
  warn  = function(self, message) if self.level >= 3 then print('WARN  :: ' .. message) end end,
  debug = function(self, message) if self.level >= 4 then print('DEBUG :: ' .. message) end end,
}


function utils.string_match(haystack, start, needle)
  local last = nil
  for i = 1, #needle do
    local c = haystack[start + i - 1]
    if not c then return nil end
    if c ~= needle:sub(i, i) then return nil end
    if not last then last = start end
    last = last + 1
  end
  return last - 1
end

utils.CacheMap = {
  mt = {
    __index = function(self, i)
      local result = self:compute(i)
      if result then
        self[i] = result
        return result
      end
    end
  },
  new = function(initial, compute)
    local cache_map = initial
    cache_map.compute = compute
    setmetatable(cache_map, utils.CacheMap.mt)
    return cache_map
  end
}

function string.escaped(s)
  local escaped = ''
  for i = 1, #s do
    local c = s:sub(i, i)

    -- TODO incomplete
    if c == '\n' then
      escaped = escaped .. '\\n'
    else
      escaped = escaped .. c
    end
  end
  return escaped
end

-- TODO come up with a more formal tree structure for this (maybe in rust???)
function utils.matchtreev1(match)
  local root = { nil, 0 }
  -- view[1] -> fn at the key that terminates at this node (if present) : number | nil
  -- view[2] -> number of keys that either terminate or surpass this node : number
  -- view[c] -> ref to node at which all keys with c at the index of this tree's depth either
  --            terminate or surpass, or fn at key that ends with c : table | fn | nil
  local view
  for key, fn in pairs(match) do
    view = root
    for i = 1, #key do
      local c = key:sub(i, i)
      -- print('> ' .. c)
      if view[c] == nil then
        if i == #key then
          view[2] = view[2] + 1
          view[c] = fn
        else
          view[2] = view[2] + 1
          view[c] = {}
          view = view[c]
        end
      elseif type(view[c]) == 'function' then
        if i == #key then
          print("error: duplicate key")
          return
        else
          local old_fn = view[c]
          view[c] = { old_fn, 1 }
          view = view[c]
        end
      elseif type(view[c]) == 'table' then
        if i == #key then
          if view[c][1] ~= nil then
            print("error: duplicate key")
            return
          else
            view[c][2] = view[c][2] + 1
            view[c][1] = fn
          end
        else
          view = view[c]
        end
      else
        print("error: unsupported type")
        return
      end
    end
  end
  return root
end

function utils.matchtreev2(match)
  local root = {}
  local view
  for key, fn in pairs(match) do
    view = root
    for i = 1, #key do
      local c = key:sub(i, i)
      view[c] = { { key, fn } }
    end
  end
end

-- function playmatchtreev1(root, playlist)
--   local view = root
--   for i = 1, #playlist do
--     local c = playlist:sub(i, i)
--     if view[c] == nil then
--       print("ignore: no key" .. c)
--       view = root
--     elseif type(view[c]) == 'function' then
--       view[c]()
--       view = root
--     elseif type(view[c]) == 'table' then
--       if view[c][2] == 1 then
--         for
--       else
--         view = view[c]
--       end
--     else
--       print("error: unsupported type") return
--     end
--   end
-- end

return utils
