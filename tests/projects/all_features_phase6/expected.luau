type Comparable = number
type User = { id: number, name: string }
type Direction = "North" | "South"
local Direction = table.freeze({
    North = "North" :: Direction,
    South = "South" :: Direction,
})
type Hero = { name: string, score: number, label: (Hero) -> string }
local Hero = {}
Hero.__index = Hero
function Hero.new(name: string, score: number): Hero
    local self = setmetatable({} :: Hero, Hero)
    self.name = name
    self.score = score
    return self
end
function Hero:label(): string
    return self.name
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
local DEFAULTS = table.freeze({bonus = 2})
local settings = {}
local _lhs2 = settings.bonus
local bonus = (if _lhs2 ~= nil then _lhs2 else DEFAULTS.bonus)
local _comp3 = {}
for _, n in {1, 2, 3, 4} do
    if (n > 1) then
        table.insert(_comp3, (n * 2))
    end
end
local values = _comp3
local hero = Hero.new("Ava", max(bonus, 5))
local _opt4 = nil
do
    local _cur5 = hero
    if _cur5 ~= nil then
        _cur5 = _cur5.name
        _opt4 = _cur5
    end
end
local _lhs6 = _opt4
local who = (if _lhs6 ~= nil then _lhs6 else "Guest")
local _cond7 = (bonus > 3)
local style = (if _cond7 then "bold" else "plain")
local dir: Direction = Direction.North
local function loadUser(name: string): thread
    return coroutine.create(function()
        local user = coroutine.yield({id = hero.score, name = name})
        return user
    end)
end
local _co8 = loadUser(who)
local _ok9, _result10 = coroutine.resume(_co8)
while _ok9 and coroutine.status(_co8) ~= "dead" do
    _ok9, _result10 = coroutine.resume(_co8, _result10)
end
if _ok9 then
    local user = _result10
    local _sw11 = dir
    if _sw11 == Direction.North then
        print(user.name, style, values[1])
    elseif _sw11 == Direction.South then
        print("south")
    end
else
    local err = _result10
    warn(err)
end
return hero:label(), values
