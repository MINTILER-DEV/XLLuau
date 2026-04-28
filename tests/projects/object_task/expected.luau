type Animal = { name: string, sound: string, speak: (self: Animal) -> string }
local Animal = {}
Animal.__index = Animal
function Animal.new(name: string, sound: string): Animal
    local self = setmetatable({} :: Animal, Animal)
    self.name = name
    self.sound = sound
    return self
end
function Animal:speak(): string
    return self.sound
end
type Dog = Animal & { breed: string, speak: (self: Dog) -> string }
local Dog = {}
Dog.__index = Dog
setmetatable(Dog, { __index = Animal })
function Dog.new(name: string, breed: string): Dog
    local self = Animal.new(name, "Woof") :: Dog
    setmetatable(self, Dog)
    self.breed = breed
    return self
end
function Dog:speak(): string
    return Animal.speak(self)
end
local function loadDog(name: string): thread
    return coroutine.create(function()
        local dog = coroutine.yield(Dog.new(name, "Mixed"))
        return dog
    end)
end
local _co0 = loadDog("Poppy")
local _ok1, _result2 = coroutine.resume(_co0)
while _ok1 and coroutine.status(_co0) ~= "dead" do
    _ok1, _result2 = coroutine.resume(_co0, _result2)
end
if _ok1 then
    local dog = _result2
    print(dog:speak(), dog.breed)
else
    local err = _result2
    warn("load failed", err)
end
