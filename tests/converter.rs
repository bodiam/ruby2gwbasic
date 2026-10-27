use ruby2gwbasic::convert;

#[test]
fn hello_and_empty_program() {
    assert_eq!(
        convert("puts \"Hello!\"\n").unwrap(),
        "10 PRINT \"Hello!\"\n20 END\n"
    );
    assert_eq!(convert("# Just a comment\n").unwrap(), "10 END\n");
}

#[test]
fn arithmetic_precedence_and_integer_division() {
    let basic = convert("answer = -5 / 2 + 3 * (4 - 1)\nputs answer\n").unwrap();
    assert!(basic.contains("10 V1#=(INT((-5#)/2#)+(3#*(4#-1#)))"));
    assert!(basic.contains("N#=V1#"));
    assert!(basic.contains("PRINT MID$(STR$(N#),2+(N#<0#))"));
    let basic = convert("x = 5.0 / 2\ny = -5 % 2\n").unwrap();
    assert!(basic.contains("V1#=(5.0#/2#)"));
    assert!(basic.contains("V2#=((-5#)-2#*INT((-5#)/2#))"));
}

#[test]
fn unary_minus_binds_tighter_than_division() {
    assert_eq!(
        convert("x = -5 / 2").unwrap(),
        "10 V1#=INT((-5#)/2#)\n20 END\n"
    );
}

#[test]
fn puts_distinguishes_call_parentheses_from_grouped_arguments() {
    let basic = convert("puts (2 + 3) * 4\nputs(2 + 3, 4)\n").unwrap();
    assert!(basic.contains("N#=((2#+3#)*4#)"));
    assert!(basic.contains("N#=(2#+3#)"));
    assert!(basic.contains("N#=4#"));
    assert!(convert("puts(2 + 3) * 4").is_err());
}

#[test]
fn variables_are_mapped_without_case_collisions_or_reserved_names() {
    let basic = convert("total = 1\ntoTal = 2\ninput = 'name'\ntotal += toTal\n").unwrap();
    assert!(basic.contains("V1#=1#"));
    assert!(basic.contains("V2#=2#"));
    assert!(basic.contains("V3$=\"name\""));
    assert!(basic.contains("V1#=(V1#+V2#)"));
}

#[test]
fn while_and_if_targets_are_resolved() {
    let basic = convert("n = 2\nwhile n > 0\n  if n == 2\n    puts \"two\"\n  else\n    puts \"one\"\n  end\n  n -= 1\nend\n").unwrap();
    assert_eq!(
        basic,
        "10 V1#=2#\n20 IF (V1#>0#) THEN 40\n30 GOTO 110\n40 IF (V1#=2#) THEN 60\n50 GOTO 80\n60 PRINT \"two\"\n70 GOTO 90\n80 PRINT \"one\"\n90 V1#=(V1#-1#)\n100 GOTO 20\n110 END\n"
    );
}

#[test]
fn logical_conditions_use_short_circuit_jumps() {
    let basic = convert("x = 0\nif x != 0 && 10 / x > 1\nputs 'yes'\nend\n").unwrap();
    assert!(basic.contains("20 IF (V1#<>0#) THEN 40\n30 GOTO 80\n40 IF (INT(10#/V1#)>1#) THEN 60"));
    let basic = convert("if true || 1 / 0 > 0\nputs 'safe'\nend\n").unwrap();
    assert!(basic.starts_with("10 GOTO 40\n20 IF"));
    let basic = convert("if !(1 == 2)\nputs 'yes'\nend\n").unwrap();
    assert!(basic.starts_with("10 IF (1#=2#) THEN 50\n20 GOTO 30"));
}

#[test]
fn every_branch_must_assign_before_use() {
    let error = convert("if true\nx = 1\nend\nputs x").unwrap_err();
    assert_eq!((error.line, error.column), (4, 6));
    assert!(error.message.contains("every path"));
    assert!(convert("if true\nx = 1\nelse\nx = 2\nend\nputs x").is_ok());
    assert!(convert("if false\nx = 1\nelsif true\nx = 2\nelse\nx = 3\nend\nputs x").is_ok());
    assert!(convert("if true\nx = 1\nelse\nputs x\nend").is_err());
    assert!(convert("while false\nx = 1\nend\nputs x").is_err());
}

#[test]
fn quotes_comments_and_escapes_are_not_confused() {
    let basic =
        convert("puts \"say \\\"hi\\\" # ok\" # comment\nprint 'a\\nb'\nprint \"\\t\\r\\n\"\n")
            .unwrap();
    assert!(basic.contains("PRINT (\"say \"+CHR$(34)+\"hi\"+CHR$(34)+\" # ok\")"));
    assert!(basic.contains("PRINT \"a\\nb\";"));
    assert!(basic.contains("PRINT (CHR$(9)+CHR$(13)+CHR$(10));"));
}

#[test]
fn puts_avoids_adding_an_extra_newline_and_print_does_not_add_one() {
    let basic = convert("puts \"hello\\n\"\nprint(\"one\", \"two\")\nputs\nprint\n").unwrap();
    assert!(basic.contains("PRINT P$;\n30 IF RIGHT$(P$,1)<>CHR$(10) THEN PRINT"));
    assert!(basic.contains("40 PRINT \"one\";\n50 PRINT \"two\";\n60 PRINT\n70 PRINT \"\";"));
}

#[test]
fn crlf_and_no_trailing_newline() {
    assert_eq!(
        convert("x = 1\r\nputs x").unwrap(),
        convert("x = 1\nputs x\n").unwrap()
    );
}

#[test]
fn rejects_unsupported_or_unsafe_ruby() {
    for source in [
        "def greet\nend",
        "puts [1, 2]",
        "puts \"#{name}\"",
        "puts missing",
        "x += 1",
        "x = 1\nx = 'text'",
        "x = 1\nx = 1.5",
        "if 0\nputs 'Ruby treats zero as true'\nend",
        "puts 1 + 'x'",
        "puts true",
        "flag = false",
        "while true\nputs 1",
        "else\nputs 1",
        "puts 'unfinished",
        "puts \"\\q\"",
        "puts '☃'",
        "x = 2 ** 3",
        "x = 1; puts x",
        "x = gets.chomp",
        "if true then\nend",
        "x = 9007199254740992",
        "x = 999999999999999999999999999999999999999.0",
    ] {
        assert!(convert(source).is_err(), "unexpectedly accepted {source:?}");
    }
}

#[test]
fn enforces_target_program_limits() {
    assert!(
        convert(&format!("puts '{}'", "x".repeat(256)))
            .unwrap_err()
            .message
            .contains("255")
    );
    assert!(
        convert(&format!("puts '{}'", "x".repeat(250)))
            .unwrap_err()
            .message
            .contains("generated BASIC line")
    );
    assert!(
        convert(&"puts 1\n".repeat(3300))
            .unwrap_err()
            .message
            .contains("line-number range")
    );
}

#[test]
fn examples_compile_with_valid_jump_targets() {
    for source in [
        include_str!("../examples/hello.rb"),
        include_str!("../examples/countdown.rb"),
        include_str!("../examples/fizzbuzz.rb"),
    ] {
        let basic = convert(source).unwrap();
        let lines: Vec<_> = basic.lines().collect();
        for line in &lines {
            if line.contains("GOTO ") || line.contains(" THEN ") {
                let target = line.split_whitespace().last().unwrap();
                if target.parse::<u16>().is_err() {
                    continue;
                }
                assert!(
                    lines
                        .iter()
                        .any(|candidate| candidate.starts_with(&format!("{target} "))),
                    "missing target {target}"
                );
            }
        }
    }
}
