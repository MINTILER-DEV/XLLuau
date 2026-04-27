type Direction = "North" | "South" | "East" | "West"
local Direction = table.freeze({
    North = "North" :: Direction,
    South = "South" :: Direction,
    East = "East" :: Direction,
    West = "West" :: Direction,
})
local origin = {x = 0, y = 0}
local point = {x = 3, y = 4}
local dir: Direction = Direction.East
local _de0
do
    local dx = (point.x - origin.x)
    local dy = (point.y - origin.y)
    _de0 = math.sqrt(((dx ^ 2) + (dy ^ 2)))
end
local distance = _de0
local _sw2 = dir
local _swexpr1
if _sw2 == Direction.North then
    _swexpr1 = "N"
elseif _sw2 == Direction.South then
    _swexpr1 = "S"
elseif _sw2 == Direction.East then
    _swexpr1 = "E"
else
    _swexpr1 = "W"
end
local label = _swexpr1
local _sw3 = dir
if _sw3 == Direction.North then
    print("north")
elseif _sw3 == Direction.South then
    print("south")
elseif _sw3 == Direction.East then
    print("east")
elseif _sw3 == Direction.West then
    print("west")
end
print(distance, label)
