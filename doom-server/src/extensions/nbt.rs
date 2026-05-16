use valence::nbt::{Compound, List, Value};

#[derive(Clone, Debug)]
pub struct NbtValue(pub Value);

macro_rules! from_nbt {
    ($t:ty, $variant:ident) => {
        impl From<$t> for NbtValue {
            #[inline]
            fn from(v: $t) -> Self {
                NbtValue(Value::$variant(v))
            }
        }
    };
}

from_nbt!(i8, Byte);
from_nbt!(i16, Short);
from_nbt!(i32, Int);
from_nbt!(i64, Long);
from_nbt!(f32, Float);
from_nbt!(f64, Double);
from_nbt!(Vec<i8>, ByteArray);
from_nbt!(Vec<i32>, IntArray);
from_nbt!(Vec<i64>, LongArray);
from_nbt!(List<String>, List);
from_nbt!(Compound<String>, Compound);

impl From<String> for NbtValue {
    #[inline]
    fn from(v: String) -> Self {
        NbtValue(Value::String(v))
    }
}
impl From<&str> for NbtValue {
    #[inline]
    fn from(v: &str) -> Self {
        NbtValue(Value::String(v.to_string()))
    }
}

impl From<NbtValue> for Value {
    #[inline]
    fn from(v: NbtValue) -> Self {
        v.0
    }
}

#[must_use]
#[inline]
pub fn nbt_equals(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Byte(l), Value::Byte(r)) => l == r,
        (Value::Short(l), Value::Short(r)) => l == r,
        (Value::Int(l), Value::Int(r)) => l == r,
        (Value::Long(l), Value::Long(r)) => l == r,
        (Value::Float(l), Value::Float(r)) => l.to_bits() == r.to_bits(),
        (Value::Double(l), Value::Double(r)) => l.to_bits() == r.to_bits(),
        (Value::ByteArray(l), Value::ByteArray(r)) => l == r,
        (Value::String(l), Value::String(r)) => l == r,
        (Value::List(l), Value::List(r)) => l == r,
        (Value::Compound(l), Value::Compound(r)) => l == r,
        (Value::IntArray(l), Value::IntArray(r)) => l == r,
        (Value::LongArray(l), Value::LongArray(r)) => l == r,
        _ => false,
    }
}
