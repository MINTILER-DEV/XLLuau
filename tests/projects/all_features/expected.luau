type Direction = "North" | "South" | "East" | "West"
local Direction = table.freeze({
    North = "North" :: Direction,
    South = "South" :: Direction,
    East = "East" :: Direction,
    West = "West" :: Direction,
})
local config = {timeout = nil, filters = {1, 2, 3, 4}, mode = Direction.North}
local stats = {}
local result = {kind = "ok", value = {items = {2, 4}}}
local fallback = 99
local _lhs0 = config.timeout
local timeout = (if _lhs0 ~= nil then _lhs0 else 30)
if stats.total == nil then
    stats.total = 0
end
stats.total = stats.total + 1
local _opt1 = nil
do
    local _cur2 = result
    if _cur2 ~= nil then
        _cur2 = _cur2.value
        if _cur2 ~= nil then
            _cur2 = _cur2.items
            if _cur2 ~= nil then
                _cur2 = _cur2[1]
                _opt1 = _cur2
            end
        end
    end
end
local _lhs3 = _opt1
local selected = (if _lhs3 ~= nil then _lhs3 else fallback)
local _comp4 = {}
for _, x in config.filters do
    if (x > 1) then
        table.insert(_comp4, (x * 2))
    end
end
local doubled = _comp4
local _de5
do
    local base = (if selected ~= nil then selected else 0)
    _de5 = (base + (#doubled))
end
local distance = _de5
local _sw7 = config.mode
local _swexpr6
if _sw7 == Direction.North then
    _swexpr6 = "north"
elseif _sw7 == Direction.South then
    _swexpr6 = "south"
elseif _sw7 == Direction.East then
    _swexpr6 = "east"
else
    _swexpr6 = "west"
end
local label = _swexpr6
local _matched8 = false
do
    local _mbind9 = {}
    _mbind9["data"] = result.value
    local _mcond10 = (not _matched8) and (type(result) == "table" and result.kind == "ok" and true)
    if _mcond10 then
        local data = _mbind9["data"]
        _matched8 = true
        print(timeout, stats.total, selected, label, distance, data.items[2])
    end
end
do
    local _mbind11 = {}
    _mbind11["message"] = result.error
    local _mcond12 = (not _matched8) and (type(result) == "table" and result.kind == "err" and true)
    if _mcond12 then
        local message = _mbind11["message"]
        _matched8 = true
        print(message)
    end
end
