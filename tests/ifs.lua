function DoStuff(x, y)
    if x == 0 then
        if y == 0 then
            console.log("a")
        elseif y == 1 then
            console.log("b")
        elseif y == 2 then
            console.log("c")
        else
            console.log("d")
        end
    else
        console.log("e")
    end
end

DoStuff(0, 3)

local v = 10
local w = 1
if (v > 5 and v <= 10) or w > 0 then
    console.log("vw!!!")
end
