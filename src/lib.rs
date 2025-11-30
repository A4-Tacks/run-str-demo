use crate::value::{Cmp, Value};
use Kind::*;
use char_classes::any;
use std::{collections::HashMap, fmt, mem};

mod value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Ident,
    Number,
    StringLit,
    Punct,
    Unknown,
}

impl Kind {
    /// Returns `true` if the kind is [`Ident`].
    ///
    /// [`Ident`]: Kind::Ident
    #[must_use]
    fn is_ident(&self) -> bool {
        matches!(self, Self::Ident)
    }
}

trait StrExt {
    fn next_boundary(&self, at: usize) -> &str;
}

impl StrExt for str {
    #[track_caller]
    fn next_boundary(&self, at: usize) -> &str {
        let Some(ch) = self[at..].chars().next() else {
            panic!("Cannot take next_boundary at {at} for {self}")
        };
        &self[..at+ch.len_utf8()]
    }
}

pub trait Config {
    fn print(&mut self, args: fmt::Arguments<'_>);
}

#[derive(Debug, Clone)]
pub struct Rt<'a, Cfg> {
    src: &'a str,
    i: usize,
    vars: HashMap<&'a str, Value>,
    ignore_level: u32,
    pub cfg: Cfg,
}

impl<Cfg: Default> Default for Rt<'_, Cfg> {
    fn default() -> Self {
        Self::with_config(Default::default())
    }
}

