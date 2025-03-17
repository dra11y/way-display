use std::collections::HashMap;

use crate::{Error, Result};
use zbus::zvariant::OwnedValue;

pub trait PropertyMapExt {
    fn get_as<T>(&self, key: &str) -> Option<T>
    where
        T: TryFrom<OwnedValue>,
        T::Error: Into<Error>;

    fn try_get_as<T>(&self, key: &str) -> Option<Result<T>>
    where
        T: TryFrom<OwnedValue>,
        T::Error: Into<Error>;
}

impl PropertyMapExt for HashMap<String, OwnedValue> {
    fn get_as<T>(&self, key: &str) -> Option<T>
    where
        T: TryFrom<OwnedValue>,
        T::Error: Into<Error>,
    {
        self.try_get_as::<T>(key).and_then(|v| v.ok())
    }

    fn try_get_as<T>(&self, key: &str) -> Option<Result<T>>
    where
        T: TryFrom<OwnedValue>,
        T::Error: Into<Error>,
    {
        self.get(key)
            .map(|v| T::try_from(v.clone()).map_err(Into::into))
    }
}
