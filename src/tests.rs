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
