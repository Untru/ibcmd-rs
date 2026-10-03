//! Deserialize the closed resource using each actual serde type's declared
//! field names. Unknown nested fields and duplicate object keys never disappear.
use serde::de::{
    self, DeserializeSeed, EnumAccess, IntoDeserializer, MapAccess, SeqAccess, VariantAccess,
    Visitor,
};
use serde::{Deserialize, Deserializer};
use std::collections::HashSet;

pub(super) fn parse<'de, T: Deserialize<'de>>(bytes: &'de [u8]) -> Result<T, serde_json::Error> {
    let mut parser = serde_json::Deserializer::from_slice(bytes);
    parser.disable_recursion_limit();
    let value = T::deserialize(Strict(&mut parser))?;
    parser.end()?;
    Ok(value)
}
struct Strict<D>(D);
struct Seed<S>(S);
impl<'de, S: DeserializeSeed<'de>> DeserializeSeed<'de> for Seed<S> {
    type Value = S::Value;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        self.0.deserialize(Strict(d))
    }
}
struct V<VV> {
    inner: VV,
    fields: Option<&'static [&'static str]>,
}
macro_rules! scalar {($($name:ident($ty:ty)),*)=>{$(fn $name<E:de::Error>(self,v:$ty)->Result<Self::Value,E>{self.inner.$name(v)})*};}
impl<'de, VV: Visitor<'de>> Visitor<'de> for V<VV> {
    type Value = VV::Value;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.expecting(f)
    }
    scalar!(
        visit_bool(bool),
        visit_i8(i8),
        visit_i16(i16),
        visit_i32(i32),
        visit_i64(i64),
        visit_i128(i128),
        visit_u8(u8),
        visit_u16(u16),
        visit_u32(u32),
        visit_u64(u64),
        visit_u128(u128),
        visit_f32(f32),
        visit_f64(f64),
        visit_char(char),
        visit_string(String),
        visit_byte_buf(Vec<u8>)
    );
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
        self.inner.visit_str(v)
    }
    fn visit_borrowed_str<E: de::Error>(self, v: &'de str) -> Result<Self::Value, E> {
        self.inner.visit_borrowed_str(v)
    }
    fn visit_bytes<E: de::Error>(self, v: &[u8]) -> Result<Self::Value, E> {
        self.inner.visit_bytes(v)
    }
    fn visit_borrowed_bytes<E: de::Error>(self, v: &'de [u8]) -> Result<Self::Value, E> {
        self.inner.visit_borrowed_bytes(v)
    }
    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        self.inner.visit_none()
    }
    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        self.inner.visit_unit()
    }
    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        self.inner.visit_some(Strict(d))
    }
    fn visit_newtype_struct<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        self.inner.visit_newtype_struct(Strict(d))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, a: A) -> Result<Self::Value, A::Error> {
        self.inner.visit_seq(S(a))
    }
    fn visit_map<A: MapAccess<'de>>(self, a: A) -> Result<Self::Value, A::Error> {
        self.inner.visit_map(M {
            inner: a,
            fields: self.fields,
            seen: HashSet::new(),
        })
    }
    fn visit_enum<A: EnumAccess<'de>>(self, a: A) -> Result<Self::Value, A::Error> {
        self.inner.visit_enum(E(a))
    }
}
struct S<A>(A);
impl<'de, A: SeqAccess<'de>> SeqAccess<'de> for S<A> {
    type Error = A::Error;
    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        s: T,
    ) -> Result<Option<T::Value>, A::Error> {
        self.0.next_element_seed(Seed(s))
    }
    fn size_hint(&self) -> Option<usize> {
        self.0.size_hint()
    }
}
struct M<A> {
    inner: A,
    fields: Option<&'static [&'static str]>,
    seen: HashSet<String>,
}
impl<'de, A: MapAccess<'de>> MapAccess<'de> for M<A> {
    type Error = A::Error;
    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        s: K,
    ) -> Result<Option<K::Value>, A::Error> {
        let Some(key) = self.inner.next_key::<String>()? else {
            return Ok(None);
        };
        if self
            .fields
            .is_some_and(|fields| !fields.contains(&key.as_str()))
        {
            return Err(de::Error::custom("unknown nested resource field"));
        }
        if !self.seen.insert(key.clone()) {
            return Err(de::Error::custom("duplicate resource object key"));
        }
        s.deserialize(key.into_deserializer()).map(Some)
    }
    fn next_value_seed<VV: DeserializeSeed<'de>>(&mut self, s: VV) -> Result<VV::Value, A::Error> {
        self.inner.next_value_seed(Seed(s))
    }
    fn size_hint(&self) -> Option<usize> {
        self.inner.size_hint()
    }
}
struct E<A>(A);
impl<'de, A: EnumAccess<'de>> EnumAccess<'de> for E<A> {
    type Error = A::Error;
    type Variant = R<A::Variant>;
    fn variant_seed<VV: DeserializeSeed<'de>>(
        self,
        s: VV,
    ) -> Result<(VV::Value, Self::Variant), A::Error> {
        let (v, r) = self.0.variant_seed(Seed(s))?;
        Ok((v, R(r)))
    }
}
struct R<A>(A);
impl<'de, A: VariantAccess<'de>> VariantAccess<'de> for R<A> {
    type Error = A::Error;
    fn unit_variant(self) -> Result<(), A::Error> {
        self.0.unit_variant()
    }
    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, s: T) -> Result<T::Value, A::Error> {
        self.0.newtype_variant_seed(Seed(s))
    }
    fn tuple_variant<VV: Visitor<'de>>(self, n: usize, v: VV) -> Result<VV::Value, A::Error> {
        self.0.tuple_variant(
            n,
            V {
                inner: v,
                fields: None,
            },
        )
    }
    fn struct_variant<VV: Visitor<'de>>(
        self,
        f: &'static [&'static str],
        v: VV,
    ) -> Result<VV::Value, A::Error> {
        self.0.struct_variant(
            f,
            V {
                inner: v,
                fields: Some(f),
            },
        )
    }
}
macro_rules! forward {($($name:ident $(($($arg:ident:$ty:ty),*))?),*)=>{$(fn $name<VV:Visitor<'de>>(self,$($($arg:$ty,)*)?v:VV)->Result<VV::Value,D::Error>{self.0.$name($($($arg,)*)?V{inner:v,fields:None})})*};}
impl<'de, D: Deserializer<'de>> Deserializer<'de> for Strict<D> {
    type Error = D::Error;
    forward!(deserialize_any,deserialize_bool,deserialize_i8,deserialize_i16,deserialize_i32,deserialize_i64,deserialize_i128,deserialize_u8,deserialize_u16,deserialize_u32,deserialize_u64,deserialize_u128,deserialize_f32,deserialize_f64,deserialize_char,deserialize_str,deserialize_string,deserialize_bytes,deserialize_byte_buf,deserialize_option,deserialize_unit,deserialize_unit_struct(name:&'static str),deserialize_newtype_struct(name:&'static str),deserialize_seq,deserialize_tuple(len:usize),deserialize_tuple_struct(name:&'static str,len:usize),deserialize_map,deserialize_enum(name:&'static str,variants:&'static[&'static str]),deserialize_identifier,deserialize_ignored_any);
    fn deserialize_struct<VV: Visitor<'de>>(
        self,
        n: &'static str,
        f: &'static [&'static str],
        v: VV,
    ) -> Result<VV::Value, D::Error> {
        self.0.deserialize_struct(
            n,
            f,
            V {
                inner: v,
                fields: Some(f),
            },
        )
    }
    fn is_human_readable(&self) -> bool {
        self.0.is_human_readable()
    }
}
