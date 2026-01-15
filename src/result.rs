#[macro_export]
macro_rules! ok {
    ($($value:expr),* $(,)?) => {{
        let mut array = VarArray::new();
        array.push(&godot::global::Error::OK.to_variant());
        $(array.push(&$value.to_variant());)*
        array
    }};
}

#[macro_export]
macro_rules! failed {
    ($err:expr) => {{
        let mut array = VarArray::new();
        array.push(&godot::global::Error::FAILED.to_variant());
        array.push(&(i64::from(&$err)).to_variant());
        array.push(&($err.to_string()).to_variant());
        array
    }};
}