impl<Cfg: Config> Rt<'_, Cfg> {
    pub fn proc(&mut self) {
        self.skip_trivias();
        loop {
            match self.kind() {
                Unknown => break self.eof_or_error("Invalid input"),
                _ => self.stmt(),
            }
        }
    }

    fn stmt(&mut self) {
        match self.tok() {
            "if" => self.if_(),
            "while" => self.while_(),
            "{" => self.block(),
            _ => {
                self.cmd();
                self.expect_and_bump(";", "semicolon");
            },
        }
    }

    fn expect_and_bump(&mut self, s: &str, msg: &str) {
        if self.tok() != s {
            self.error(&format!("Expected a {msg}"));
        }
        self.bump(s);
    }

    fn while_(&mut self) {
        self.bump("while");
        let mark = self.mark();

        loop {
            let mut cond = false;
            self.expr(|_, v| cond = v.bool());

            if !cond {
                self.ignore();
                self.block();
                self.ognore();
                return;
            }

            self.block();
            self.back(mark);
        }
    }

    fn if_(&mut self) {
        self.bump("if");
        let mut cond = true;
        self.expr(|this, v| if !v.bool() {
            cond = false;
            this.ignore();
        });

        self.block();

        if !cond {
            self.ognore();
        }
    }

    fn block(&mut self) {
        self.bump("{");
        loop {
            match self.kind() {
                Punct if self.tok() == "}" => break,
                _ => self.stmt(),
            }
        }
        self.expect_and_bump("}", "right brace");
    }

    fn cmd(&mut self) {
        match self.tok() {
            "print" => {
                self.bump("print");
                self.expr(|this, v| {
                    this.cfg.print(format_args!("{v}\n"))
                })
            },
            var if self.kind().is_ident() => {
                self.bump(var);
                self.expect_and_bump("=", "`=`");
                self.expr(|this, v| {
                    this.vars.insert(var, v);
                });
            }
            _ => self.error("Expected a command or assign"),
        }
    }

    fn expr(&mut self, f: impl FnOnce(&mut Self, Value)) {
        let value = self.atom_and_mixed(0);
        if self.effect() {
            f(self, value)
        }
    }

    fn atom_and_mixed(&mut self, min_bp: u32) -> Value {
        let mut value = self.atom_and_prefix(min_bp);

        loop {
            let Some((bp, rbp)) = self.prec() else { break };
            if bp < min_bp { break }
            let tok = self.tok();
            macro_rules! infix {
                ($($op:literal $method:ident $(($($t:tt)*))?,)+) => {
                    match tok {
                        $($op => {
                            self.bump($op);
                            let rhs = self.atom_and_mixed(rbp);
                            if self.effect() {
                                value.$method(rhs $(, $($t)*)?);
                            }
                        })+
                        _ => self.error("Invalid operator")
                    }
                };
            }
            match tok {
                "||" if value.bool() => {
                    self.bump("||");
                    self.ignore();
                    self.atom_and_mixed(rbp);
                    self.ognore();
                }
                "&&" if !value.bool() => {
                    self.bump("&&");
                    self.ignore();
                    self.atom_and_mixed(rbp);
                    self.ognore();
                }
                _ => infix! {
                    "+" apply_add,
                    "-" apply_sub,
                    "*" apply_mul,
                    "/" apply_div,
                    "%" apply_rem,
                    "<"  apply_cmp(Cmp::Lt),
                    "<=" apply_cmp(Cmp::Le),
                    ">"  apply_cmp(Cmp::Gt),
                    ">=" apply_cmp(Cmp::Ge),
                    "==" apply_cmp(Cmp::Eq),
                    "!=" apply_cmp(Cmp::Ne),
                    "&&" apply_replace,
                    "||" apply_replace,
                }
            }
        }

        self.atom_apply_suffix(&mut value, min_bp);
        value
    }

    fn atom_apply_suffix(&self, _value: &mut Value, _min_bp: u32) {}

    fn atom_and_prefix(&mut self, min_bp: u32) -> Value {
        let bp = self.prec_prefix();
        match self.tok() {
            "-" if bp >= min_bp => {
                self.bump("-");
                let mut value = self.atom_and_mixed(bp);
                value.apply_neg();
                value
            }
            "!" if bp >= min_bp => {
                self.bump("!");
                let mut value = self.atom_and_mixed(bp);
                value.apply_not();
                value
            }
            "(" => {
                self.bump("(");
                let value = self.atom_and_mixed(bp);
                self.expect_and_bump(")", "close parentheses");
                value
            }
            _ => {
                self.atom()
            },
        }
    }

    fn atom(&mut self) -> Value {
        if !self.effect() {
            self.bump_any(self.tok());
            return Value::Null;
        }
        match self.kind() {
            Ident => {
                let name = self.tok();
                let val = self.vars.get(name).cloned()
                    .unwrap_or_else(|| self.error(&format!("Unknown variable `{name}`")));
                self.bump(name);
                val
            },
            Number => {
                let num = self.tok();
                let val = num.parse().map(Value::Number)
                    .unwrap_or_else(|e| self.error(&format!("Invalid number ({e})")));
                self.bump(num);
                val
            },
            StringLit => {
                let tok = self.tok();
                let content = &tok[1..tok.len()-1];
                if tok.starts_with('"') {
                    let mut escape = false;
                    let mut buf = String::with_capacity(content.len());
                    for ch in content.chars() {
                        match ch {
                            'n' if escape => buf.push('\n'),
                            'r' if escape => buf.push('\r'),
                            't' if escape => buf.push('\t'),
                            '"' if escape => buf.push('"'),
                            '\\' if escape => buf.push('\\'),
                            '\\' => { escape = true; continue },
                            _ if escape => self.error(&format!("Invalid soft string escape `\\{ch}`")),
                            _ => buf.push(ch),
                        }
                        escape = false;
                    }
                    self.bump(tok);
                    Value::String(buf)
                } else {
                    self.bump(tok);
                    Value::String(content.to_owned())
                }
            }
            kind => self.error(&format!("Invalid expression {kind:?}")),
        }
    }

    fn prec(&self) -> Option<(u32, u32)> {
        let tok = self.tok();
        let (prec, left) = match tok {
            ")" => (0, true),
            "||" => (2, true),
            "&&" => (3, true),
            "==" | "!=" => (4, true),
            "<" | ">" | "<=" | ">=" => (5, true),
            "+" | "-" => (6, true),
            "*" | "/" | "%" => (7, true),
            _ => return None,
        };

        Some((prec, prec + u32::from(left)))
    }

    fn prec_prefix(&self) -> u32 {
        if self.tok() == "(" {
            1
        } else {
            8
        }
    }

    fn _prec_suffix(&self) -> u32 {
        9
    }
}

impl<'a, Cfg> Rt<'a, Cfg> {
    pub fn with_config(cfg: Cfg) -> Self {
        let mut vars = HashMap::new();
        vars.insert("null", Value::Null);
        Self {
            src: Default::default(),
            i: Default::default(),
            vars,
            ignore_level: Default::default(),
            cfg,
        }
    }

    pub fn load_source(&mut self, src: &'a str) {
        self.src = src;
    }

    fn ignore(&mut self) {
        self.ignore_level += 1;
    }

