local raw = "  HeLLo WoRLD  "
local player = {}
player.Character = {}
local _obj0 = player.Character
_obj0.Humanoid = {}
local _obj1 = player.Character.Humanoid
_obj1.GetHealth = function()
    return 100
end
local list = {}
local _pipe2 = raw:trim()
local _pipe3 = _pipe2:lower()
local words = _pipe3:split(" ")
local _opt4 = nil
do
    local _cur5 = player
    if _cur5 ~= nil then
        _cur5 = _cur5.Character
        if _cur5 ~= nil then
            _cur5 = _cur5.Humanoid
            if _cur5 ~= nil then
                _cur5 = _cur5:GetHealth()
                _opt4 = _cur5
            end
        end
    end
end
local health = _opt4
local _opt6 = nil
do
    local _cur7 = list
    if _cur7 ~= nil then
        _cur7 = _cur7[1]
        _opt6 = _cur7
    end
end
local first = _opt6
print(table.concat(words, ", "), health, first)
