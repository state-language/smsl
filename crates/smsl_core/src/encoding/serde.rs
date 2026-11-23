use std::{collections::BTreeMap, fmt, marker::PhantomData};

use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, Visitor},
};

pub(crate) fn unique_map<'de, D, T>(deserializer: D) -> Result<BTreeMap<String, T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct UniqueMap<T>(PhantomData<T>);

    impl<'de, T: Deserialize<'de>> Visitor<'de> for UniqueMap<T> {
        type Value = BTreeMap<String, T>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an object with unique keys")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut values = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<String, T>()? {
                if values.insert(key.clone(), value).is_some() {
                    return Err(de::Error::custom(format!("duplicate key {key:?}")));
                }
            }
            Ok(values)
        }
    }

    deserializer.deserialize_map(UniqueMap(PhantomData))
}

pub(crate) fn unique_optional_map<'de, D, T>(
    deserializer: D,
) -> Result<Option<BTreeMap<String, T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct OptionalMap<T>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>> Visitor<'de> for OptionalMap<T> {
        type Value = Option<BTreeMap<String, T>>;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("null or an object with unique keys")
        }
        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
        fn visit_some<D: Deserializer<'de>>(
            self,
            deserializer: D,
        ) -> Result<Self::Value, D::Error> {
            unique_map(deserializer).map(Some)
        }
    }
    deserializer.deserialize_option(OptionalMap(PhantomData))
}
