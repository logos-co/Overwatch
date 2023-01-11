use std::marker::PhantomData;

pub mod const_checks;
pub mod runtime;

/// Like PhantomData<T> but without
/// ownership of T
pub struct PhantomBound<T> {
    _inner: PhantomData<*const T>,
}
