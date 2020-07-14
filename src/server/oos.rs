use std::ops::Deref;
use std::sync::Arc;

use crate::server::oos::Oos::{Owned, Shared};

pub enum Oos<T> {
    Shared(Arc<T>),
    Owned(T),
}

impl<T> Deref for Oos<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Shared(v) => v.deref(),
            Owned(v) => v
        }
    }
}