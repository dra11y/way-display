use std::collections::HashMap;

use anyhow::Result;
use zbus::zvariant::OwnedValue;

pub trait PropertyMapExt {
    fn get_as<T>(&self, key: &str) -> Option<T>
    where
        T: TryFrom<OwnedValue>,
        T::Error: std::error::Error + Send + Sync + 'static;

    fn try_get_as<T>(&self, key: &str) -> Option<Result<T>>
    where
        T: TryFrom<OwnedValue>,
        T::Error: std::error::Error + Send + Sync + 'static;
}

impl PropertyMapExt for HashMap<String, OwnedValue> {
    fn get_as<T>(&self, key: &str) -> Option<T>
    where
        T: TryFrom<OwnedValue>,
        T::Error: std::error::Error + Send + Sync + 'static,
    {
        self.try_get_as::<T>(key).and_then(|v| v.ok())
    }

    fn try_get_as<T>(&self, key: &str) -> Option<Result<T>>
    where
        T: TryFrom<OwnedValue>,
        T::Error: std::error::Error + Send + Sync + 'static,
    {
        self.get(key)
            .map(|v| T::try_from(v.clone()).map_err(Into::into))
    }
}