    fn ognore(&mut self) {
        self.ignore_level -= 1;
    }

    fn effect(&self) -> bool {
        self.ignore_level == 0
    }

    #[track_caller]
    fn eof_or_error(&mut self, msg: &str) {
        self.skip_trivias();
        if self.i == self.src.len() {
            return;
        }
        self.error(msg);
    }

    #[track_caller]
    fn error(&self, msg: &str) -> ! {
        let (line, col) = line_column::line_column(self.src, self.i);
        let preview = self.mind(any!(^"\r\n"));
        if preview.is_empty() {
            panic!("{msg} at {line}:{col} (EOF)")
        } else {
            panic!("{msg} at {line}:{col} `{preview}`")
        }
    }

    fn tok(&self) -> &'a str {
        match self.kind() {
            Ident => self.ident(),
            Punct => self.punct(),
            Number => self.number(),
            StringLit => self.string(),
            Unknown => "",
        }
    }

    fn rest(&self) -> &'a str {
        &self.src[self.i..]
    }

    #[track_caller]
    fn bump(&mut self, s: &str) {
        debug_assert_eq!(&self.rest()[..s.len()], s);
        self.bump_any(s)
    }

    fn bump_any(&mut self, s: &str) {
        self.i += s.len();
        self.skip_trivias();
    }

    fn skip_trivias(&mut self) {
        loop {
            self.i += self.mind(any!(" \t\r\n")).len();
            if !self.rest().starts_with("//") { break }
            self.i += self.mind(any!(^"\n")).len();
        }
    }

    #[track_caller]
    fn mind(&self, pred: fn(char) -> bool) -> &'a str {
        self.mind_at(0, pred)
    }

    #[track_caller]
    fn mind_at(&self, at: usize, pred: fn(char) -> bool) -> &'a str {
        let rest = &self.rest()[at..];
        rest.split_once(|ch| !pred(ch))
            .map_or(rest, |it| it.0)
    }

    fn kind(&self) -> Kind {
        let Some(ch) = self.rest().chars().next() else { return Unknown };
        match ch {
            any!(@"a-zA-Z_") => Ident,
            any!(@"0-9") => Number,
            any!(@"-+*/%<=>!&|{}()[];") => Punct,
            any!(@"'\"") => StringLit,
            _ => Unknown,
        }
    }

    fn ident(&self) -> &'a str {
        self.mind(any!("a-zA-Z0-9_"))
    }

    fn number(&self) -> &'a str {
        self.mind(any!("0-9."))
    }

    fn string(&self) -> &'a str {
        let rest = self.rest();
        if let Some(content) = rest.strip_prefix('"') {
            let mut escape = false;
            for (i, ch) in content.char_indices() {
                if mem::take(&mut escape) {
                    continue;
                }
                if ch == '\\' { escape = true }
                if ch == '"' {
                    return rest.next_boundary(i+1);
                }
            }
            self.error("String literal not terminated")
        } else {
            let Some(term) = rest[1..].find('\'') else {
                self.error("String literal not terminated")
            };
            rest.next_boundary(term+1)
        }
    }

    fn punct(&self) -> &'a str {
        const DOUBLE_OPS: [&str; 6] = ["&&", "||", "<=", ">=", "==", "!="];
        let rest = self.rest();
        let double = DOUBLE_OPS.iter().any(|op| rest.starts_with(op));
        rest.next_boundary(double.into())
    }
}

mod mark {
    use crate::Rt;

    #[derive(Debug, Clone, Copy)]
    pub struct Mark(usize);
    impl<Cfg> Rt<'_, Cfg> {
        pub(crate) fn mark(&self) -> Mark {
            Mark(self.i)
        }

        pub(crate) fn back(&mut self, Mark(mark): Mark) {
            debug_assert!(mark < self.i);
            self.i = mark;
        }
    }
}

#[cfg(test)]
mod tests {
    use expect_test::{Expect, expect};

    use super::*;

    #[derive(Debug)]
    struct Output(String);
    impl Config for Output {
        fn print(&mut self, args: fmt::Arguments<'_>) {
            fmt::write(&mut self.0, args).unwrap();
        }
    }

    #[track_caller]
    fn check(src: &str, expect: Expect) {
        let rt = run(src);
        let actual = rt.cfg.0.as_str();
        if actual == "\n" {
            expect.assert_eq("<only has empty newline>");
        } else {
            expect.assert_eq(actual);
        }
    }

