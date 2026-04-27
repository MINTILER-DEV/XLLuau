export type Box<T = string> = { value: T }

local foo: { [string]: number, bar: (number, string) -> () } = {
    a = 0,
    bar = function(_x, _y)
    end,
}

foo.bar(1, "x")
foo["a"] += 1

local cast = foo :: any
local label = if cast then `value: {cast}` else "none"

for key, value in foo do
    if key then
        continue
    end
end

print(label, foo["a"])
