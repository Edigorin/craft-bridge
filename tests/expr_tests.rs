//! Tests for the expression engine: dimensional evaluation with contextual
//! units, meaning-preserving translation, and hostile-input bounds.

use craft_bridge::replay::expr::{self, Context, Dim, ExprError, Quantity};

fn env<'a>(pairs: &'a [(&str, f64, Dim)]) -> impl Fn(&str) -> Option<Quantity> + 'a {
    move |name| {
        pairs
            .iter()
            .find(|(n, _, _)| *n == name)
            .map(|(_, v, d)| Quantity { value: *v, dim: *d })
    }
}

fn eval(src: &str, pairs: &[(&str, f64, Dim)]) -> Result<Quantity, ExprError> {
    let node = expr::parse(src)?;
    expr::eval(&node, &Context::millimetres(), &env(pairs))
}

#[test]
fn literals_carry_units_and_bare_numbers_follow_context() {
    let q = eval("12 mm", &[]).unwrap();
    assert!((q.value - 0.012).abs() < 1e-12);
    assert_eq!(q.dim, Dim::Length);

    let q = eval("3 deg", &[]).unwrap();
    assert!((q.value - 3.0 * std::f64::consts::PI / 180.0).abs() < 1e-12);
    assert_eq!(q.dim, Dim::Angle);

    // A bare result reads in the context default (millimetres here).
    let q = eval("2", &[]).unwrap();
    assert!((q.value - 0.002).abs() < 1e-12);
    assert_eq!(q.dim, Dim::Length);

    // A bare result in a count context stays a count.
    let node = expr::parse("7").unwrap();
    let q = expr::eval(
        &node,
        &Context {
            bare_length: 0.001,
            bare_angle: 1.0,
            bare_dim: Dim::Count,
            bare_text: "mm",
        },
        &env(&[]),
    )
    .unwrap();
    assert_eq!(
        q,
        Quantity {
            value: 7.0,
            dim: Dim::Count
        }
    );
}

#[test]
fn bare_operands_stay_dimensionless_in_products() {
    let pairs = [("width", 0.04, Dim::Length)];
    // 2 * width doubles the length; 2 is not two millimetres.
    let q = eval("2 * width", &pairs).unwrap();
    assert_eq!(q.dim, Dim::Length);
    assert!((q.value - 0.08).abs() < 1e-12, "{}", q.value);

    let q = eval("width / 2", &pairs).unwrap();
    assert!((q.value - 0.02).abs() < 1e-12);

    let q = eval("2 + 3", &[]).unwrap(); // bare + bare stays poly -> context
    assert!((q.value - 0.005).abs() < 1e-12);

    let q = eval("width + 3 mm", &pairs).unwrap();
    assert!((q.value - 0.043).abs() < 1e-12);
}

#[test]
fn dimension_mismatches_are_refused() {
    let pairs = [("width", 0.04, Dim::Length), ("alpha", 1.0, Dim::Angle)];
    assert!(eval("width + alpha", &pairs).is_err());
    assert!(eval("3 mm * 2 mm", &[]).is_err());
    assert!(eval("2 + width / alpha", &pairs).is_err());
    // Same dimensions divide to a count.
    let q = eval("3 mm / 2 mm", &[]).unwrap();
    assert_eq!(q.dim, Dim::Count);
    assert!((q.value - 1.5).abs() < 1e-12);
}

#[test]
fn hostile_and_malformed_expressions_are_refused() {
    assert!(matches!(eval("1 / 0", &[]), Err(ExprError::DivideByZero)));
    assert!(matches!(
        eval("width ^ 0.5", &[("width", 1.0, Dim::Length)]),
        Err(ExprError::Unsupported(_))
    ));
    assert!(matches!(
        eval("2 width", &[]),
        Err(ExprError::Unsupported(_))
    )); // implicit multiplication
    assert!(matches!(
        eval("sqrt(2)", &[]),
        Err(ExprError::Unexpected(_))
    ));
    assert!(matches!(eval("", &[]), Err(ExprError::UnexpectedEnd)));
    assert!(matches!(
        eval("mystery", &[]),
        Err(ExprError::UnknownName(_))
    ));

    let long = "1 + ".repeat(120);
    assert!(matches!(
        eval(&long, &[]),
        Err(ExprError::TooLong(_)) | Err(ExprError::TooManyTokens(_)) | Err(ExprError::TooDeep(_))
    ));

    let deep = format!("{}1{}", "(".repeat(40), ")".repeat(40));
    assert!(matches!(eval(&deep, &[]), Err(ExprError::TooDeep(_))));

    let mut name = "a".repeat(80);
    name.push_str(" + 1");
    assert!(matches!(eval(&name, &[]), Err(ExprError::BadIdentifier(_))));

    // A long identifier is rejected rather than truncated into an alias.
    let mut eq = "\"".to_string();
    eq.push_str(&"b".repeat(300));
    assert!(expr::parse(&eq).is_err());
}

#[test]
fn translation_preserves_meaning() {
    let node = expr::parse("2 * width").unwrap();
    assert_eq!(
        expr::translate(&node, &Context::millimetres()),
        "2 * \"width\""
    );

    let node = expr::parse("40 mm").unwrap();
    assert_eq!(expr::translate(&node, &Context::millimetres()), "40mm");

    let node = expr::parse("-0.5 in").unwrap();
    assert_eq!(expr::translate(&node, &Context::millimetres()), "-0.5in");

    let node = expr::parse("(a + 1) * 2").unwrap();
    assert_eq!(
        expr::translate(&node, &Context::millimetres()),
        "(\"a\" + 1mm) * 2"
    );

    let node = expr::parse("3 deg").unwrap();
    assert_eq!(expr::translate(&node, &Context::millimetres()), "3deg");
}

#[test]
fn round_trip_eval_matches_translation_semantics() {
    // What the equation must produce equals what evaluation expects.
    let pairs = [("width", 0.04, Dim::Length), ("depth", 0.012, Dim::Length)];
    let node = expr::parse("width / 2 + depth").unwrap();
    let q = expr::eval(&node, &Context::millimetres(), &env(&pairs)).unwrap();
    assert!((q.value - 0.032).abs() < 1e-12);
    assert_eq!(
        expr::translate(&node, &Context::millimetres()),
        "(\"width\" / 2 + \"depth\")"
    );
}
