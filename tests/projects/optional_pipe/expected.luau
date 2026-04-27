local _pipe0 = raw:trim()
local _pipe1 = _pipe0:lower()
local words = _pipe1:split(" ")
local _opt2 = nil
do
    local _cur3 = player
    if _cur3 ~= nil then
        _cur3 = _cur3.Character
        if _cur3 ~= nil then
            _cur3 = _cur3.Humanoid
            if _cur3 ~= nil then
                _cur3 = _cur3:GetHealth()
                _opt2 = _cur3
            end
        end
    end
end
local health = _opt2
local _opt4 = nil
do
    local _cur5 = list
    if _cur5 ~= nil then
        _cur5 = _cur5[1]
        _opt4 = _cur5
    end
end
local first = _opt4