    #[track_caller]
    fn run(src: &str) -> Rt<'_, Output> {
        if src.trim().contains('\n') {
            println!("Run case ...");
        } else {
            println!("Run case `{}`", src.trim());
        }
        let mut rt = Rt::with_config(Output(String::new()));
        rt.load_source(src);
        rt.proc();

        assert_eq!(rt.ignore_level, 0, "Not cleanly effects");

        rt
    }

    #[test]
    fn print_number() {
        check("print 2;", expect![[r#"
            2
        "#]]);
        check("print 2.3;", expect![[r#"
            2.3
        "#]]);
        check("print 0;", expect![[r#"
            0
        "#]]);
    }

    #[test]
    fn comments() {
        check("print 2; // foo", expect![[r#"
            2
        "#]]);
        check("print 2.3;//foo", expect![[r#"
            2.3
        "#]]);
        check(r#"
            //foo
            print 0;"#,
            expect![[r#"
            0
        "#]]);
    }

    #[test]
    fn multi_print_number() {
        check("print 2; print 3;", expect![[r#"
            2
            3
        "#]]);
        check("print 2; print 3; print 4.2;", expect![[r#"
            2
            3
            4.2
        "#]]);
    }

    mod ops {
        use super::*;

        #[test]
        fn simple_neg() {
            check("print -2; print - -2;", expect![[r#"
                -2
                2
            "#]]);
        }

        #[test]
        fn simple_not() {
            check("print !null;", expect![[r#"
                1
            "#]]);
            check("print !0;", expect![[r#"
                NULL
            "#]]);
            check("print !!null;", expect![[r#"
                NULL
            "#]]);
            check("print !!3;", expect![[r#"
                1
            "#]]);
            check("print !!'';", expect![[r#"
                1
            "#]]);
        }

        #[test]
        fn add() {
            check("print 1+2;", expect![[r#"
                3
            "#]]);
        }

        #[test]
        fn sub() {
            check("print 3-2;", expect![[r#"
                1
            "#]]);
        }

        #[test]
        fn rem() {
            check("print 5%2;", expect![[r#"
                1
            "#]]);
            check("print 5%3;", expect![[r#"
                2
            "#]]);
            check("print 6%3;", expect![[r#"
                0
            "#]]);
            check("print -4%3;", expect![[r#"
                -1
            "#]]);
        }

        #[test]
        fn multi_sub() {
            check("print 3-2-1;", expect![[r#"
                0
            "#]]);
        }

        #[test]
        fn multi_sub_and_div() {
            check("print 3-1/2-2;", expect![[r#"
                0.5
            "#]]);
        }

        #[test]
        fn multi_sub_and_neg() {
            check("print 3 - - 2;", expect![[r#"
                5
            "#]]);
        }

        #[test]
        fn parens() {
            check("print 3-(2-1);", expect![[r#"
                2
            "#]]);
            check("print 3-((2)-1);", expect![[r#"
                2
            "#]]);
            check("print 3-((2)-(1));", expect![[r#"
                2
            "#]]);
            check("print (3)-((2)-(1));", expect![[r#"
                2
            "#]]);
            check("print ((3))-((2)-(1));", expect![[r#"
                2
            "#]]);
        }

        #[test]
        fn cmp() {
            check("print 1 < 2;", expect![[r#"
                1
            "#]]);
            check("print 1 <= 1;", expect![[r#"
                1
            "#]]);
            check("print 1 < 1;", expect![[r#"
                NULL
            "#]]);
            check("print 1 > 1;", expect![[r#"
                NULL
            "#]]);
            check("print 1 < 2 == 3 < 4;", expect![[r#"
                1
            "#]]);
            check("print 1 < 2 != 3 < 4;", expect![[r#"
                NULL
            "#]]);
        }

        #[test]
        fn cmp_equal() {
            check("print 2 == 1;", expect![[r#"
                NULL
            "#]]);
            check("print 2 != 1;", expect![[r#"
                1
            "#]]);
            check("print 2 == null;", expect![[r#"
                NULL
            "#]]);
            check("print 2 != null;", expect![[r#"
                1
            "#]]);
        }

        #[test]
        fn logic_and() {
            check("print 1 && 2;", expect![[r#"
                2
            "#]]);
            check("print 0 && 2;", expect![[r#"
                2
            "#]]);
            check("print null && 2;", expect![[r#"
                NULL
            "#]]);
            check("print null && null;", expect![[r#"
                NULL
            "#]]);
            check("print 2 && null;", expect![[r#"
                NULL
            "#]]);
        }

        #[test]
        fn logic_or() {
            check("print 1 || 2;", expect![[r#"
                1
            "#]]);
            check("print 0 || 2;", expect![[r#"
                0
            "#]]);
            check("print null || 2;", expect![[r#"
                2
            "#]]);
            check("print null || null;", expect![[r#"
                NULL
            "#]]);
            check("print 2 || null;", expect![[r#"
                2
            "#]]);
        }

        #[test]
        fn logic_prec() {
            check("print 1 && 2 || 3 && 4;", expect![[r#"
                2
            "#]]);
            check("print null && 2 || 3 && 4;", expect![[r#"
                4
            "#]]);
            check("print 1 && null || 3 && 4;", expect![[r#"
                4
            "#]]);
            check("print 1 && null || 3 && 4 || 5 && 6;", expect![[r#"
                4
            "#]]);
            check("print 1 && 2 || 3 && 4 || 5 && 6;", expect![[r#"
                2
            "#]]);
            check("print 1 && 2 || 3 && null || 5 && 6;", expect![[r#"
                2
            "#]]);
            check("print 1 && null || 3 && null || 5 && 6;", expect![[r#"
                6
            "#]]);
        }
    }

    mod str_ops {
        use super::*;

        #[test]
        fn add() {
            check(r#"print 'a'+'b';"#, expect![[r#"
                ab
            "#]]);
            check(r#"print 'a'+2;"#, expect![[r#"
                a2
            "#]]);
            check(r#"print 2+'a';"#, expect![[r#"
                3
            "#]]);
            check(r#"print 'a'+null;"#, expect![[r#"
                a
            "#]]);
            check(r#"print null+'a';"#, expect![[r#"
                a
            "#]]);
        }

        #[test]
        fn sub() {
            check(r#"print 'a.b.c'-'.';"#, expect![[r#"
                abc
            "#]]);
            check(r#"print 'a.b.c'-'';"#, expect![[r#"
                a.b.c
            "#]]);
            check(r#"print 'a..b..c'-'..';"#, expect![[r#"
                abc
            "#]]);
            check(r#"print 'a..b..c'-'.';"#, expect![[r#"
                abc
            "#]]);
            check(r#"print 'a.b.c'-'..';"#, expect![[r#"
                a.b.c
            "#]]);
        }

        #[test]
        fn mul() {
            check(r#"print ''*2;"#, expect!["<only has empty newline>"]);
            check(r#"print 'a'*2;"#, expect![[r#"
                aa
            "#]]);
            check(r#"print 'ab'*2;"#, expect![[r#"
                abab
            "#]]);
            check(r#"print 'a'*1;"#, expect![[r#"
                a
            "#]]);
            check(r#"print 'ab'*1;"#, expect![[r#"
                ab
            "#]]);
            check(r#"print 'ab'*0;"#, expect!["<only has empty newline>"]);
            check(r#"print 'ab'*-1;"#, expect![[r#"
                ba
            "#]]);
            check(r#"print 'ab'*-2;"#, expect![[r#"
                baba
            "#]]);
            check(r#"print 'ab'*null;"#, expect!["<only has empty newline>"]);
            check(r#"print 'ab'*'x';"#, expect![[r#"
                ab
            "#]]);
        }

        #[test]
        fn div() {
            check(r#"print 'abc'/0;"#, expect![[r#"
                abc
            "#]]);
            check(r#"print 'abc'/1;"#, expect![[r#"
                bc
            "#]]);
            check(r#"print 'abc'/2;"#, expect![[r#"
                c
            "#]]);
            check(r#"print 'abc'/3;"#, expect!["<only has empty newline>"]);
            check(r#"print 'abc'/4;"#, expect!["<only has empty newline>"]);
            check(r#"print ''/0;"#, expect!["<only has empty newline>"]);
            check(r#"print ''/1;"#, expect!["<only has empty newline>"]);
            check(r#"print ''/2;"#, expect!["<only has empty newline>"]);
            check(r#"print '测试'/0;"#, expect![[r#"
                测试
            "#]]);
            check(r#"print '测试'/1;"#, expect![[r#"
                试
            "#]]);
            check(r#"print '测试'/2;"#, expect!["<only has empty newline>"]);
            check(r#"print '测试'/3;"#, expect!["<only has empty newline>"]);
            check(r#"print '测试'/4;"#, expect!["<only has empty newline>"]);
        }

        #[test]
        fn rem() {
            check(r#"print 'abc'%0;"#, expect!["<only has empty newline>"]);
            check(r#"print 'abc'%1;"#, expect![[r#"
                a
            "#]]);
            check(r#"print 'abc'%2;"#, expect![[r#"
                ab
            "#]]);
            check(r#"print 'abc'%3;"#, expect![[r#"
                abc
            "#]]);
            check(r#"print 'abc'%4;"#, expect![[r#"
                abc
            "#]]);
            check(r#"print 'abc测试'%2;"#, expect![[r#"
                ab
            "#]]);
            check(r#"print 'abc测试'%3;"#, expect![[r#"
                abc
            "#]]);
            check(r#"print 'abc测试'%4;"#, expect![[r#"
                abc测
            "#]]);
            check(r#"print 'abc测试'%5;"#, expect![[r#"
                abc测试
            "#]]);
            check(r#"print 'abc测试'%6;"#, expect![[r#"
                abc测试
            "#]]);
            check(r#"print 'abc测试'%7;"#, expect![[r#"
                abc测试
            "#]]);
        }

        #[test]
        fn neg() {
            check(r#"print -'';"#, expect![[r#"
                0
            "#]]);
            check(r#"print -'a';"#, expect![[r#"
                1
            "#]]);
            check(r#"print -'ab';"#, expect![[r#"
                2
            "#]]);
            check(r#"print -'abc';"#, expect![[r#"
                3
            "#]]);
            check(r#"print -'abc测';"#, expect![[r#"
                4
            "#]]);
            check(r#"print -'abc测试';"#, expect![[r#"
                5
            "#]]);
        }
    }

    #[test]
    fn print_hard_string() {
        check(r#"print 'foo';"#, expect![[r#"
            foo
        "#]]);
        check(r#"print 'foo\n\""';"#, expect![[r#"
            foo\n\""
        "#]]);
    }

    #[test]
    fn print_soft_string() {
        check(r#"print "foo\nbar";"#, expect![[r#"
            foo
            bar
        "#]]);
        check(r#"print "foo\"bar";"#, expect![[r#"
            foo"bar
        "#]]);
        check(r#"print "foo\tbar";"#, expect![[r#"
            foo	bar
        "#]]);
    }

    #[test]
    fn assign() {
        check("x = 2; print x;", expect![[r#"
            2
        "#]]);
        check("x = 2; x = 3; print x;", expect![[r#"
            3
        "#]]);
        check("x = 2; x = 3; print x; x = 4; print x;", expect![[r#"
            3
            4
        "#]]);
    }

    #[test]
    #[should_panic = "Invalid input"]
    fn unknown_input() {
        run("  @  ");
    }

    #[test]
    fn if_() {
        check(r#"
            if 2 {
                if null {
                    print 1;
                }
                print 2;
            }
            print 3;
        "#, expect![[r#"
            2
            3
        "#]]);
        check(r#"
            if null {
                if null {
                    print 1;
                }
                print 2;
            }
            print 3;
        "#, expect![[r#"
            3
        "#]]);
        check(r#"
            print 1;
            if 1 {
                print 2;
                if null {
                    print 3;
                    if 2 { print 4; }
                    print 5;
                }
                print 6;
            }
            print 7;
        "#, expect![[r#"
            1
            2
            6
            7
        "#]]);
    }

    #[test]
    fn while_loop() {
        check(r#"
            i = 0;
            while i < 3 {
                print i;
                i = i + 1;
            }
            print 'i: '+i;
        "#, expect![[r#"
            0
            1
            2
            i: 3
        "#]]);
        check(r#"
            i = 2;
            while i < 3 {
                print i;
                i = i + 1;
            }
            print 'i: '+i;
        "#, expect![[r#"
            2
            i: 3
        "#]]);
        check(r#"
            i = 3;
            while i < 3 {
                print i;
                i = i + 1;
            }
            print 'i: '+i;
        "#, expect![[r#"
            i: 3
        "#]]);
    }
}
