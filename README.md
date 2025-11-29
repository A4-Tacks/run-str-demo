Experimental language, directly parsed and executed without constructing AST

Features:

- [x] print output
- [x] hard string literal
- [x] soft string literal
- [x] float literal
- [x] comments
- [x] operations
- [x] short-circuit operations
- [x] block
- [x] if statement
- [ ] elif & else statement
- [ ] while statement
- [ ] builtin functions call
- [ ] function
- [ ] scopes
- [ ] hygiene scopes

**Grammar**:

```abnf
proc    = *stmt
block   = { *stmt }
stmt    = "if" expr block *("elif" block) ["else" block]
        / "while" expr block
        / block
        / cmd ";"
cmd     = print expr
        / ident "=" expr
expr    = ;;pratt implements;;
trivia  = ;;any-whitespace;;
        / "//" *(%x0-9 / %xb-10ffff)
```
