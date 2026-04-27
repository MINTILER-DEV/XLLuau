local PI = 3.14
local _v0 = point
local _d1 = _v0
local x = _d1.x
local posY = _d1.y
local role = if _d1.role ~= nil then _d1.role else "user"
local rest = {}
for _rest2, _v in _d1 do
    if _rest2 ~= "x" and _rest2 ~= "y" and _rest2 ~= "role" then
        rest[_rest2] = _v
    end
end
local _v3 = values
local _d4 = _v3
local head = _d4[1]
local third = _d4[3]
local tail = table.move(_d4, 4, #_d4, 1, {})
function update(_param0: Point, _param1: ArrayLike)
    local _d5 = _param0
    local x = _d5.x
    local y = _d5.y
    local _d6 = _param1
    local first = _d6[1]
    local others = table.move(_d6, 2, #_d6, 1, {})
    return x, y, first, others
end
for _for7 in players do
    local _d8 = _for7
    local name = _d8.name
    local score = _d8.score
    print(name, score)
end
