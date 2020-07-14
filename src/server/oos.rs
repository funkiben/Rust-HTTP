use std::sync::Arc;
use std::ops::Deref;
use crate::common::oos::Oos::{Shared, Owned};

pub enum Oos<T> {
    Shared(Arc<T>),
    Owned(T)
}

impl<T> Deref for Oos<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Shared(v) => v,
            Owned(v) => v
        }
    }
}