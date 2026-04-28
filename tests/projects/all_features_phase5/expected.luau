type Comparable = number
type User = { id: number, name: string }
type Result<T, E = string> =
    { ok: true, value: T }
    | { ok: false, error: E }
type Settings = {
    read timeout: number,
    retries: number,
}
type Direction = "North" | "South"
local Direction = table.freeze({
    North = "North" :: Direction,
    South = "South" :: Direction,
})
local function wrap<T>(value: T): Result<T>
    return {ok = true, value = value}
end
local function max<T>(a: (T & Comparable), b: (T & Comparable)): T
    local _ifexpr0
    local _ifcond1 = (a > b)
    if _ifcond1 then
        _ifexpr0 = a
    else
        _ifexpr0 = b
    end
    return _ifexpr0
end
local DEFAULTS = table.freeze({timeout = 30, retries = 2})
type FrozenSettings = {
    read timeout: number,
    read retries: number,
}
type SettingFlags = { fast: boolean, safe: boolean }
local config = {}
local _v2 = DEFAULTS
local _d3 = _v2
local timeout = if _d3.timeout ~= nil then _d3.timeout else DEFAULTS.timeout
local retries = _d3.retries
local _lhs4 = config.timeout
local current = (if _lhs4 ~= nil then _lhs4 else timeout)
local _cond5 = (current > 20)
local label = (if _cond5 then "slow" else "fast")
local _comp6 = {}
for _, value in {1, 2, 3, 4} do
    if (value > 1) then
        table.insert(_comp6, (value * 2))
    end
end
local doubled = _comp6
local direction: Direction = Direction.North
local winner = max(current, retries)
local boxed = wrap((winner :: number))
local user = {id = 7, name = "Ada"}
local _opt7 = nil
do
    local _cur8 = user
    if _cur8 ~= nil then
        _cur8 = _cur8.name
        _opt7 = _cur8
    end
end
local _lhs9 = _opt7
local greeting = (if _lhs9 ~= nil then _lhs9 else "Guest")
local lowered = greeting:lower()
local _sw10 = direction
if _sw10 == Direction.North then
    print(label, lowered)
elseif _sw10 == Direction.South then
    print("south", doubled[1])
end
local _matched11 = false
do
    local _mbind12 = {}
    _mbind12["value"] = boxed.value
    local _mcond13 = (not _matched11) and (type(boxed) == "table" and boxed.ok == true and true)
    if _mcond13 then
        local value = _mbind12["value"]
        _matched11 = true
        print(value)
    end
end
do
    local _mbind14 = {}
    _mbind14["message"] = boxed.error
    local _mcond15 = (not _matched11) and (type(boxed) == "table" and boxed.ok == false and true)
    if _mcond15 then
        local message = _mbind14["message"]
        _matched11 = true
        print(message)
    end
end
return doubled, boxed, greeting
