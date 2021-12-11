<p align="center">
    <img src="assets/Bell logo + wordmark.svg#gh-light-mode-only" width="497" height="177">
    <img src="assets/Bell logo + wordmark (white).svg#gh-dark-mode-only" width="497" height="177">
</p>

---

Bell is a programming language that is designed to be readable, expressive and high-level (in comparison to MCfunction).
Bell's compiler is created to be efficient, error tolerant and generate code that runs **fast**.

Bell is high-level compared to MCfunction, and features many advanced features not seen in it. Such features include:
* Expression oriented constructs
* Conditionals
* Parameterized functions
* Structures (Classes)
* Loops
* Type inference
* Strings*

*Strings are compile time values, so they don't offer all of the flexibility of values like integers for example.

## Progress
Bell has finished it's first release. The goal of this release was to provide a simplistic, working version of Bell, to get something out there. There are some parts I am really proud of, others less.

I've learned a lot while developing this release. And there are so many parts I'd like to work on now. Bell will be getting a rewrite in the near future so that it has more features, better and original syntax and produces more optimized code.

## Known bugs
It is possible for condition branches to effect other ones. I.E:
```
if x == 3 {
    println(x);
    x = x + 1;
} else if x == 5 {
    println(x);
} else if x == 4 {
    println(x);   
}
```
will print:
```
3
4
```
This is actually a common bug in many compilers like this (such as Debris and WASMcraft).


A fix will arrive once I redo Bell's MIR. The gist of the fix is to lower `if-else-if-else` into `if-else` like so:
```
if x == 2 {
    // Snip!
} else {
    if x == 3 {
        // Snip!
    } else {
        // Snip!
    }
}
```
In more detail, the actual fix involves tracking which branch was taken using a special variable.
