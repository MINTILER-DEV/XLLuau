type Player = { Name: string }
type _Signal_OnPlayerJoined = {
    _handlers: { (player: Player) -> () },
    connect: (self: _Signal_OnPlayerJoined, fn: (player: Player) -> ()) -> { disconnect: () -> () },
    once: (self: _Signal_OnPlayerJoined, fn: (player: Player) -> ()) -> { disconnect: () -> () },
    fire: (self: _Signal_OnPlayerJoined, player: Player) -> (),
}
local OnPlayerJoined: _Signal_OnPlayerJoined = {
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
type _Signal_OnDied = {
    _handlers: { () -> () },
    connect: (self: _Signal_OnDied, fn: () -> ()) -> { disconnect: () -> () },
    once: (self: _Signal_OnDied, fn: () -> ()) -> { disconnect: () -> () },
    fire: (self: _Signal_OnDied) -> (),
}
local OnDied: _Signal_OnDied = {
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
local playerCount: number = 0
local _watchers_playerCount_0: { (old: number, new: number) -> () } = {}
local currentMap: string? = nil
local _watchers_currentMap_1: { (old: string?, new: string?) -> () } = {}
local _conn2 = OnPlayerJoined:connect(function(player)
    print("joined", player.Name)
end)
local conn = _conn2
OnDied:once(function()
    print("died")
end)
table.insert(_watchers_playerCount_0, function(old, new)
    print("count", old, new)
end)
table.insert(_watchers_currentMap_1, function(old, new)
    print("map", old, new)
end)
do
    local _old3 = playerCount
    playerCount = (playerCount + 1)
    if playerCount ~= _old3 then
        for _, _w in _watchers_playerCount_0 do
            _w(_old3, playerCount)
        end
    end
end
do
    local _old4 = playerCount
    playerCount = playerCount + 2
    if playerCount ~= _old4 then
        for _, _w in _watchers_playerCount_0 do
            _w(_old4, playerCount)
        end
    end
end
if currentMap == nil then
    local _old5 = currentMap
    currentMap = "Lobby"
    if currentMap ~= _old5 then
        for _, _w in _watchers_currentMap_1 do
            _w(_old5, currentMap)
        end
    end
end
OnPlayerJoined:fire({Name = "Ada"})
OnDied:fire()
conn:disconnect()
return playerCount, currentMap
