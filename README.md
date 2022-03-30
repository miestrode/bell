<p align="center">
    <img src="assets/Bell logo + wordmark.svg#gh-light-mode-only" width="497" height="177">
    <img src="assets/Bell logo + wordmark (white).svg#gh-dark-mode-only" width="497" height="177">
</p>

<h4 align="center">
    :warning:This project is actively being worked upon. It may not compile and some features will be missing!:warning:
</h4>

---

*If you are looking for a page explaining the motivation behind this project, please go to [this](/WHY.md) page.*

# Hello, Bell!
Bell is a programming language that compiles down to datapacks (MCfunction).
It offers a rich user experience which is supported by it's advanced and user-friendly compiler, and it's high level features.

# Why should I use bell?
By using Bell, you will be able to productively create programs that run in Minecraft.

Your work process will become simpler, faster and less error-prone.
This is partly because Bell is compiled, which means it can catch common errors at compile-time and not at run-time,
and partly because Bell is very different to MCfunction.

Whilst Bell's program/module model is based on top-level declarations and files,
MCfunction's model is based on "commands" (instructions) and "functions" (procedures).

MCfunction's programming model is a bit confusing both for programming beginners and experts.
The "functions" in MCfunction don't actually take any parameters or return anything (which is generally not the common defintion of a function).
Furthermore, the commands in MCfunction are very verbose and a lot of them are needed for otherwise simple things.

# A taste of Bell
So far we have only talked about Bell code, but we think it's time you see some Bell code.

The following program implements fixed point numbers.
```rust
use std::math
use std::check

struct Fixed {
    number: Int,
    div: Int
}

extend Fixed {
    func new(digits, point_loc) {
        Fixed {
            number: digits,
            div: point_loc
        }
    }

    // Sets the divisor without changing the overall value
    func with_div(self: &Fixed, div: Int) {
        self.number *= math::pow(10, div - self.div);
        self.div = div;
    }

    // Executes an integer binary operation on two fixed-point numbers.
    func exec_op(self, other, op: func(int, int) -> int) {
        var max_div = math::max(self.div, other.div);
        self.with_divisor(max_div);
        other.with_divisor(max_div);
        
        self.number = op(self.number, other.number);
        self
    }

    func add(self, other) {
        self.exec_op(other, func(a, b) a + b)
    }

    func sub(self, other) {
        self.exec_op(other, func(a, b) a - b)
    }

    func mul(self, other) {
        self.exec_op(other, func(a, b) a * b)
    }

    func div(self, other) {
        self.exec_op(other, func(a, b) a / b)
    }
}

func main() {
    var a = Fixed::new(1, 0); // This equals 1.0.
    var b = Fixed::new(15, 1); // This equals 1.5.

    check::assert(a.add(b) == Fixed::new(25, 1)); // Their sum equals 2.5.
}
```

# Great, how do I get started?
Bell is currently in the middle of a refactor, you cannot use it at the moment
That said, progress is being made!

## The Roadmap
The current roadmap looks like this:
- [x] Front end
  - [x] Internal file tree creation
  - [x] Lexing
  - [x] Parsing
  - [x] Internal module tree creation
- [ ] Middle end
  - [x] HIR lowering
  - [ ] Type checking
    - [x] Type gathering
    - [ ] Type checking
  - [ ] MIR lowering
  - [ ] IR optimization
  - [ ] LIR lowering
  - [ ] Peep-hole optimization
- [ ] Backend
  - [ ] VDP (Virtual Data pack) lowering
  - [ ] Assembling
