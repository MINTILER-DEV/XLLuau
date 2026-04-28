type Comparable = number
type User = { id: number, name: string }
type Direction = "North" | "South"
local Direction = table.freeze({
    North = "North" :: Direction,
    South = "South" :: Direction,
})
type Hero = { name: string, score: number, label: (self: Hero) -> string }
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
type _Signal_OnHeroLoaded = {
    _handlers: { (hero: Hero) -> () },
    connect: (self: _Signal_OnHeroLoaded, fn: (hero: Hero) -> ()) -> { disconnect: () -> () },
    once: (self: _Signal_OnHeroLoaded, fn: (hero: Hero) -> ()) -> { disconnect: () -> () },
    fire: (self: _Signal_OnHeroLoaded, hero: Hero) -> (),
}
local OnHeroLoaded: _Signal_OnHeroLoaded = {
    _handlers = {},
    connect = function(self, fn)
        table.insert(self._handlers, fn)
        return { disconnect = function()
            local _index = table.find(self._handlers, fn)
            if _index ~= nil then
                table.remove(self._handlers, _index)
            end
        end }
    end,
    once = function(self, fn)
        local conn
        conn = self:connect(function(...)
            conn:disconnect()
            fn(...)
        end)
        return conn
    end,
    fire = function(self, ...)
        for _, handler in self._handlers do
            handler(...)
        end
    end,
}
local bonus: number = 2
local _watchers_bonus_0: { (old: number, new: number) -> () } = {}
table.insert(_watchers_bonus_0, function(old, new)
    print("bonus", old, new)
end)
local function max<T>(a: (T & Comparable), b: (T & Comparable)): T
    local _ifexpr1
    local _ifcond2 = (a > b)
    if _ifcond2 then
        _ifexpr1 = a
    else
        _ifexpr1 = b
    end
    return _ifexpr1
end
local function loadUser(name: string): thread
    return coroutine.create(function()
        local user = coroutine.yield({id = 7, name = name})
        return user
    end)
end
local DEFAULTS = table.freeze({fallback = "Guest"})
type FrozenDefaults = {
    read fallback: string,
}
local _comp3 = {}
for _, value in {1, 2, 3, 4} do
    if (value > 1) then
        table.insert(_comp3, (value * 2))
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
local who = (if _lhs6 ~= nil then _lhs6 else DEFAULTS.fallback)
local _cond7 = (bonus > 3)
local style = (if _cond7 then "bold" else "plain")
local dir: Direction = Direction.North
local _conn8 = OnHeroLoaded:once(function(loaded)
    print(loaded:label())
end)
local conn = _conn8
do
    local _old9 = bonus
    bonus = bonus + 3
    if bonus ~= _old9 then
        for _, _w in _watchers_bonus_0 do
            _w(_old9, bonus)
        end
    end
end
OnHeroLoaded:fire(hero)
local _co10 = loadUser(who)
local _ok11, _result12 = coroutine.resume(_co10)
while _ok11 and coroutine.status(_co10) ~= "dead" do
    _ok11, _result12 = coroutine.resume(_co10, _result12)
end
if _ok11 then
    local user = _result12
    local _sw13 = dir
    if _sw13 == Direction.North then
        print(user.name, style, values[1])
    elseif _sw13 == Direction.South then
        print("south")
    end
else
    local err = _result12
    warn(err)
end
conn:disconnect()
return hero:label(), values
