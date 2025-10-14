use godot::prelude::*;
use std::error::Error;

#[macro_export]
macro_rules! ok {
    ($($value:expr),* $(,)?) => {{
        let mut array = VariantArray::new();
        array.push(&godot::global::Error::OK.to_variant());
        $(array.push(&$value.to_variant());)*
        array
    }};
}

#[macro_export]
macro_rules! failed {
    ($err:expr) => {{
        let mut array = VariantArray::new();
        array.push(&godot::global::Error::FAILED.to_variant());
        array.push(&$err.to_string().to_variant());
        array
    }};
}

pub fn variant_from_result<V, E>(res: Result<V, E>) -> VariantArray
where
    V: ToGodot,
    E: Error,
{
    match res {
        Ok(value) => {
            ok!(value)
        }
        Err(err) => {
            failed!(err)
        }
    }
}
