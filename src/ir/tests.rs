use env_logger;
use failure;

use super::*;
use crate::ast;
use crate::test_util;

#[test]
fn entity_assignments() -> Result<(), failure::Error> {
    let _ = env_logger::try_init();

    let source = r#"
pickFirst = |a: u32, b: u32| -> u32 {
  capture = |x: u32| -> u32 { a };
  capture(b)
};
main = || -> u32 { pickFirst(1u32, 2u32) };
"#;
    let expected = Ok(());
    let actual = check_module("entity_assignments", source);

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn recursive_module_variables() -> Result<(), failure::Error> {
    let _ = env_logger::try_init();

    let source = r#"
a = || -> u32 {
  b()
};
b = || -> u32 {
  a()
};
"#;
    let expected = Ok(());
    let actual = check_module("recursive_module_variables", source);

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn lexically_scoped_closure_vars() -> Result<(), failure::Error> {
    let _ = env_logger::try_init();

    let source = r#"
a = || -> u32 {
  b = || -> u32 {
    c = 3u32;
    c
  };
  c
};
"#;
    let expected = Err(r#"error: undefined reference to `c`
- <lexically_scoped_closure_vars>:7:3
7 |   c
  |   ^
"#
    .to_owned());
    let actual = check_module("lexically_scoped_closure_vars", source);

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn ordered_local_vars() -> Result<(), failure::Error> {
    let _ = env_logger::try_init();

    let source = r#"
a = || -> u32 {
  b = c;
  c = 3u32;
  b
};
"#;
    let expected = Err(r#"error: undefined reference to `c`
- <ordered_local_vars>:3:7
3 |   b = c;
  |       ^
"#
    .to_owned());
    let actual = check_module("ordered_local_vars", source);

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn type_error() -> Result<(), failure::Error> {
    let _ = env_logger::try_init();

    let source = r#"
a = || -> u32 {
  1f32 + 2f64
};
"#;
    let expected = Err(r#"error: type error
- <type_error>:3:3
3 |   1f32 + 2f64
  |   ^^^^^^^^^^^
- <type_error>:3:10
3 |   1f32 + 2f64
  |          ^^^^ expected `f32` but got `f64`
- <type_error>:3:3
3 |   1f32 + 2f64
  |   ---- other operand has type `f32`
"#
    .to_owned());
    let actual = check_module("type_error", source);

    assert_eq!(expected, actual);

    Ok(())
}

fn check_module(name: &'static str, source: &str) -> Result<(), String> {
    use crate::parser::Parse;

    let mut codemap = codespan::CodeMap::new();
    let span = codemap
        .add_filemap(codespan::FileName::Virtual(name.into()), source.to_owned())
        .span();
    let ast_module =
        ast::Module::parse(span, source).map_err(|e| crate::diagnostic::to_string(&codemap, &e))?;

    let mut ir = Ir::new();
    ir.load(&ast_module)
        .map_err(|e| crate::diagnostic::to_string(&codemap, &e))?;
    ir.check_types()
        .map_err(|e| crate::diagnostic::to_string(&codemap, &e))?;

    test_util::render_graph(&format!(concat!(module_path!(), "::{}"), name), &ir).unwrap();

    Ok(())
}
