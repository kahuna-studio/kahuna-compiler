# Functions

Function definitions are assignments to a symbol (function name) within the current scope.

    fib = (n int, b bool) -> if n < 2 then n else fib(n - 1) + fib(n - 2)

Function bodies can be blocks: optional sequences of assignments followed by a mandatory expression, which is the
return value.

    fib = (n int, b bool) int ->
      if n < 2 then
        n
      else
        let a = fib(n - 1)
        let b = fib(n - 2)
        a + b

- Return type is optional
- Parameter types are optional if the compiler is able to infer it from the context
- The function itself is available within the function body scope (recursion)

## Optional parameters

    do_someting = (a int, b int = 42) bool -> ...

- Optional parameters have to be at the end of the parameter list. 
- Types of optional parameters can be omitted:

    let do_something = (a int, b = 42) bool -> ...
    

## Calling functions

    fib(3)
    fib(n = 3)

## Automatic currying / partial application

Is a function called with mandatory parameters missing, then a function is returned that expects the missing parameters.

    add = (a int, b int) -> a + b

    add32 = add(32)
    result = add_32(10)   // -> 42 

    add32 = add(b = 32)
    result = add_32(10)   // -> 42 
