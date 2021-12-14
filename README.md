<p align="center">
    <img src="assets/Bell logo + wordmark.svg#gh-light-mode-only" width="497" height="177">
    <img src="assets/Bell logo + wordmark (white).svg#gh-dark-mode-only" width="497" height="177">
</p>

<h2 align="center">
    :warning:This project is actively being worked on in another branch. Some features are missing!:warning:
</h2>

---

# Background
Running code in Minecraft: Java Edition (abberviated by MJE) is done using a language called MCfunction.

MCfunction code, as it's name is made up of a bunch of differents "functions". Although they are callled functions, they're really more like
procedures in that they take no parameters and don't return anything.

Every function is made up of a list of "commands". A typical command may look like this:
```mcfunction
scoreboard objectives add foo dummy
```

Unfortunately, MCfunction is verbose: It requires many lines just to do something as simple as multipling and adding 4 numbers. Furthermore, it lacks many programming constructs essential to doing useful calculations.

So why should we spend time writing so many lines for what could should have been done in little? It's time to go beyond MCfunction.

# Introducing Bell!
Bell is a programming language that is readable, expressive and concise.
Additionaly, it's compiler is fast and error tolerant.

Bell offers features many advanced features not seen in MCfunction. Such features include:
* Expression oriented constructs
* Conditionals
* Parameterized functions
* Structures (Classes)
* Loops
* Type inference
* Strings[^1]

[^1]: Strings are compile time values, so they don't offer all of the flexibility of values like integers for example.

# A taste of Bell
Heres example of Bell's high-level features. The following a MCfunction program prints 10 factorial:

> at `program:init`
> ```mcfunction
> scoreboard objectives add variables dummy
> ```
>
> at `program:main`
> ```mcfunction
> scoreboard players set current variables 10
> scoreboard players set product variables 1
> function program:main_calc
> tellraw @a {"score": {"name": "product", "objective": "variables"}}
> ```
>
> at `program:main_calc`
> ```mcfunction
> scoreboard players operation product variables *= current variables
> scoreboard players remove current variables 1
> execute if score current variables matches 2.. run function program:main_calc
> ```

Heres it's equivalent Bell program:
> ```go
> func factorial(number) {
>    var product = 1;
>     
>    loop {
>        if number == 0 {
>            break product
>        }
> 
>        product *= number;
>        number -= 1;
>    }
> }
> 
> func main() {
>    println(factorial(10))
> }
>```

As previously mentioned, Bell can also do other interesting things, like creating structures.
```go
struct Car {
    name,
    color,
    kph
}

func main() {
    var my_car = Car {
        name: "Cary McCarface",
        color: "Red",
        kph: 123
    };
    
    my_car.color = "Blue"
    println("My car's color is now {my_car.color}")
}
```

Bell also supports basic interoperability with MCfunction using the `mcfunction` function:
```go
func main() {
    mcfunction("say Hello from Minecraft!")
}
```

# Getting started
Currently, the only way to run Bell is by building it from source. Make sure you have the Rust and Cargo installed.

Then, to build Bell you can just run this command (inside the installed directory):
```bash
cargo build --release
```

an executable should now be seen inside the `target` directory. Now run it with the `--help` argument, and start writing code! 

# Contributing
For minor fixes, feel free to open a pull request. For more major ones, please open an issue first to explain what your would like to change.

# Known bugs
A branch in one conditional can currently effect others. For example:
```go
func main() {
    var x = 0;
    
    if x == 0 {
        x += 1;
    } else if x == 1 {
        println(x); // This prints! >:(
    }
}
```

The fix to this will drop when the new `v2` version of Bell is finished.
